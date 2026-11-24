#![forbid(unsafe_code)]

//! `agentlens-alerts` crate.
//! Typed alert rule engine, deterministic state machine, and deduplication logic.

use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};
use sqlx::Row;
use thiserror::Error;
use uuid::Uuid;

use agentlens_database::{
    AlertEventsRepository, DatabaseError, NotificationChannelsRepository,
    NotificationDeliveriesRepository,
};
pub use agentlens_domain::{AlertEvent, AlertMetric, AlertOperator, AlertRule};

#[derive(Debug, Error)]
pub enum AlertError {
    #[error("Database error: {0}")]
    Database(#[from] DatabaseError),
    #[error("SQLx error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("Metric evaluation error: {0}")]
    Evaluation(String),
}

/// Computes deterministic SHA-256 fingerprint for deduplication.
/// Format: `SHA-256(rule_id:dimension_key)`
#[must_use]
pub fn compute_alert_fingerprint(rule_id: Uuid, dimension_key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(rule_id.as_bytes());
    hasher.update(b":");
    hasher.update(dimension_key.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone)]
pub struct AlertEvaluationOutcome {
    pub rule_id: Uuid,
    pub metric: AlertMetric,
    pub dimension_key: String,
    pub current_value: Decimal,
    pub threshold: Decimal,
    pub sample_count: i64,
    pub is_breached: bool,
    pub evaluated_at: DateTime<Utc>,
}

pub struct AlertEngine<'a> {
    pool: &'a sqlx::PgPool,
}

impl<'a> AlertEngine<'a> {
    #[must_use]
    pub fn new(pool: &'a sqlx::PgPool) -> Self {
        Self { pool }
    }

