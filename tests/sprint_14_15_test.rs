use std::sync::Arc;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use job_hunter_rs::automation;
use job_hunter_rs::config::AppConfig;
use job_hunter_rs::db::Database;
use job_hunter_rs::execution::{ExecutionEngine, ExecutionType};
use job_hunter_rs::web::{build_router, WebState};

#[tokio::test]
async fn test_sprint_14_executions_summary_and_stats() {
    let temp_db_path = format!("sqlite://jobs_test_s14_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");
    let engine = ExecutionEngine::new(db.clone());

    // 1. Cria execução concluída
    let h1 = engine
        .start_execution(ExecutionType::JobSearch, "TEST_S14", Some(json!({"query": "Engenheiro"})))
        .await
        .expect("Erro ao criar h1");
    h1.finish_success(5, 0, Some("Concluído 5")).await.expect("Erro");

    // 2. Cria execução com falha
    let h2 = engine
        .start_execution(ExecutionType::JobSearch, "TEST_S14", Some(json!({"query": "Analista"})))
        .await
        .expect("Erro ao criar h2");
    h2.finish_failure("Falha de rede simulada").await.expect("Erro");

    // 3. Testa engine.get_executions_summary()
    let summary = engine.get_executions_summary().await.expect("Erro ao obter resumo");
    assert_eq!(summary.completed_executions, 1);
    assert_eq!(summary.failed_executions, 1);
    assert_eq!(summary.total_processed, 5);

    // 4. Testa rota Web /api/executions/summary
    let cfg = AppConfig {
        gemini_api_key: "".to_string(),
        database_url: temp_db_path.clone(),
        python_bin: ".venv/bin/python".to_string(),
        resume_path: "curriculo.md".to_string(),
    };
    let auto_status = automation::new_shared_status();
    let state = Arc::new(WebState {
        cfg,
        db,
        engine,
        auto_status,
    });
    let app = build_router(state);

    let req = Request::builder()
        .uri("/api/executions/summary")
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["completed_executions"], 1);
    assert_eq!(val["failed_executions"], 1);
    assert_eq!(val["total_processed"], 5);

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_sprint_15_execution_retry_endpoint() {
    let temp_db_path = format!("sqlite://jobs_test_s15_retry_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");
    let engine = ExecutionEngine::new(db.clone());

    // Cria execução falhada
    let h_orig = engine
        .start_execution(
            ExecutionType::JobSearch,
            "WEB_TEST",
            Some(json!({
                "query": "Rust Developer",
                "location": "Brasil",
                "limit": 5,
                "remote": true,
            })),
        )
        .await
        .expect("Erro ao criar exec");
    h_orig.finish_failure("Timeout na fonte").await.expect("Erro");

    let cfg = AppConfig {
        gemini_api_key: "".to_string(),
        database_url: temp_db_path.clone(),
        python_bin: ".venv/bin/python".to_string(),
        resume_path: "curriculo.md".to_string(),
    };
    let auto_status = automation::new_shared_status();
    let state = Arc::new(WebState {
        cfg,
        db,
        engine: engine.clone(),
        auto_status,
    });
    let app = build_router(state);

    // Dispara POST /api/executions/:id/retry
    let req = Request::builder()
        .method("POST")
        .uri(format!("/api/executions/{}/retry", h_orig.id()))
        .body(Body::empty())
        .unwrap();

    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["ok"], true);
    assert!(val["new_execution_id"].is_string());

    let new_id = val["new_execution_id"].as_str().unwrap();
    let new_exec = engine.get_execution(new_id).await.unwrap().expect("Nova execução deve existir");
    assert_eq!(new_exec.request_origin, "RETRY");
    assert_eq!(new_exec.execution_type, "JOB_SEARCH");

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_sprint_15_monitored_company_due_cycle() {
    let temp_db_path = format!("sqlite://jobs_test_s15_company_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");

    // Cadastra empresa monitorada
    let comp_id = db
        .upsert_company(
            "Acme Corp",
            "https://acme.com",
            "Tecnologia",
            "Araraquara - SP",
            "Empresa de tecnologia local",
            true, // is_monitored
            true, // is_preferred
            false,
        )
        .await
        .expect("Erro ao cadastrar empresa");

    // Adiciona fonte corporativa com next_check_at no passado
    let source_id = db
        .add_company_source(comp_id, "ATS_BOARD", "https://boards.greenhouse.io/acme", "greenhouse", "acme")
        .await
        .expect("Erro ao adicionar fonte");

    sqlx::query("UPDATE company_sources SET next_check_at = datetime('now', '-1 hour') WHERE id = ?1")
        .bind(source_id)
        .execute(db.pool())
        .await
        .expect("Erro ao atualizar data da fonte");

    // Verifica que get_due_monitored_companies encontra a empresa recém-cadastrada
    let due = db.get_due_monitored_companies(10).await.expect("Erro ao buscar empresas devidas");
    assert!(due.iter().any(|(id, name)| *id == comp_id && name == "Acme Corp"));

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}
