use agentlens_config::AppConfig;
use agentlens_database::{
    create_pool, run_migrations, ApiKeysRepository, EnvironmentsRepository,
    OrganizationMembersRepository, OrganizationsRepository, ProjectsRepository, SpansRepository,
    TokenUsagesRepository, TracesRepository, UsersRepository,
};
use agentlens_domain::{
    OrgRole, Span, SpanId, SpanType, TokenUsage, TraceId, TraceStatus, UsageSource,
};
use chrono::{Duration, Utc};
use rust_decimal::Decimal;
use std::env;
use uuid::Uuid;

async fn setup_test_db() -> Option<agentlens_database::DbPool> {
    if env::var("DATABASE_URL").is_err() {
        // Skip if DATABASE_URL is not set (e.g. in environments without DB)
        return None;
    }

    let config = AppConfig::default();
    let pool = create_pool(&config).await.ok()?;
    run_migrations(&pool)
        .await
        .expect("Migrations must succeed");
    Some(pool)
}

#[tokio::test]
async fn test_migrations_and_project_auto_environments() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);
    let envs_repo = EnvironmentsRepository::new(&pool);

    let slug = format!("test-org-{}", Uuid::new_v4());
    let org = orgs_repo
        .create("Test Organization", &slug)
        .await
        .expect("Failed to create org");

    let p_slug = format!("test-project-{}", Uuid::new_v4());
    let (project, envs) = projects_repo
        .create(
            org.id,
            "Test Project",
            &p_slug,
            Some("Integration test project"),
        )
        .await
        .expect("Failed to create project");

    assert_eq!(envs.len(), 3);
    let env_names: Vec<&str> = envs.iter().map(|e| e.name.as_str()).collect();
    assert!(env_names.contains(&"production"));
    assert!(env_names.contains(&"staging"));
    assert!(env_names.contains(&"development"));

    let listed_envs = envs_repo
        .list_for_project(project.id)
        .await
        .expect("Failed to list envs");
    assert_eq!(listed_envs.len(), 3);
}

#[tokio::test]
async fn test_out_of_order_span_and_root_reconciliation() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);
    let traces_repo = TracesRepository::new(&pool);
    let spans_repo = SpansRepository::new(&pool);

    let slug = format!("test-org-{}", Uuid::new_v4());
    let org = orgs_repo.create("Out-of-Order Org", &slug).await.unwrap();

    let p_slug = format!("test-p-{}", Uuid::new_v4());
    let (project, envs) = projects_repo
        .create(org.id, "OOO Project", &p_slug, None)
        .await
        .unwrap();
    let env = &envs[0];

    let trace_id = TraceId::new("1234567890abcdef1234567890abcdef").unwrap();
    let child_span_id = SpanId::new("abcdef1234567890").unwrap();
    let root_span_id = SpanId::new("0987654321fedcba").unwrap();

    let t1 = Utc::now() + Duration::seconds(2);
    let t2 = Utc::now() + Duration::seconds(5);

    // 1. Child span arrives first -> creates provisional trace row
    traces_repo
        .upsert_provisional(org.id, project.id, env.id, &trace_id, t1, Some(t2))
        .await
        .expect("Provisional upsert should succeed");

    let prov_trace = traces_repo
        .get_by_trace_id(project.id, &trace_id)
        .await
        .unwrap()
        .expect("Trace must exist");

    assert_eq!(prov_trace.status, TraceStatus::Unset);
    assert!(prov_trace.root_span_id.is_none());

    // 2. Insert child span
    let child_span = Span {
        id: Uuid::new_v4(),
        organization_id: org.id,
        project_id: project.id,
        trace_id: trace_id.clone(),
        span_id: child_span_id.clone(),
        parent_span_id: Some(root_span_id.clone()),
        agent_id: None,
        name: "tool_execution".to_string(),
        span_type: SpanType::Tool,
        status: TraceStatus::Ok,
        started_at: t1,
        ended_at: Some(t2),
        duration_ms: Some(3000),
        provider: None,
        model: None,
        tool_name: Some("calculator".to_string()),
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        payload_s3_key: None,
        error: None,
        received_at: Utc::now(),
        created_at: Utc::now(),
    };
    spans_repo.insert_span(&child_span).await.unwrap();

    // 3. Root span arrives later -> reconciles authoritative metadata
    let t0 = Utc::now();
    traces_repo
        .upsert_provisional(org.id, project.id, env.id, &trace_id, t0, Some(t2))
        .await
        .unwrap();

    traces_repo
        .reconcile_root_span(
            project.id,
            &trace_id,
            &root_span_id,
            "root_agent_task",
            TraceStatus::Ok,
            None,
        )
        .await
        .unwrap();

    let final_trace = traces_repo
        .get_by_trace_id(project.id, &trace_id)
        .await
        .unwrap()
        .expect("Trace must exist");

    assert_eq!(final_trace.name.as_deref(), Some("root_agent_task"));
    assert_eq!(final_trace.status, TraceStatus::Ok);
    assert_eq!(final_trace.root_span_id.as_ref(), Some(&root_span_id));
    assert!(final_trace.started_at <= t1);
}

