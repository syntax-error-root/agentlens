#![forbid(unsafe_code)]

use agentlens_config::AppConfig;
use agentlens_database::{
    create_pool, run_migrations, DbPool, OrganizationsRepository, OutboxRepository,
    ProjectsRepository, TracesRepository,
};
use agentlens_domain::{SpanId, SpanType, TraceId, TraceStatus};
use agentlens_jobs::{OutboxWorker, WorkerConfig};
use agentlens_observability::ObservabilityRegistry;
use agentlens_telemetry::{NormalizedSpan, NormalizedUsage};
use chrono::{Duration as ChronoDuration, Utc};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

async fn setup_test_db() -> Option<(DbPool, Uuid, Uuid, Uuid)> {
    if env::var("DATABASE_URL").is_err() {
        return None;
    }

    let config = AppConfig::default();
    let pool = create_pool(&config).await.ok()?;
    run_migrations(&pool)
        .await
        .expect("Migrations must succeed");

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);

    let org = orgs_repo
        .create("Worker Test Org", &format!("worker-org-{}", Uuid::new_v4()))
        .await
        .unwrap();

    let (project, envs) = projects_repo
        .create(
            org.id,
            "Worker Test Project",
            &format!("worker-p-{}", Uuid::new_v4()),
            None,
        )
        .await
        .unwrap();
    let env = &envs[0];

    Some((pool, org.id, project.id, env.id))
}

#[tokio::test]
async fn test_worker_normal_finalization() {
    let Some((pool, org_id, project_id, env_id)) = setup_test_db().await else {
        return;
    };

    let registry = Arc::new(ObservabilityRegistry::new());
    let worker_config = WorkerConfig {
        worker_id: format!("test-worker-{}", Uuid::new_v4()),
        batch_size: 10,
        lease_duration_seconds: 30,
        poll_interval: Duration::from_millis(50),
        project_id: Some(project_id),
    };
    let worker = OutboxWorker::new(pool.clone(), registry.clone(), worker_config);

    let trace_id = TraceId::new("10000000000000000000000000000001").unwrap();
    let root_sid = SpanId::new("1000000000000001").unwrap();
    let child_sid = SpanId::new("1000000000000002").unwrap();

    let t0 = Utc::now() - ChronoDuration::seconds(10);
    let t1 = t0 + ChronoDuration::seconds(2);
    let t2 = t0 + ChronoDuration::seconds(5);

    let root_span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: root_sid.clone(),
        parent_span_id: None,
        name: "orchestrator_agent".to_string(),
        span_type: SpanType::Agent,
        status: TraceStatus::Ok,
        started_at: t0,
        ended_at: Some(t2),
        duration_ms: Some(5000),
        provider: None,
        model: None,
        tool_name: None,
        agent_name: Some("PlannerAgent".to_string()),
        agent_version: Some("1.0.0".to_string()),
        session_id: Some("session-123".to_string()),
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: None,
    };

    let child_span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: child_sid.clone(),
        parent_span_id: Some(root_sid.clone()),
        name: "llm_generate".to_string(),
        span_type: SpanType::Llm,
        status: TraceStatus::Ok,
        started_at: t0 + ChronoDuration::seconds(1),
        ended_at: Some(t1),
        duration_ms: Some(1000),
        provider: Some("anthropic".to_string()),
        model: Some("claude-3-5-sonnet".to_string()),
        tool_name: None,
        agent_name: None,
        agent_version: None,
        session_id: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: Some(NormalizedUsage {
            provider: "anthropic".to_string(),
            model: "claude-3-5-sonnet".to_string(),
            input_tokens: 300,
            output_tokens: 150,
            cached_input_tokens: 50,
            reasoning_tokens: 20,
        }),
    };

    let traces_repo = TracesRepository::new(&pool);
    let affected = traces_repo
        .persist_batch(org_id, project_id, env_id, &[root_span, child_span])
        .await
        .unwrap();
    assert_eq!(affected.len(), 1);

    // Process job with worker
    let processed_count = worker.run_once().await.unwrap();
    assert_eq!(processed_count, 1);
    assert_eq!(registry.get_worker_jobs_processed(), 1);

    // Verify outbox job status is completed
    let _outbox_repo = OutboxRepository::new(&pool);
    let job_row = sqlx::query(
        "SELECT status, completed_at FROM outbox_jobs WHERE project_id = $1 AND job_type = 'trace_ingested'",
    )
    .bind(project_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let job_status: String = sqlx::Row::get(&job_row, "status");
    assert_eq!(job_status, "completed");

    // Verify trace finalization fields
    let trace = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .expect("Trace must exist");

    assert_eq!(trace.span_count, 2);
    assert_eq!(trace.error_span_count, 0);
    assert_eq!(trace.status, TraceStatus::Ok);
    assert_eq!(trace.name.as_deref(), Some("orchestrator_agent"));
    assert_eq!(trace.root_span_id.as_ref(), Some(&root_sid));
    assert_eq!(trace.total_input_tokens, 300);
    assert_eq!(trace.total_output_tokens, 150);
    assert_eq!(trace.total_cached_input_tokens, 50);
    assert_eq!(trace.total_reasoning_tokens, 20);
    assert!(trace.finalized_at.is_some());

    // Wall-clock duration must match t2 - t0 (5000ms), NOT 5000 + 1000
    assert_eq!(trace.duration_ms, Some(5000));
}

