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
        let agents = sqlx::query_scalar::<_, Uuid>(
            "SELECT id FROM users WHERE role = 'agent' AND status = 'active' ORDER BY id"
        )
        .fetch_all(&self.pool)
        .await?;

        if agents.is_empty() {
            return Err(AppError::Internal(anyhow::anyhow!("No active agents found for assignment")));
        }

        // 2. Get last assigned agent for this entity type
        let last_tracking = sqlx::query_scalar::<_, Option<Uuid>>(
            "SELECT last_agent_id FROM assignment_tracking WHERE entity_type = $1"
        )
        .bind(entity_type)
        .fetch_optional(&self.pool)
        .await?;

        let last_agent_id = last_tracking.flatten();

        // 3. Find next agent
        let next_agent_id = if let Some(last_id) = last_agent_id {
            let current_index = agents.iter().position(|a| *a == last_id);
            if let Some(index) = current_index {
                let next_index = (index + 1) % agents.len();
                agents[next_index]
            } else {
                agents[0]
            }
        } else {
            agents[0]
        };

        // 4. Update tracking
        sqlx::query(
            "INSERT INTO assignment_tracking (entity_type, last_agent_id, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (entity_type) DO UPDATE SET last_agent_id = $2, updated_at = NOW()"
        )
        .bind(entity_type)
        .bind(next_agent_id)
        .execute(&self.pool)
        .await?;

        Ok(next_agent_id)
    }
}