#[tokio::test]
async fn test_idempotent_duplicate_upserts() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);
    let traces_repo = TracesRepository::new(&pool);
    let spans_repo = SpansRepository::new(&pool);
    let usages_repo = TokenUsagesRepository::new(&pool);

    let slug = format!("test-org-{}", Uuid::new_v4());
    let org = orgs_repo.create("Dedup Org", &slug).await.unwrap();

    let p_slug = format!("test-p-{}", Uuid::new_v4());
    let (project, envs) = projects_repo
        .create(org.id, "Dedup Project", &p_slug, None)
        .await
        .unwrap();
    let env = &envs[0];

    let trace_id = TraceId::new("fedcba0987654321fedcba0987654321").unwrap();
    let span_id = SpanId::new("1122334455667788").unwrap();

    traces_repo
        .upsert_provisional(org.id, project.id, env.id, &trace_id, Utc::now(), None)
        .await
        .unwrap();

    let span = Span {
        id: Uuid::new_v4(),
        organization_id: org.id,
        project_id: project.id,
        trace_id: trace_id.clone(),
        span_id: span_id.clone(),
        parent_span_id: None,
        agent_id: None,
        name: "llm_call".to_string(),
        span_type: SpanType::Llm,
        status: TraceStatus::Ok,
        started_at: Utc::now(),
        ended_at: Some(Utc::now()),
        duration_ms: Some(150),
        provider: Some("openai".to_string()),
        model: Some("gpt-4o".to_string()),
        tool_name: None,
        attributes: serde_json::json!({}),
        resource_attributes: serde_json::json!({}),
        scope_attributes: serde_json::json!({}),
        payload_s3_key: None,
        error: None,
        received_at: Utc::now(),
        created_at: Utc::now(),
    };

    // Insert span twice
    spans_repo.insert_span(&span).await.unwrap();
    spans_repo.insert_span(&span).await.unwrap();

    let count = spans_repo
        .count_spans_for_trace(project.id, &trace_id)
        .await
        .unwrap();
    assert_eq!(count, 1, "Duplicate span upsert must be idempotent");

    let usage = TokenUsage {
        id: Uuid::new_v4(),
        organization_id: org.id,
        project_id: project.id,
        trace_id: trace_id.clone(),
        span_id: span_id.clone(),
        provider: "openai".to_string(),
        model: "gpt-4o".to_string(),
        input_tokens: 100,
        output_tokens: 50,
        cached_input_tokens: 0,
        reasoning_tokens: 0,
        cost_estimate: Some(Decimal::new(15, 4)),
        currency: "USD".to_string(),
        pricing_version: Some("v1".to_string()),
        input_rate_per_million: None,
        output_rate_per_million: None,
        cached_rate_per_million: None,
        usage_source: UsageSource::ProviderReported,
        recorded_at: Utc::now(),
        created_at: Utc::now(),
    };

    // Insert usage twice
    usages_repo.upsert_usage(&usage).await.unwrap();
    usages_repo.upsert_usage(&usage).await.unwrap();
}

