#![forbid(unsafe_code)]

use agentlens_api::{create_router, AppState};
use agentlens_auth::generate_api_key;
use agentlens_config::AppConfig;
use agentlens_database::{
    create_pool, run_migrations, ApiKeysRepository, DbPool, OrganizationsRepository,
    ProjectsRepository, SpansRepository, TracesRepository,
};
use agentlens_domain::{SpanId, TraceId, TraceStatus};
use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use flate2::write::GzEncoder;
use flate2::Compression;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::tonic::common::v1::any_value::Value as PV;
use opentelemetry_proto::tonic::common::v1::{AnyValue, KeyValue};
use opentelemetry_proto::tonic::trace::v1::status::StatusCode as ProtoStatusCode;
use opentelemetry_proto::tonic::trace::v1::{ResourceSpans, ScopeSpans, Span, Status};
use prost::Message;
use std::env;
use std::io::Write;
use tower::ServiceExt;
use uuid::Uuid;

async fn setup_test_api() -> Option<(DbPool, axum::Router, Uuid, Uuid, Uuid, String)> {
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
    let keys_repo = ApiKeysRepository::new(&pool);

    let org = orgs_repo
        .create("OTLP Test Org", &format!("otlp-org-{}", Uuid::new_v4()))
        .await
        .unwrap();

    let (project, envs) = projects_repo
        .create(
            org.id,
            "OTLP Test Project",
            &format!("otlp-p-{}", Uuid::new_v4()),
            None,
        )
        .await
        .unwrap();
    let env = &envs[0];

    let (secret, prefix, hash) = generate_api_key();
    let _ = keys_repo
        .create(
            org.id,
            project.id,
            env.id,
            "Test Key",
            &prefix,
            &hash,
            serde_json::json!(["telemetry:write"]),
            None,
        )
        .await
        .unwrap();

    let state = AppState {
        db: Some(pool.clone()),
        config,
    };
    let app = create_router(state);

    Some((pool, app, org.id, project.id, env.id, secret))
}

#[tokio::test]
async fn test_otlp_protobuf_and_gzip_ingestion() {
    let Some((pool, app, _org_id, project_id, _env_id, secret)) = setup_test_api().await else {
        return;
    };

    let trace_bytes = hex::decode("1234567890abcdef1234567890abcdef").unwrap();
    let root_span_bytes = hex::decode("1111222233334444").unwrap();
    let child_span_bytes = hex::decode("5555666677778888").unwrap();

    let root_span = Span {
        trace_id: trace_bytes.clone(),
        span_id: root_span_bytes.clone(),
        name: "root_agent_task".to_string(),
        start_time_unix_nano: 1_700_000_000_000_000_000,
        end_time_unix_nano: 1_700_000_005_000_000_000,
        status: Some(Status {
            code: ProtoStatusCode::Ok as i32,
            ..Default::default()
        }),
        attributes: vec![
            KeyValue {
                key: "agent.name".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::StringValue("ResearchAgent".to_string())),
                }),
            },
            KeyValue {
                key: "gen_ai.prompt".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::StringValue("Secret prompt text".to_string())),
                }),
            },
        ],
        ..Default::default()
    };

    let child_span = Span {
        trace_id: trace_bytes.clone(),
        span_id: child_span_bytes.clone(),
        parent_span_id: root_span_bytes.clone(),
        name: "llm_completion".to_string(),
        start_time_unix_nano: 1_700_000_001_000_000_000,
        end_time_unix_nano: 1_700_000_003_000_000_000,
        attributes: vec![
            KeyValue {
                key: "gen_ai.system".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::StringValue("openai".to_string())),
                }),
            },
            KeyValue {
                key: "gen_ai.request.model".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::StringValue("gpt-4o".to_string())),
                }),
            },
            KeyValue {
                key: "gen_ai.usage.input_tokens".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::IntValue(500)),
                }),
            },
            KeyValue {
                key: "gen_ai.usage.output_tokens".to_string(),
                value: Some(AnyValue {
                    value: Some(PV::IntValue(150)),
                }),
            },
        ],
        ..Default::default()
    };

    let otlp_req = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: vec![root_span, child_span],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };

    let mut proto_bytes = Vec::new();
    otlp_req.encode(&mut proto_bytes).unwrap();

    // Gzip compress
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&proto_bytes).unwrap();
    let gzip_payload = encoder.finish().unwrap();

    let req = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .header(header::CONTENT_ENCODING, "gzip")
        .body(Body::from(gzip_payload))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    let status = res.status();
    let body_bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "Failed with response: {}",
        String::from_utf8_lossy(&body_bytes)
    );

    // Verify DB persistence
    let trace_id = TraceId::new("1234567890abcdef1234567890abcdef").unwrap();
    let traces_repo = TracesRepository::new(&pool);
    let trace = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .expect("Trace must exist");

    assert_eq!(trace.name.as_deref(), Some("root_agent_task"));
    assert_eq!(trace.status, TraceStatus::Ok);

    let spans_repo = SpansRepository::new(&pool);
    let span_count = spans_repo
        .count_spans_for_trace(project_id, &trace_id)
        .await
        .unwrap();
    assert_eq!(span_count, 2);

    // Verify prompt was stripped due to capture_content_default = false
    let spans = spans_repo
        .list_for_trace(project_id, &trace_id)
        .await
        .unwrap();
    let root = spans.iter().find(|s| s.parent_span_id.is_none()).unwrap();
    assert_eq!(
        root.attributes["gen_ai.prompt"],
        "[CONTENT_CAPTURE_DISABLED]"
    );

    // Verify Outbox job was created
    let job_row = sqlx::query("SELECT COUNT(*) as count FROM outbox_jobs WHERE project_id = $1 AND job_type = 'trace_ingested'")
        .bind(project_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let count: i64 = sqlx::Row::get(&job_row, "count");
    assert!(count >= 1, "Transactional outbox job must be committed");
}

