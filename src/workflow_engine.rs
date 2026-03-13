//! Workflow Automation Engine
//! Executes workflow definitions with triggers, conditions, actions, delays

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;
use chrono::{DateTime, Utc, Duration};
use crate::{error::AppError, AppState};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowNode {
    pub id: String,
    pub node_type: NodeType,
    pub config: Value,
    pub position: Option<(f32, f32)>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeType {
    Trigger,
    Condition,
    Action,
    Delay,
    Webhook,
    Assign,
    Notification,
    UpdateStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub condition: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowContext {
    pub entity_id: Uuid,
    pub entity_type: String,
    pub trigger_data: Value,
    pub variables: HashMap<String, Value>,
    pub current_node: Option<String>,
    pub history: Vec<NodeExecutionResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeExecutionResult {
    pub node_id: String,
    pub node_type: NodeType,
    pub status: ExecutionStatus,
    pub result: Option<Value>,
    pub error: Option<String>,
    pub executed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    Success,
    Failed,
    Skipped,
    Pending,
}

pub struct WorkflowEngine {
    state: AppState,
}

impl WorkflowEngine {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }

    pub async fn execute_workflow(
        &self,
        workflow_id: Uuid,
        context: WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let workflow = self.load_workflow(workflow_id).await?;
        let definition: WorkflowDefinition = serde_json::from_value(workflow.definition_json)
            .map_err(|e| AppError::BadRequest(format!("Invalid workflow definition: {}", e)))?;

        self.execute_nodes(definition, context).await
    }

    async fn load_workflow(&self, workflow_id: Uuid) -> Result<crate::api::workflow::WorkflowRow, AppError> {
        sqlx::query_as!(
            crate::api::workflow::WorkflowRow,
            "SELECT * FROM workflows WHERE id = $1 AND active = true",
            workflow_id
        )
        .fetch_one(&self.state.pool)
        .await
        .map_err(|_| AppError::NotFound("Workflow not found or inactive".into()))
    }

    async fn execute_nodes(
        &self,
        definition: WorkflowDefinition,
        mut context: WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        // Find trigger node
        let trigger_node = definition.nodes
            .iter()
            .find(|n| matches!(n.node_type, NodeType::Trigger))
            .ok_or_else(|| AppError::BadRequest("No trigger node found".into()))?;

        // Start execution from trigger
        let mut current_node_id = trigger_node.id.clone();
        let mut last_result: Option<NodeExecutionResult> = None;

        while let Some(node_id) = current_node_id.clone().into() {
            let node = definition.nodes
                .iter()
                .find(|n| n.id == node_id)
                .ok_or_else(|| AppError::BadRequest(format!("Node {} not found", node_id)))?;

            context.current_node = Some(node_id.clone());

            let result = match node.node_type {
                NodeType::Trigger => self.execute_trigger(node, &context).await?,
                NodeType::Condition => self.execute_condition(node, &context).await?,
                NodeType::Action => self.execute_action(node, &context).await?,
                NodeType::Delay => self.execute_delay(node, &context).await?,
                NodeType::Webhook => self.execute_webhook(node, &context).await?,
                NodeType::Assign => self.execute_assign(node, &context).await?,
                NodeType::Notification => self.execute_notification(node, &context).await?,
                NodeType::UpdateStatus => self.execute_update_status(node, &context).await?,
            };

            last_result = Some(result.clone());

            // Find next node based on edges
            if let Some(next_node_id) = self.find_next_node(&definition, &node_id, &result).await? {
                current_node_id = next_node_id;
            } else {
                break;
            }

            // Update context history
            context.history.push(result);
        }

        last_result.ok_or_else(|| AppError::BadRequest("No nodes executed".into()))
    }

    async fn find_next_node(
        &self,
        definition: &WorkflowDefinition,
        current_node_id: &str,
        result: &NodeExecutionResult,
    ) -> Result<Option<String>, AppError> {
        let edges: Vec<_> = definition.edges
            .iter()
            .filter(|e| e.source == current_node_id)
            .collect();

        if edges.is_empty() {
            return Ok(None);
        }

        for edge in edges {
            if let Some(condition) = &edge.condition {
                if self.evaluate_condition(condition, &result.result.as_ref().map(|v| v.clone())).await? {
                    return Ok(Some(edge.target.clone()));
                }
            } else {
                return Ok(Some(edge.target.clone()));
            }
        }

        Ok(None)
    }

    async fn evaluate_condition(&self, condition: &str, data: &Option<Value>) -> Result<bool, AppError> {
        // Simple condition evaluation - in production, use a proper expression engine
        let data = data.as_ref().unwrap_or(&Value::Null);
        
        // Basic string matching for now
        if condition.contains("==") {
            let parts: Vec<&str> = condition.split("==").collect();
            if parts.len() == 2 {
                let left = parts[0].trim();
                let right = parts[1].trim().trim_matches('"');
                
                if let Some(value) = data.get(left) {
                    return Ok(value.as_str().unwrap_or("") == right);
                }
            }
        }
        
        Ok(false)
    }

    async fn execute_trigger(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        // Trigger nodes just pass through the trigger data
        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(context.trigger_data.clone()),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_condition(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let condition = node.config
            .get("condition")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Condition node missing condition".into()))?;

        let result = self.evaluate_condition(condition, &Some(context.trigger_data.clone())).await?;

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ "result": result })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_action(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let action_type = node.config
            .get("action_type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Action node missing action_type".into()))?;

        match action_type {
            "send_sms" => self.send_sms_action(node, context).await,
            "send_email" => self.send_email_action(node, context).await,
            "create_task" => self.create_task_action(node, context).await,
            _ => Err(AppError::BadRequest(format!("Unknown action type: {}", action_type))),
        }
    }

    async fn send_sms_action(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let message = node.config
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Get contact details based on entity
        let contact = self.get_contact_for_entity(&context.entity_id, &context.entity_type).await?;
        
        // In a real implementation, integrate with SMS service
        tracing::info!("Sending SMS to {}: {}", contact.mobile, message);

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ "sent_to": contact.mobile, "message": message })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn send_email_action(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let subject = node.config
            .get("subject")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        
        let body = node.config
            .get("body")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let contact = self.get_contact_for_entity(&context.entity_id, &context.entity_type).await?;
        
        // In a real implementation, integrate with email service
        tracing::info!("Sending email to {}: {} - {}", contact.email.unwrap_or_default(), subject, body);

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ 
                "sent_to": contact.email, 
                "subject": subject,
                "body": body
            })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn create_task_action(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let task_title = node.config
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Automated Task");

        let assign_to = node.config
            .get("assign_to")
            .and_then(|v| v.as_str());

        // Create task in database
        let task_id = Uuid::new_v4();
        
        if let Some(user_email) = assign_to {
            sqlx::query!(
                r#"INSERT INTO tasks (id, title, assigned_to, entity_id, entity_type, created_at)
                   VALUES ($1, $2, (SELECT id FROM users WHERE email = $3), $4, $5, NOW())"#,
                task_id,
                task_title,
                user_email,
                context.entity_id,
                context.entity_type
            )
            .execute(&self.state.pool)
            .await?;
        }

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ 
                "task_id": task_id,
                "title": task_title,
                "assigned_to": assign_to
            })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_delay(
        &self,
        node: &WorkflowNode,
        _context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let delay_seconds = node.config
            .get("seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(60);

        // In a real implementation, this would be handled by a job queue
        tokio::time::sleep(tokio::time::Duration::from_secs(delay_seconds)).await;

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ "delayed_seconds": delay_seconds })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_webhook(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let url = node.config
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Webhook node missing URL".into()))?;

        let method = node.config
            .get("method")
            .and_then(|v| v.as_str())
            .unwrap_or("POST");

        let client = reqwest::Client::new();
        let response = match method {
            "GET" => client.get(url).send().await?,
            "POST" => {
                let payload = serde_json::json!({
                    "entity_id": context.entity_id,
                    "entity_type": context.entity_type,
                    "trigger_data": context.trigger_data
                });
                client.post(url).json(&payload).send().await?
            },
            _ => return Err(AppError::BadRequest(format!("Unsupported HTTP method: {}", method))),
        };

        let status = response.status();
        let response_text = response.text().await.unwrap_or_default();

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: if status.is_success() { ExecutionStatus::Success } else { ExecutionStatus::Failed },
            result: Some(serde_json::json!({ 
                "status": status.as_u16(),
                "response": response_text
            })),
            error: if status.is_success() { None } else { Some(format!("Webhook failed: {}", status)) },
            executed_at: Utc::now(),
        })
    }

    async fn execute_assign(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let assign_to = node.config
            .get("assign_to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("Assign node missing assign_to".into()))?;

        // Update the entity assignment based on type
        match context.entity_type.as_str() {
            "lead" => {
                sqlx::query!(
                    "UPDATE leads SET assigned_to = (SELECT id FROM users WHERE email = $1) WHERE id = $2",
                    assign_to,
                    context.entity_id
                )
                .execute(&self.state.pool)
                .await?;
            },
            "interaction" => {
                sqlx::query!(
                    "UPDATE interactions SET assigned_to = (SELECT id FROM users WHERE email = $1) WHERE id = $2",
                    assign_to,
                    context.entity_id
                )
                .execute(&self.state.pool)
                .await?;
            },
            _ => return Err(AppError::BadRequest(format!("Cannot assign entity type: {}", context.entity_type))),
        }

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ "assigned_to": assign_to })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_notification(
        &self,
        node: &WorkflowNode,
        _context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let message = node.config
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let notification_type = node.config
            .get("type")
            .and_then(|v| v.as_str())
            .unwrap_or("info");

        // In a real implementation, send via WebSocket, push notification, etc.
        tracing::info!("Notification ({}): {}", notification_type, message);

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ 
                "type": notification_type,
                "message": message
            })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn execute_update_status(
        &self,
        node: &WorkflowNode,
        context: &WorkflowContext,
    ) -> Result<NodeExecutionResult, AppError> {
        let new_status = node.config
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::BadRequest("UpdateStatus node missing status".into()))?;

        // Update the entity status based on type
        match context.entity_type.as_str() {
            "lead" => {
                sqlx::query!(
                    "UPDATE leads SET status = $1, updated_at = NOW() WHERE id = $2",
                    new_status,
                    context.entity_id
                )
                .execute(&self.state.pool)
                .await?;
            },
            "interaction" => {
                sqlx::query!(
                    "UPDATE interactions SET status = $1 WHERE id = $2",
                    new_status,
                    context.entity_id
                )
                .execute(&self.state.pool)
                .await?;
            },
            _ => return Err(AppError::BadRequest(format!("Cannot update status for entity type: {}", context.entity_type))),
        }

        Ok(NodeExecutionResult {
            node_id: node.id.clone(),
            node_type: node.node_type.clone(),
            status: ExecutionStatus::Success,
            result: Some(serde_json::json!({ "new_status": new_status })),
            error: None,
            executed_at: Utc::now(),
        })
    }

    async fn get_contact_for_entity(
        &self,
        entity_id: &Uuid,
        entity_type: &str,
    ) -> Result<crate::api::contacts::ContactRow, AppError> {
        match entity_type {
            "lead" => {
                let row = sqlx::query_as!(
                    crate::api::contacts::ContactRow,
                    r#"SELECT c.* FROM contacts c 
                       JOIN leads l ON c.id = l.contact_id 
                       WHERE l.id = $1"#,
                    entity_id
                )
                .fetch_one(&self.state.pool)
                .await?;
                Ok(row)
            },
            "interaction" => {
                let row = sqlx::query_as!(
                    crate::api::contacts::ContactRow,
                    r#"SELECT c.* FROM contacts c 
                       JOIN interactions i ON c.id = i.contact_id 
                       WHERE i.id = $1"#,
                    entity_id
                )
                .fetch_one(&self.state.pool)
                .await?;
                Ok(row)
            },
            _ => Err(AppError::BadRequest(format!("Cannot get contact for entity type: {}", entity_type))),
        }
    }
}