#[tokio::test]
async fn test_cross_tenant_constraint_rejection() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);
    let traces_repo = TracesRepository::new(&pool);

    let org_a = orgs_repo
        .create("Org A", &format!("org-a-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let (project_a, _) = projects_repo
        .create(org_a.id, "Proj A", &format!("p-a-{}", Uuid::new_v4()), None)
        .await
        .unwrap();

    let org_b = orgs_repo
        .create("Org B", &format!("org-b-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let (_project_b, envs_b) = projects_repo
        .create(org_b.id, "Proj B", &format!("p-b-{}", Uuid::new_v4()), None)
        .await
        .unwrap();

    let trace_id = TraceId::new("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa").unwrap();

    // Attempt to insert a trace in Project A using Project B's environment_id!
    // The composite foreign key FOREIGN KEY (project_id, environment_id) REFERENCES environments(project_id, id) MUST REJECT THIS!
    let bad_result = traces_repo
        .upsert_provisional(
            org_a.id,
            project_a.id,
            envs_b[0].id,
            &trace_id,
            Utc::now(),
            None,
        )
        .await;

    assert!(
        bad_result.is_err(),
        "Cross-project environment reference must be rejected by PostgreSQL foreign key constraint"
    );
}

#[tokio::test]
async fn test_api_key_lifecycle_and_verification() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let projects_repo = ProjectsRepository::new(&pool);
    let keys_repo = ApiKeysRepository::new(&pool);

    let org = orgs_repo
        .create("Key Org", &format!("key-org-{}", Uuid::new_v4()))
        .await
        .unwrap();
    let (project, envs) = projects_repo
        .create(
            org.id,
            "Key Project",
            &format!("kp-{}", Uuid::new_v4()),
            None,
        )
        .await
        .unwrap();
    let env = &envs[0];

    // 1. Generate key
    let (secret, prefix, hash) = agentlens_auth::generate_api_key();
    assert!(secret.starts_with("agl_live_"));

    // 2. Insert into DB
    let key = keys_repo
        .create(
            org.id,
            project.id,
            env.id,
            "Production Ingestion Key",
            &prefix,
            &hash,
            serde_json::json!(["telemetry:write"]),
            None,
        )
        .await
        .expect("Create key");

    // 3. Find by prefix
    let found_keys = keys_repo.find_by_prefix(&prefix).await.unwrap();
    assert_eq!(found_keys.len(), 1);
    assert_eq!(found_keys[0].id, key.id);

    // 4. Verify in constant time
    assert!(agentlens_auth::verify_api_key_hash(
        &found_keys[0].key_hash,
        &secret
    ));
    assert!(!agentlens_auth::verify_api_key_hash(
        &found_keys[0].key_hash,
        "agl_live_wrongsecret"
    ));

    // 5. Revoke key
    keys_repo.revoke(key.id).await.unwrap();
    let active_after_revoke = keys_repo.find_by_prefix(&prefix).await.unwrap();
    assert!(
        active_after_revoke.is_empty(),
        "Revoked keys must be filtered out"
    );
}

#[tokio::test]
async fn test_organization_members_rbac_and_isolation() {
    let Some(pool) = setup_test_db().await else {
        return;
    };

    let orgs_repo = OrganizationsRepository::new(&pool);
    let users_repo = UsersRepository::new(&pool);
    let members_repo = OrganizationMembersRepository::new(&pool);

    let org = orgs_repo
        .create("RBAC Org", &format!("rbac-org-{}", Uuid::new_v4()))
        .await
        .unwrap();

    let user = users_repo
        .create(
            &format!("user-{}@example.com", Uuid::new_v4()),
            Some("fake_pw_hash"),
            Some("RBAC Tester"),
        )
        .await
        .unwrap();

    // 1. Add as Viewer
    let member = members_repo
        .add(org.id, user.id, OrgRole::Viewer)
        .await
        .unwrap();
    assert_eq!(member.role, OrgRole::Viewer);

    let role = members_repo.get_role(org.id, user.id).await.unwrap();
    assert_eq!(role, Some(OrgRole::Viewer));

    // 2. Promote to Admin
    members_repo
        .update_role(org.id, user.id, OrgRole::Admin)
        .await
        .unwrap();
    let role = members_repo.get_role(org.id, user.id).await.unwrap();
    assert_eq!(role, Some(OrgRole::Admin));

    // 3. Remove member
    members_repo.remove(org.id, user.id).await.unwrap();
    let role = members_repo.get_role(org.id, user.id).await.unwrap();
    assert_eq!(role, None);
}