#[tokio::test]
async fn test_malformed_protobuf_and_oversized_payload() {
    let Some((_pool, app, _org_id, _project_id, _env_id, secret)) = setup_test_api().await else {
        return;
    };

    // 1. Malformed Protobuf returns 400
    let bad_req = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(Body::from(vec![0xFF, 0xFF, 0x00, 0x12]))
        .unwrap();

    let res = app.clone().oneshot(bad_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);

    // 2. Gzip bomb (> 16 MiB decompressed) returns 413
    let huge_data = vec![b'A'; 17 * 1024 * 1024]; // 17 MiB of repeating bytes
    let mut encoder = GzEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&huge_data).unwrap();
    let compressed_huge = encoder.finish().unwrap();

    let bomb_req = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .header(header::CONTENT_ENCODING, "gzip")
        .body(Body::from(compressed_huge))
        .unwrap();

    let res = app.oneshot(bomb_req).await.unwrap();
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn test_out_of_order_span_batches() {
    let Some((pool, app, _org_id, project_id, _env_id, secret)) = setup_test_api().await else {
        return;
    };

    let trace_bytes = hex::decode("fedcba9876543210fedcba9876543210").unwrap();
    let root_span_bytes = hex::decode("a1b2c3d4e5f60718").unwrap();
    let child_span_bytes = hex::decode("b2c3d4e5f6a1b2c3").unwrap();

    // Batch 1: Child span arrives first
    let child_span = Span {
        trace_id: trace_bytes.clone(),
        span_id: child_span_bytes.clone(),
        parent_span_id: root_span_bytes.clone(),
        name: "child_tool".to_string(),
        start_time_unix_nano: 1_700_000_002_000_000_000,
        end_time_unix_nano: 1_700_000_004_000_000_000,
        ..Default::default()
    };

    let req1 = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: vec![child_span],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };
    let mut bytes1 = Vec::new();
    req1.encode(&mut bytes1).unwrap();

    let http_req1 = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(Body::from(bytes1))
        .unwrap();

    let res1 = app.clone().oneshot(http_req1).await.unwrap();
    let status1 = res1.status();
    let body1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status1,
        StatusCode::OK,
        "Failed res1: {}",
        String::from_utf8_lossy(&body1)
    );

    // Verify provisional state
    let trace_id = TraceId::new("fedcba9876543210fedcba9876543210").unwrap();
    let traces_repo = TracesRepository::new(&pool);
    let prov_trace = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .expect("Provisional trace exists");
    assert_eq!(prov_trace.status, TraceStatus::Unset);
    assert!(prov_trace.root_span_id.is_none());

    // Batch 2: Root span arrives second
    let root_span = Span {
        trace_id: trace_bytes.clone(),
        span_id: root_span_bytes.clone(),
        name: "orchestrator_root".to_string(),
        start_time_unix_nano: 1_700_000_000_000_000_000,
        end_time_unix_nano: 1_700_000_005_000_000_000,
        status: Some(Status {
            code: ProtoStatusCode::Ok as i32,
            ..Default::default()
        }),
        ..Default::default()
    };

    let req2 = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: vec![root_span],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };
    let mut bytes2 = Vec::new();
    req2.encode(&mut bytes2).unwrap();

    let http_req2 = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(Body::from(bytes2))
        .unwrap();

    let res2 = app.oneshot(http_req2).await.unwrap();
    let status2 = res2.status();
    let body2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status2,
        StatusCode::OK,
        "Failed res2: {}",
        String::from_utf8_lossy(&body2)
    );

    // Authoritative trace verified
    let final_trace = traces_repo
        .get_by_trace_id(project_id, &trace_id)
        .await
        .unwrap()
        .expect("Final trace exists");
    assert_eq!(final_trace.name.as_deref(), Some("orchestrator_root"));
    assert_eq!(final_trace.status, TraceStatus::Ok);
    let expected_root_sid = SpanId::new("a1b2c3d4e5f60718").unwrap();
    assert_eq!(final_trace.root_span_id.as_ref(), Some(&expected_root_sid));
}