    /// Evaluates an alert rule against operational telemetry.
    /// Returns `None` if sample count is below `min_sample_count` (except for inactivity).
    pub async fn evaluate_rule(
        &self,
        rule: &AlertRule,
    ) -> Result<Option<AlertEvaluationOutcome>, AlertError> {
        let (current_value, sample_count, dimension_key) = match rule.metric {
            AlertMetric::AgentErrorRate => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        COUNT(*) FILTER (WHERE status = 'error')::BIGINT as error_count
                    FROM traces
                    WHERE project_id = $1
                      AND ($2::UUID IS NULL OR environment_id = $2)
                      AND started_at >= now() - ($3 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.environment_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let errors: i64 = row.get("error_count");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                let val = if total > 0 {
                    Decimal::from(errors) / Decimal::from(total)
                } else {
                    Decimal::ZERO
                };
                (val, total, "global".to_string())
            }
            AlertMetric::AgentP95LatencyMs => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        percentile_cont(0.95) WITHIN GROUP (ORDER BY duration_ms) as p95
                    FROM traces
                    WHERE project_id = $1
                      AND ($2::UUID IS NULL OR environment_id = $2)
                      AND started_at >= now() - ($3 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.environment_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let p95: Option<f64> = row.get("p95");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                let val = p95
                    .and_then(Decimal::from_f64_retain)
                    .unwrap_or(Decimal::ZERO);
                (val, total, "global".to_string())
            }
            AlertMetric::AgentInactivitySeconds => {
                let row = sqlx::query(
                    r#"
                    SELECT MAX(last_seen_at) as last_seen
                    FROM agents
                    WHERE project_id = $1
                    "#,
                )
                .bind(rule.project_id)
                .fetch_one(self.pool)
                .await?;

                let last_seen: Option<DateTime<Utc>> = row.get("last_seen");
                let elapsed_secs = match last_seen {
                    Some(ls) => (Utc::now() - ls).num_seconds().max(0),
                    None => 86400 * 30, // Default 30 days if never seen
                };
                (Decimal::from(elapsed_secs), 1, "global".to_string())
            }
            AlertMetric::LlmErrorRate => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        COUNT(*) FILTER (WHERE status_code = 'error')::BIGINT as error_count
                    FROM spans
                    WHERE project_id = $1
                      AND span_type = 'llm'
                      AND start_time >= now() - ($2 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let errors: i64 = row.get("error_count");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                let val = if total > 0 {
                    Decimal::from(errors) / Decimal::from(total)
                } else {
                    Decimal::ZERO
                };
                (val, total, "llm".to_string())
            }
            AlertMetric::ToolFailureRate => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        COUNT(*) FILTER (WHERE status_code = 'error')::BIGINT as error_count
                    FROM spans
                    WHERE project_id = $1
                      AND span_type = 'tool'
                      AND start_time >= now() - ($2 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let errors: i64 = row.get("error_count");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                let val = if total > 0 {
                    Decimal::from(errors) / Decimal::from(total)
                } else {
                    Decimal::ZERO
                };
                (val, total, "tool".to_string())
            }
            AlertMetric::TokenUsageTotal => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        COALESCE(SUM(total_tokens), 0)::BIGINT as sum_tokens
                    FROM token_usages
                    WHERE project_id = $1
                      AND recorded_at >= now() - ($2 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let sum_tokens: i64 = row.get("sum_tokens");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                (Decimal::from(sum_tokens), total, "global".to_string())
            }
            AlertMetric::EstimatedCostUsd => {
                let row = sqlx::query(
                    r#"
                    SELECT 
                        COUNT(*)::BIGINT as total_count,
                        COALESCE(SUM(total_cost), 0)::NUMERIC as sum_cost
                    FROM traces
                    WHERE project_id = $1
                      AND ($2::UUID IS NULL OR environment_id = $2)
                      AND started_at >= now() - ($3 || ' seconds')::INTERVAL
                    "#,
                )
                .bind(rule.project_id)
                .bind(rule.environment_id)
                .bind(rule.window_seconds)
                .fetch_one(self.pool)
                .await?;

                let total: i64 = row.get("total_count");
                let sum_cost: Decimal = row.get("sum_cost");
                if total < rule.min_sample_count as i64 {
                    return Ok(None);
                }
                (sum_cost, total, "global".to_string())
            }
        };

        let is_breached = rule.operator.evaluates(current_value, rule.threshold);

        Ok(Some(AlertEvaluationOutcome {
            rule_id: rule.id,
            metric: rule.metric,
            dimension_key,
            current_value,
            threshold: rule.threshold,
            sample_count,
            is_breached,
            evaluated_at: Utc::now(),
        }))
    }

