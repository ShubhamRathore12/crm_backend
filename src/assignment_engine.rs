//! Auto-assignment engine: Round Robin

use sqlx::PgPool;
use uuid::Uuid;
use crate::error::AppError;

pub struct AssignmentEngine {
    pool: PgPool,
}

impl AssignmentEngine {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Assigns the next agent to an entity based on Round Robin
    pub async fn assign_next_agent(&self, entity_type: &str) -> Result<Uuid, AppError> {
        // 1. Get all active agents
        let agents = sqlx::query!(
            "SELECT id FROM users WHERE role = 'agent' AND status = 'active' ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await?;

        if agents.is_empty() {
            return Err(AppError::Internal("No active agents found for assignment".into()));
        }

        // 2. Get last assigned agent for this entity type
        let last_tracking = sqlx::query!(
            "SELECT last_agent_id FROM assignment_tracking WHERE entity_type = $1",
            entity_type
        )
        .fetch_optional(&self.pool)
        .await?;

        let last_agent_id = last_tracking.and_then(|t| t.last_agent_id);

        // 3. Find next agent
        let next_agent_id = if let Some(last_id) = last_agent_id {
            let current_index = agents.iter().position(|a| a.id == last_id);
            if let Some(index) = current_index {
                let next_index = (index + 1) % agents.len();
                agents[next_index].id
            } else {
                agents[0].id
            }
        } else {
            agents[0].id
        };

        // 4. Update tracking
        sqlx::query!(
            "INSERT INTO assignment_tracking (entity_type, last_agent_id, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (entity_type) DO UPDATE SET last_agent_id = $2, updated_at = NOW()",
            entity_type,
            next_agent_id
        )
        .execute(&self.pool)
        .await?;

        Ok(next_agent_id)
    }
}
