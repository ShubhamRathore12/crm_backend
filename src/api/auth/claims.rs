//! JWT claims for auth

use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub email: String,
    pub exp: i64,
    pub iat: i64,
}

impl Claims {
    pub fn new(user_id: Uuid, email: &str, expiry_secs: i64) -> Self {
        let now = Utc::now();
        Self {
            sub: user_id,
            email: email.to_string(),
            iat: now.timestamp(),
            exp: (now + Duration::seconds(expiry_secs)).timestamp(),
        }
    }

    pub fn encode(&self, secret: &str) -> anyhow::Result<String> {
        Ok(encode(
            &Header::default(),
            self,
            &EncodingKey::from_secret(secret.as_ref()),
        )?)
    }

    pub fn decode(token: &str, secret: &str) -> anyhow::Result<Self> {
        let d = decode::<Claims>(
            token,
            &DecodingKey::from_secret(secret.as_ref()),
            &Validation::default(),
        )?;
        Ok(d.claims)
    }
}