    /// Processes an evaluation outcome according to the state machine:
    /// - Deduplicates active events without firing storms.
    /// - Checks cooldown before re-firing.
    /// - Automatically resolves when metric returns within normal bounds.
    /// - Enqueues notification delivery records for enabled channels.
    pub async fn process_evaluation(
        &self,
        rule: &AlertRule,
        outcome: &AlertEvaluationOutcome,
    ) -> Result<Option<AlertEvent>, AlertError> {
        let events_repo = AlertEventsRepository::new(self.pool);
        let channels_repo = NotificationChannelsRepository::new(self.pool);
        let deliveries_repo = NotificationDeliveriesRepository::new(self.pool);

        let fingerprint = compute_alert_fingerprint(rule.id, &outcome.dimension_key);
        let active_event = events_repo
            .find_active_by_fingerprint(rule.project_id, rule.id, &fingerprint)
            .await?;

        let details = serde_json::json!({
            "metric": rule.metric.as_str(),
            "operator": rule.operator.as_str(),
            "threshold": rule.threshold,
            "current_value": outcome.current_value,
            "sample_count": outcome.sample_count,
            "dimension_key": outcome.dimension_key,
            "window_seconds": rule.window_seconds,
        });

        if outcome.is_breached {
            if let Some(event) = active_event {
                // Already firing or acknowledged: update last_triggered_at and details.
                // Do NOT create duplicate events or spam notifications.
                events_repo.update_triggered(event.id, details).await?;
                return Ok(None);
            }

            // Check cooldown from last resolution to prevent flapping
            if let Some(last_resolved) = events_repo
                .find_last_resolved_by_fingerprint(rule.project_id, rule.id, &fingerprint)
                .await?
            {
                if let Some(resolved_at) = last_resolved.resolved_at {
                    let elapsed = (Utc::now() - resolved_at).num_seconds();
                    if elapsed < rule.cooldown_seconds as i64 {
                        tracing::debug!(
                            rule_id = %rule.id,
                            fingerprint = %fingerprint,
                            elapsed = elapsed,
                            cooldown = rule.cooldown_seconds,
                            "Alert suppressed due to cooldown window"
                        );
                        return Ok(None);
                    }
                }
            }

            // Create new firing alert event
            let summary = format!(
                "Rule '{}' triggered: {} {} {} (current: {})",
                rule.name,
                rule.metric.as_str(),
                rule.operator.as_str(),
                rule.threshold,
                outcome.current_value
            );

            let event = events_repo
                .create(
                    rule.organization_id,
                    rule.project_id,
                    rule.id,
                    &fingerprint,
                    rule.severity,
                    &summary,
                    details,
                )
                .await?;

            // Enqueue notification deliveries for all enabled channels in project
            let channels = channels_repo
                .list_enabled_for_project(rule.project_id)
                .await?;
            if !channels.is_empty() {
                let delivery_pairs: Vec<(Uuid, Uuid)> =
                    channels.into_iter().map(|c| (event.id, c.id)).collect();
                deliveries_repo.create_batch(&delivery_pairs).await?;
            }

            Ok(Some(event))
        } else {
            // Not breached: if an active event exists, transition to resolved
            if let Some(event) = active_event {
                let resolved = events_repo.resolve(event.id).await?;
                if let Some(res_event) = &resolved {
                    // Enqueue resolution notification deliveries
                    let channels = channels_repo
                        .list_enabled_for_project(rule.project_id)
                        .await?;
                    if !channels.is_empty() {
                        let delivery_pairs: Vec<(Uuid, Uuid)> =
                            channels.into_iter().map(|c| (res_event.id, c.id)).collect();
                        deliveries_repo.create_batch(&delivery_pairs).await?;
                    }
                }
                return Ok(resolved);
            }

            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_alert_fingerprint_deterministic() {
        let rule_id = Uuid::new_v4();
        let fp1 = compute_alert_fingerprint(rule_id, "global");
        let fp2 = compute_alert_fingerprint(rule_id, "global");
        let fp3 = compute_alert_fingerprint(rule_id, "agent_123");

        assert_eq!(fp1, fp2);
        assert_ne!(fp1, fp3);
        assert_eq!(fp1.len(), 64);
    }

    #[test]
    fn test_operator_evaluations() {
        let op_gt = AlertOperator::Gt;
        assert!(op_gt.evaluates(Decimal::from(10), Decimal::from(5)));
        assert!(!op_gt.evaluates(Decimal::from(5), Decimal::from(5)));

        let op_gte = AlertOperator::Gte;
        assert!(op_gte.evaluates(Decimal::from(5), Decimal::from(5)));
        assert!(!op_gte.evaluates(Decimal::from(4), Decimal::from(5)));

        let op_lt = AlertOperator::Lt;
        assert!(op_lt.evaluates(Decimal::from(4), Decimal::from(5)));
        assert!(!op_lt.evaluates(Decimal::from(5), Decimal::from(5)));

        let op_lte = AlertOperator::Lte;
        assert!(op_lte.evaluates(Decimal::from(5), Decimal::from(5)));
        assert!(!op_lte.evaluates(Decimal::from(6), Decimal::from(5)));

        let op_eq = AlertOperator::Eq;
        assert!(op_eq.evaluates(Decimal::from(5), Decimal::from(5)));
        assert!(!op_eq.evaluates(Decimal::from(4), Decimal::from(5)));
    }
}
