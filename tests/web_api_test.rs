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
use job_hunter_rs::models::{ExtractedJobMetadata, MatchAnalysisResult, RawJobListing};
use job_hunter_rs::web::WebState;

#[tokio::test]
async fn test_web_execution_endpoints_and_sse() {
    let temp_db_path = format!("sqlite://jobs_test_web_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");
    let engine = ExecutionEngine::new(db.clone());
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

    // Cria uma execução para teste
    let handle = engine
        .start_execution(
            ExecutionType::JobSearch,
            "WEB_TEST",
            Some(json!({"query": "Rust"})),
        )
        .await
        .expect("Erro ao criar exec");

    handle.finish_success(12, 0, Some("Finalizado teste")).await.expect("Erro");

    // Constrói o app Axum
    let app = axum::Router::new()
        .route("/api/executions", axum::routing::get(job_hunter_rs::web::api_list_executions))
        .route("/api/executions/:id", axum::routing::get(job_hunter_rs::web::api_get_execution))
        .with_state(state);

    // 1. GET /api/executions
    let req = Request::builder()
        .uri("/api/executions")
        .body(Body::empty())
        .unwrap();

    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let body = resp.into_body().collect().await.unwrap().to_bytes();
    let val: Value = serde_json::from_slice(&body).unwrap();
    assert!(val.is_array());
    let arr = val.as_array().unwrap();
    assert!(!arr.is_empty());
    assert_eq!(arr[0]["id"], handle.id());
    assert_eq!(arr[0]["status"], "COMPLETED");

    // 2. GET /api/executions/:id
    let req_id = Request::builder()
        .uri(format!("/api/executions/{}", handle.id()))
        .body(Body::empty())
        .unwrap();

    let resp_id = app.oneshot(req_id).await.unwrap();
    assert_eq!(resp_id.status(), StatusCode::OK);

    let body_id = resp_id.into_body().collect().await.unwrap().to_bytes();
    let val_id: Value = serde_json::from_slice(&body_id).unwrap();
    assert_eq!(val_id["execution"]["id"], handle.id());
    assert_eq!(val_id["execution"]["processed_count"], 12);

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_resume_upload_pdf_endpoint() {
    let temp_db_path = format!("sqlite://jobs_test_pdf_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");
    let engine = ExecutionEngine::new(db.clone());
    let cfg = AppConfig {
        gemini_api_key: "".to_string(), // Testa modo offline resiliente
        database_url: temp_db_path.clone(),
        python_bin: ".venv/bin/python".to_string(),
        resume_path: "curriculo_test_pdf.md".to_string(),
    };
    let auto_status = automation::new_shared_status();

    let state = Arc::new(WebState {
        cfg,
        db,
        engine,
        auto_status,
    });

    let app = job_hunter_rs::web::build_router(state);

    // 1. PDF fixture real gerado com conteúdo textual
    let pdf_bytes = include_bytes!("fixtures/sample_curriculo.pdf");

    let req_pdf = Request::builder()
        .method("POST")
        .uri("/api/resume/upload-pdf")
        .header("Content-Type", "application/pdf")
        .body(Body::from(pdf_bytes.to_vec()))
        .unwrap();

    let resp = app.clone().oneshot(req_pdf).await.unwrap();
    let status = resp.status();
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    if status != StatusCode::OK {
        eprintln!("ERRO RETORNADO: status={}, body={}", status, String::from_utf8_lossy(&body));
    }
    assert_eq!(status, StatusCode::OK);
    let val: Value = serde_json::from_slice(&body).unwrap();
    let content = val["content"].as_str().expect("content deve ser string");
    assert!(content.contains("Bruno Santos"), "Conteúdo extraído deve conter nome");
    assert!(content.contains("Desenvolvedor Rust e Python"), "Conteúdo extraído deve conter cargo");
    assert_eq!(val["ai_structured"], false, "Sem chave gemini, deve operar em fallback offline");
    assert!(val["char_count"].as_u64().unwrap_or(0) > 20);

    // 2. Rejeição com 400 Bad Request se body for vazio
    let req_empty = Request::builder()
        .method("POST")
        .uri("/api/resume/upload-pdf")
        .body(Body::empty())
        .unwrap();

    let resp_empty = app.oneshot(req_empty).await.unwrap();
    assert_eq!(resp_empty.status(), StatusCode::BAD_REQUEST);

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_jobs_api_summary_and_detail_contract() {
    let temp_db_path = format!(
        "sqlite://jobs_test_payload_{}.db",
        chrono::Utc::now().timestamp_micros()
    );
    let db = Database::connect(&temp_db_path)
        .await
        .expect("Erro ao conectar no banco");
    let listing = RawJobListing {
        external_id: "payload-1".to_string(),
        source: "gupy".to_string(),
        title: "Desenvolvedor Rust".to_string(),
        company: "Tech Corp".to_string(),
        location: "Remoto".to_string(),
        url: "https://gupy.io/vaga/payload-1".to_string(),
        raw_description: "Descrição completa da vaga".to_string(),
        published_at: None,
    };
    assert!(db.insert_raw_job(&listing).await.unwrap());
    let job = db.list_jobs(&Default::default()).await.unwrap().remove(0);
    let metadata = ExtractedJobMetadata {
        summary: "Resumo exibido na interface".to_string(),
        required_skills: vec!["Rust".to_string()],
        ..Default::default()
    };
    db.update_job_enrichment(job.id, &metadata, Some(&[0.25, 0.5, 0.75]))
        .await
        .unwrap();
    let analysis = MatchAnalysisResult {
        match_score: 92.0,
        matching_skills: vec!["Rust".to_string()],
        detailed_analysis_markdown: "Análise preservada".to_string(),
        ..Default::default()
    };
    db.update_job_match(job.id, &analysis).await.unwrap();
    db.screen_job_by_id(job.id).await.unwrap();

    let state = Arc::new(WebState {
        cfg: AppConfig {
            gemini_api_key: String::new(),
            database_url: temp_db_path.clone(),
            python_bin: ".venv/bin/python".to_string(),
            resume_path: "curriculo.md".to_string(),
        },
        db: db.clone(),
        engine: ExecutionEngine::new(db),
        auto_status: automation::new_shared_status(),
    });
    let app = job_hunter_rs::web::build_router(state);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    let job = &value[0];

    assert_eq!(job["detail_loaded"], false);
    assert!(job.get("embedding_json").is_none());
    assert!(job.get("dossier_json").is_none());
    assert!(job.get("fit_json").is_none());
    assert!(job.get("screening_json").is_none());
    assert!(job.get("raw_description").is_none());
    assert!(job.get("match_analysis_md").is_none());
    assert!(job.get("dossier").is_none());
    assert!(job.get("fit").is_none());
    assert!(job.get("screening").is_none());
    assert_eq!(job["title"], "Desenvolvedor Rust");
    assert_eq!(job["ai_summary"], "Resumo exibido na interface");
    assert_eq!(job["required_skills_json"], "[\"Rust\"]");
    assert_eq!(job["verdict_kind"], "apply");

    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/jobs/{}", job["id"].as_i64().unwrap()))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let value: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(value["detail_loaded"], true);
    assert_eq!(value["raw_description"], "Descrição completa da vaga");
    assert!(value["match_analysis_md"]
        .as_str()
        .unwrap()
        .contains("Análise preservada"));
    assert!(value["dossier"].get("sections").is_some());
    assert_eq!(value["fit"]["matching_skills"], json!(["Rust"]));
    assert_eq!(
        value["fit"]["detailed_analysis_markdown"],
        "Análise preservada"
    );
    assert!(value["screening"].get("verdict").is_some());
    assert!(value.get("embedding_json").is_none());

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}
