use job_hunter_rs::db::Database;
use job_hunter_rs::execution::{
    EventLevel, ExecutionEngine, ExecutionEventMessage, ExecutionType,
};
use job_hunter_rs::models::{JobFilter, RawJobListing};

#[tokio::test]
async fn test_execution_lifecycle_and_persistence() {
    let temp_db_path = format!("sqlite://jobs_test_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco de teste");
    let engine = ExecutionEngine::new(db.clone());

    let mut rx = engine.subscribe();

    // 1. Inicia execução
    let handle = engine
        .start_execution(
            ExecutionType::JobSearch,
            "TEST_SUITE",
            Some(serde_json::json!({"query": "Rust", "location": "Brasil"})),
        )
        .await
        .expect("Erro ao iniciar execução");

    assert_eq!(handle.execution_type, ExecutionType::JobSearch);
    assert!(!handle.is_cancelled());

    // Verifica mensagem de criação no canal
    let msg1 = rx.recv().await.expect("Erro ao receber mensagem do canal");
    match msg1 {
        ExecutionEventMessage::ExecutionCreated(rec) => {
            assert_eq!(rec.id, handle.id());
            assert_eq!(rec.execution_type, "JOB_SEARCH");
            assert_eq!(rec.status, "RUNNING");
        }
        other => panic!("Esperava ExecutionCreated, recebeu {:?}", other),
    }

    // 2. Cria e finaliza etapa
    let stage = handle.start_stage("coleta_gupy").await.expect("Erro ao criar etapa");
    assert_eq!(stage.stage_name, "coleta_gupy");

    handle
        .emit(EventLevel::Info, "coleta_gupy", Some("gupy"), "Buscando vagas na Gupy", None)
        .await
        .expect("Erro ao emitir evento");

    stage.complete(10, 5).await.expect("Erro ao finalizar etapa com sucesso");

    // 3. Atualiza progresso e registra métrica
    handle.update_progress(50.0, Some("triagem"), 5, 0).await.expect("Erro ao atualizar progresso");
    handle.record_metric("fetch_duration_ms", 125.0, "ms").await.expect("Erro ao registrar métrica");

    // 4. Finaliza com sucesso
    handle
        .finish_success(5, 0, Some("Coleta concluída com sucesso"))
        .await
        .expect("Erro ao finalizar execução");

    // 5. Consulta detalhes da execução no banco
    let detail = engine
        .get_execution_detail(handle.id())
        .await
        .expect("Erro ao buscar detalhes")
        .expect("Execução deveria existir");

    assert_eq!(detail.execution.status, "COMPLETED");
    assert_eq!(detail.execution.processed_count, 5);
    assert_eq!(detail.stages.len(), 1);
    assert_eq!(detail.stages[0].stage_name, "coleta_gupy");
    assert_eq!(detail.stages[0].status, "COMPLETED");
    assert_eq!(detail.stages[0].items_in, 10);
    assert_eq!(detail.stages[0].items_out, 5);
    assert!(!detail.events.is_empty());
    assert_eq!(detail.events[0].message, "Buscando vagas na Gupy");
    assert_eq!(detail.metrics.len(), 4); // fetch_duration_ms + 3 metrics padrão

    // 6. Lista execuções
    let list = engine
        .list_executions(10, 0, Some("JOB_SEARCH"), None)
        .await
        .expect("Erro ao listar execuções");
    assert!(list.iter().any(|e| e.id == handle.id()));

    // Limpeza do arquivo de banco temporário
    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_execution_cancellation() {
    let temp_db_path = format!("sqlite://jobs_test_cancel_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");
    let engine = ExecutionEngine::new(db.clone());

    let handle = engine
        .start_execution(ExecutionType::Automation, "TEST", None)
        .await
        .expect("Erro ao iniciar execução");

    assert!(!handle.is_cancelled());

    let cancelled = engine.cancel_execution(handle.id()).await.expect("Erro ao cancelar");
    assert!(cancelled);
    assert!(handle.is_cancelled());

    let record = engine.get_execution(handle.id()).await.expect("Erro").unwrap();
    assert_eq!(record.status, "CANCELLED");

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}

#[tokio::test]
async fn test_db_source_filtering() {
    let temp_db_path = format!("sqlite://jobs_test_src_{}.db", chrono::Utc::now().timestamp_micros());
    let db = Database::connect(&temp_db_path).await.expect("Erro ao conectar no banco");

    let sample_jobs = vec![
        RawJobListing {
            external_id: "ext-1".to_string(),
            source: "gupy".to_string(),
            title: "Desenvolvedor Rust".to_string(),
            company: "Tech Corp".to_string(),
            location: "Araraquara - SP".to_string(),
            url: "https://gupy.io/vaga1".to_string(),
            raw_description: "Rust e SQLite".to_string(),
            published_at: None,
        },
        RawJobListing {
            external_id: "ext-2".to_string(),
            source: "jobspy:linkedin".to_string(),
            title: "Engenheiro de Software".to_string(),
            company: "Global Inc".to_string(),
            location: "São Paulo - SP".to_string(),
            url: "https://linkedin.com/jobs/view/2".to_string(),
            raw_description: "Go e Rust".to_string(),
            published_at: None,
        },
        RawJobListing {
            external_id: "ext-3".to_string(),
            source: "jobspy:indeed".to_string(),
            title: "Analista de Sistemas".to_string(),
            company: "Local SA".to_string(),
            location: "Remoto".to_string(),
            url: "https://indeed.com/viewjob/3".to_string(),
            raw_description: "Python".to_string(),
            published_at: None,
        },
    ];

    let (inserted, _) = db.insert_raw_jobs_batch(&sample_jobs).await.expect("Erro ao inserir");
    assert_eq!(inserted, 3);

    // Filtra por fonte Gupy
    let gupy_filter = JobFilter {
        source: Some("gupy".to_string()),
        ..Default::default()
    };
    let gupy_jobs = db.list_jobs(&gupy_filter).await.expect("Erro ao listar");
    assert_eq!(gupy_jobs.len(), 1);
    assert_eq!(gupy_jobs[0].source.as_deref(), Some("gupy"));

    // Filtra por fonte LinkedIn
    let li_filter = JobFilter {
        source: Some("linkedin".to_string()),
        ..Default::default()
    };
    let li_jobs = db.list_jobs(&li_filter).await.expect("Erro ao listar");
    assert_eq!(li_jobs.len(), 1);
    assert_eq!(li_jobs[0].source.as_deref(), Some("jobspy:linkedin"));

    // Filtra por todas
    let all_filter = JobFilter {
        source: Some("Todas".to_string()),
        ..Default::default()
    };
    let all_jobs = db.list_jobs(&all_filter).await.expect("Erro ao listar");
    assert_eq!(all_jobs.len(), 3);

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}