#[tokio::test]
async fn test_lease_expiration_recovery_after_simulated_crash() {
    let Some((pool, org_id, project_id, env_id)) = setup_test_db().await else {
        return;
    };

    let trace_id = TraceId::new("20000000000000000000000000000002").unwrap();
    let root_sid = SpanId::new("2000000000000001").unwrap();

    let root_span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: root_sid.clone(),
        parent_span_id: None,
        name: "resilient_task".to_string(),
        span_type: SpanType::Agent,
        status: TraceStatus::Ok,
        started_at: Utc::now() - ChronoDuration::seconds(5),
        ended_at: Some(Utc::now()),
        duration_ms: Some(5000),
        provider: None,
        model: None,
        tool_name: None,
        agent_name: None,
        agent_version: None,
        session_id: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: None,
    };

    let traces_repo = TracesRepository::new(&pool);
    traces_repo
        .persist_batch(org_id, project_id, env_id, &[root_span])
        .await
        .unwrap();

    // Simulate worker-1 crash: lock job with lease_expires_at in the past
    sqlx::query(
        r#"
        UPDATE outbox_jobs
        SET status = 'processing',
            locked_by = 'crashed-worker-instance',
            locked_at = now() - interval '60 seconds',
            lease_expires_at = now() - interval '10 seconds'
        WHERE project_id = $1 AND job_type = 'trace_ingested'
        "#,
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();

    // Start worker-2
    let registry = Arc::new(ObservabilityRegistry::new());
    let worker2 = OutboxWorker::new(
        pool.clone(),
        registry.clone(),
        WorkerConfig {
            worker_id: "worker-instance-2".to_string(),
            batch_size: 10,
            lease_duration_seconds: 30,
            poll_interval: Duration::from_millis(50),
            project_id: Some(project_id),
        },
    );

    // Worker 2 should reclaim the expired lease and finish the job
    let reclaimed = worker2.run_once().await.unwrap();
    assert_eq!(reclaimed, 1);
    assert_eq!(registry.get_worker_jobs_processed(), 1);

    // Verify job transitioned to completed
    let row = sqlx::query("SELECT status, attempts FROM outbox_jobs WHERE project_id = $1")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let status: String = sqlx::Row::get(&row, "status");
    let attempts: i32 = sqlx::Row::get(&row, "attempts");
    assert_eq!(status, "completed");
    assert_eq!(attempts, 1); // incremented when claimed by worker-2
}

#[tokio::test]
async fn test_duplicate_job_idempotency() {
    let Some((pool, org_id, project_id, env_id)) = setup_test_db().await else {
        return;
    };

    let trace_id = TraceId::new("30000000000000000000000000000003").unwrap();
    let sid = SpanId::new("3000000000000001").unwrap();

    let span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: sid.clone(),
        parent_span_id: None,
        name: "idempotent_task".to_string(),
        span_type: SpanType::Agent,
        status: TraceStatus::Ok,
        started_at: Utc::now() - ChronoDuration::seconds(10),
        ended_at: Some(Utc::now() - ChronoDuration::seconds(2)),
        duration_ms: Some(8000),
        provider: None,
        model: None,
        tool_name: None,
        agent_name: None,
        agent_version: None,
        session_id: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: None,
    };

    let traces_repo = TracesRepository::new(&pool);
    traces_repo
        .persist_batch(org_id, project_id, env_id, &[span])
        .await
        .unwrap();

    let registry = Arc::new(ObservabilityRegistry::new());
    let worker = OutboxWorker::new(
        pool.clone(),
        registry,
        WorkerConfig {
            project_id: Some(project_id),
            ..WorkerConfig::default()
        },
    );

    // Pass 1
    worker.run_once().await.unwrap();
    let trace1 = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .unwrap();

    // Replay job 2nd time
    sqlx::query(
        "UPDATE outbox_jobs SET status = 'pending', available_at = now() WHERE project_id = $1",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    let trace2 = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .unwrap();

    // Replay job 3rd time
    sqlx::query(
        "UPDATE outbox_jobs SET status = 'pending', available_at = now() WHERE project_id = $1",
    )
    .bind(project_id)
    .execute(&pool)
    .await
    .unwrap();
    worker.run_once().await.unwrap();
    let trace3 = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(trace1.span_count, trace2.span_count);
    assert_eq!(trace2.span_count, trace3.span_count);
    assert_eq!(trace1.duration_ms, trace2.duration_ms);
    assert_eq!(trace2.duration_ms, trace3.duration_ms);
    assert_eq!(trace1.status, trace3.status);
}

