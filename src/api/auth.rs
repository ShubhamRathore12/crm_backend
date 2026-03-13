//! Auth: login, register, OTP (MFA), logout

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use super::claims::Claims;
use crate::{error::AppError, AppState};
use bcrypt::verify;

mod claims;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/otp", post(otp))
        .route("/logout", post(logout))
        .route("/me", get(me))
        .route("/forgot-password", post(forgot_password))
        .route("/reset-password", post(reset_password))
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct RegisterRequest {
    pub name: String,
    pub email: String,
    pub password: String,
}

#[derive(Deserialize)]
pub struct OtpRequest {
    pub email: String,
    pub otp: String,
}

#[derive(Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Deserialize)]
pub struct ResetPasswordRequest {
    pub email: String,
    pub otp: String,
    pub new_password: String,
}

#[derive(Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub expires_in: i64,
    pub user: UserResponse,
}

#[derive(Serialize)]
pub struct UserResponse {
    pub id: String,
    pub name: String,
    pub email: String,
    pub role: String,
}

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Look up user by email
    let user = sqlx::query!(
        "SELECT id, name, email, password_hash, role FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(state.db.write_pool())
    .await?
    .ok_or(AppError::Unauthorized("Invalid email or password".into()))?;

    // Verify password
    let valid = verify(&req.password, &user.password_hash)
        .map_err(|_| AppError::Unauthorized("Invalid credentials".into()))?;

    if !valid {
        return Err(AppError::Unauthorized("Invalid email or password".into()));
    }

    // Check if user is active
    let user_status: String = sqlx::query_scalar!(
        "SELECT status FROM users WHERE id = $1",
        user.id
    )
    .fetch_one(state.db.write_pool())
    .await?;

    if user_status != "active" {
        return Err(AppError::Unauthorized("Account is disabled".into()));
    }

    // Generate JWT
    let claims = Claims::new(user.id, &user.email, state.config.jwt_expiry_secs);
    let token = claims.encode(&state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        expires_in: state.config.jwt_expiry_secs,
        user: UserResponse {
            id: user.id.to_string(),
            name: user.name,
            email: user.email,
            role: user.role,
        },
    }))
}

async fn register(
    State(state): State<AppState>,
    Json(req): Json<RegisterRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // Check if email already exists
    let existing = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(state.db.write_pool())
    .await?;

    if existing.is_some() {
        return Err(AppError::BadRequest("Email already registered".into()));
    }

    // Hash password
    let password_hash = bcrypt::hash(&req.password, 12)?;

    // Create user
    let user_id = uuid::Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO users (id, name, email, password_hash, role, status) 
         VALUES ($1, $2, $3, $4, 'agent', 'active')",
        user_id,
        req.name,
        req.email,
        password_hash
    )
    .execute(state.db.write_pool())
    .await?;

    // Generate JWT
    let claims = Claims::new(user_id, &req.email, state.config.jwt_expiry_secs);
    let token = claims.encode(&state.config.jwt_secret)?;

    Ok(Json(AuthResponse {
        token,
        expires_in: state.config.jwt_expiry_secs,
        user: UserResponse {
            id: user_id.to_string(),
            name: req.name,
            email: req.email,
            role: "agent".to_string(),
        },
    }))
}

async fn otp(
    State(state): State<AppState>,
    Json(req): Json<OtpRequest>,
) -> Result<Json<AuthResponse>, AppError> {
    // TODO: verify OTP from DB/Redis, then issue JWT
    // For now, accept any 6-digit OTP for demo
    if req.otp.len() != 6 || !req.otp.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest("Invalid OTP format".into()));
    }

    // Look up user by email
    let user = sqlx::query!(
        "SELECT id, name, email, role FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(state.db.write_pool())
    .await?
    .ok_or(AppError::Unauthorized("User not found".into()))?;

    let claims = Claims::new(user.id, &user.email, state.config.jwt_expiry_secs);
    let token = claims.encode(&state.config.jwt_secret)?;
    Ok(Json(AuthResponse {
        token,
        expires_in: state.config.jwt_expiry_secs,
        user: UserResponse {
            id: user.id.to_string(),
            name: user.name,
            email: user.email,
            role: user.role,
        },
    }))
}

async fn logout() -> Result<Json<serde_json::Value>, AppError> {
    // Stateless JWT: client discards token; optional blacklist in Redis
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn me(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<UserResponse>, AppError> {
    // Extract token from Authorization header
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(AppError::Unauthorized("Missing token".into()))?;

    // Decode and verify JWT
    let claims = Claims::decode(token, &state.config.jwt_secret)?;

    // Fetch user from database
    let user = sqlx::query!(
        "SELECT id, name, email, role FROM users WHERE id = $1",
        claims.sub
    )
    .fetch_optional(state.db.write_pool())
    .await?
    .ok_or(AppError::Unauthorized("User not found".into()))?;

    Ok(Json(UserResponse {
        id: user.id.to_string(),
        name: user.name,
        email: user.email,
        role: user.role,
    }))
}

async fn forgot_password(
    State(state): State<AppState>,
    Json(req): Json<ForgotPasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // Check if user exists
    let user = sqlx::query!(
        "SELECT id FROM users WHERE email = $1",
        req.email
    )
    .fetch_optional(state.db.write_pool())
    .await?;

    if user.is_none() {
        // Don't reveal if email exists
        return Ok(Json(serde_json::json!({ "message": "If an account exists, a recovery code has been sent." })));
    }

    // TODO: Generate OTP, save to DB/Redis, send email
    tracing::info!("Forgot password requested for: {}", req.email);
    Ok(Json(serde_json::json!({ "message": "If an account exists, a recovery code has been sent." })))
}

async fn reset_password(
    State(state): State<AppState>,
    Json(req): Json<ResetPasswordRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    // TODO: Verify OTP from DB/Redis
    // For demo, accept any valid OTP
    if req.otp.len() != 6 || !req.otp.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::BadRequest("Invalid OTP".into()));
    }

    // Hash new password
    let password_hash = bcrypt::hash(&req.new_password, 12)?;

    // Update password
    let updated = sqlx::query!(
        "UPDATE users SET password_hash = $1 WHERE email = $2",
        password_hash,
        req.email
    )
    .execute(state.db.write_pool())
    .await?;

    if updated.rows_affected() == 0 {
        return Err(AppError::BadRequest("User not found".into()));
    }

    tracing::info!("Password reset completed for: {}", req.email);
    Ok(Json(serde_json::json!({ "ok": true, "message": "Password updated successfully." })))
}