#[tokio::test]
async fn test_duplicate_batch_idempotency() {
    let Some((pool, app, _org_id, project_id, _env_id, secret)) = setup_test_api().await else {
        return;
    };

    let trace_bytes = hex::decode("abcdef0123456789abcdef0123456789").unwrap();
    let span_bytes = hex::decode("9876543210fedcba").unwrap();

    let span = Span {
        trace_id: trace_bytes.clone(),
        span_id: span_bytes.clone(),
        name: "idempotent_span".to_string(),
        start_time_unix_nano: 1_700_000_000_000_000_000,
        end_time_unix_nano: 1_700_000_001_000_000_000,
        ..Default::default()
    };

    let req = ExportTraceServiceRequest {
        resource_spans: vec![ResourceSpans {
            scope_spans: vec![ScopeSpans {
                spans: vec![span],
                ..Default::default()
            }],
            ..Default::default()
        }],
    };
    let mut bytes = Vec::new();
    req.encode(&mut bytes).unwrap();

    // Ingest first time
    let http_req1 = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(Body::from(bytes.clone()))
        .unwrap();
    let res1 = app.clone().oneshot(http_req1).await.unwrap();
    let status1 = res1.status();
    let body1 = axum::body::to_bytes(res1.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status1,
        StatusCode::OK,
        "Failed res1: {}",
        String::from_utf8_lossy(&body1)
    );

    // Ingest exact duplicate second time
    let http_req2 = Request::builder()
        .method("POST")
        .uri("/v1/traces")
        .header(header::AUTHORIZATION, format!("Bearer {secret}"))
        .header(header::CONTENT_TYPE, "application/x-protobuf")
        .body(Body::from(bytes))
        .unwrap();
    let res2 = app.oneshot(http_req2).await.unwrap();
    let status2 = res2.status();
    let body2 = axum::body::to_bytes(res2.into_body(), usize::MAX)
        .await
        .unwrap();
    assert_eq!(
        status2,
        StatusCode::OK,
        "Failed res2: {}",
        String::from_utf8_lossy(&body2)
    );

    let trace_id = TraceId::new("abcdef0123456789abcdef0123456789").unwrap();
    let spans_repo = SpansRepository::new(&pool);
    let span_count = spans_repo
        .count_spans_for_trace(project_id, &trace_id)
        .await
        .unwrap();
    assert_eq!(
        span_count, 1,
        "Duplicate batch retry must not produce duplicate spans"
    );
}