#[tokio::test]
async fn test_late_arriving_span_reconciliation() {
    let Some((pool, org_id, project_id, env_id)) = setup_test_db().await else {
        return;
    };

    let trace_id = TraceId::new("40000000000000000000000000000004").unwrap();
    let root_sid = SpanId::new("4000000000000001").unwrap();
    let late_sid = SpanId::new("4000000000000002").unwrap();

    let t0 = Utc::now() - ChronoDuration::seconds(20);
    let t1 = t0 + ChronoDuration::seconds(5);
    let t2 = t0 + ChronoDuration::seconds(15); // Late arriving extends duration to 15s

    let root_span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: root_sid.clone(),
        parent_span_id: None,
        name: "root_task".to_string(),
        span_type: SpanType::Agent,
        status: TraceStatus::Ok,
        started_at: t0,
        ended_at: Some(t1),
        duration_ms: Some(5000),
        provider: None,
        model: None,
        tool_name: None,
        agent_name: None,
        agent_version: None,
        session_id: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: Some(NormalizedUsage {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            input_tokens: 100,
            output_tokens: 50,
            cached_input_tokens: 0,
            reasoning_tokens: 0,
        }),
    };

    let traces_repo = TracesRepository::new(&pool);
    traces_repo
        .persist_batch(org_id, project_id, env_id, &[root_span])
        .await
        .unwrap();

    let registry = Arc::new(ObservabilityRegistry::new());
    let worker = OutboxWorker::new(
        pool.clone(),
        registry,
        WorkerConfig {
            project_id: Some(project_id),
            ..WorkerConfig::default()
        },
    );

    // Finalize first batch
    worker.run_once().await.unwrap();

    let trace_v1 = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(trace_v1.span_count, 1);
    assert_eq!(trace_v1.duration_ms, Some(5000));
    assert_eq!(trace_v1.total_input_tokens, 100);

    // Now ingest late-arriving span in second batch
    let late_span = NormalizedSpan {
        trace_id: trace_id.clone(),
        span_id: late_sid.clone(),
        parent_span_id: Some(root_sid.clone()),
        name: "late_tool_call".to_string(),
        span_type: SpanType::Tool,
        status: TraceStatus::Ok,
        started_at: t1,
        ended_at: Some(t2),
        duration_ms: Some(10000),
        provider: None,
        model: None,
        tool_name: Some("fetch_data".to_string()),
        agent_name: None,
        agent_version: None,
        session_id: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        error: None,
        events: vec![],
        links: vec![],
        usage: Some(NormalizedUsage {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            input_tokens: 250,
            output_tokens: 100,
            cached_input_tokens: 0,
            reasoning_tokens: 0,
        }),
    };

    traces_repo
        .persist_batch(org_id, project_id, env_id, &[late_span])
        .await
        .unwrap();

    // Process second batch job
    worker.run_once().await.unwrap();

    let trace_v2 = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(trace_v2.span_count, 2);
    assert_eq!(trace_v2.duration_ms, Some(15000));
    assert_eq!(trace_v2.total_input_tokens, 350);
    assert_eq!(trace_v2.total_output_tokens, 150);
}

#[tokio::test]
async fn test_dead_letter_transition_on_exhausted_attempts() {
    let Some((pool, org_id, project_id, _env_id)) = setup_test_db().await else {
        return;
    };

    let registry = Arc::new(ObservabilityRegistry::new());
    let worker = OutboxWorker::new(
        pool.clone(),
        registry.clone(),
        WorkerConfig {
            project_id: Some(project_id),
            ..WorkerConfig::default()
        },
    );

    // Insert job referencing non-existent trace with attempts = 2, max_attempts = 3
    let job_id = Uuid::new_v4();
    let bogus_payload = serde_json::json!({
        "organization_id": org_id,
        "project_id": project_id,
        "trace_id": "99999999999999999999999999999999"
    });

    sqlx::query(
        r#"
        INSERT INTO outbox_jobs (
            id, organization_id, project_id, job_type, dedup_key, payload,
            status, attempts, max_attempts, available_at
        ) VALUES ($1, $2, $3, 'trace_ingested', $4, $5, 'pending', 2, 3, now())
        "#,
    )
    .bind(job_id)
    .bind(org_id)
    .bind(project_id)
    .bind(format!("bogus:{job_id}"))
    .bind(bogus_payload)
    .execute(&pool)
    .await
    .unwrap();

    let outbox_repo = OutboxRepository::new(&pool);
    assert_eq!(
        outbox_repo
            .get_pending_count(Some(project_id))
            .await
            .unwrap(),
        1
    );

    // Worker claims this job (attempts becomes 3 == max_attempts), handler fails (trace not found)
    let processed = worker.run_once().await.unwrap();
    assert_eq!(processed, 1);

    // Should transition to 'dead'
    let row = sqlx::query("SELECT status, last_error FROM outbox_jobs WHERE id = $1")
        .bind(job_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    let status: String = sqlx::Row::get(&row, "status");
    let last_error: Option<String> = sqlx::Row::get(&row, "last_error");

    assert_eq!(status, "dead");
    assert!(last_error.is_some());
    assert_eq!(registry.get_worker_jobs_dead(), 1);
}
