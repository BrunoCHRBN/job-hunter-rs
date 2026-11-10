use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use job_hunter_rs::automation;
use job_hunter_rs::collectors::brasil::parse_vagas_com_html;
use job_hunter_rs::config::AppConfig;
use job_hunter_rs::db::Database;
use job_hunter_rs::execution::ExecutionEngine;
use job_hunter_rs::models::{MatchAnalysisResult, RawJobListing};
use job_hunter_rs::normalizer::{
    canonicalize_url, normalize_company_name, normalize_job_title, normalize_location,
    titles_are_near_duplicates,
};
use job_hunter_rs::web::{build_router, WebState};

#[test]
fn test_sprint_10_normalizer_and_near_duplicates() {
    assert_eq!(normalize_company_name("Raízen Energia S.A."), "raizen energia");
    assert_eq!(normalize_company_name("Raizen Energia S/A"), "raizen energia");
    assert_eq!(normalize_company_name("Citrosuco do Brasil Ltda."), "citrosuco");

    assert_eq!(
        normalize_job_title("Engenheiro(a) de Dados Pl. (Remoto)"),
        "engenheiro de dados pleno"
    );
    assert_eq!(
        normalize_job_title("Eng. de Dados Pleno - 100% Remoto"),
        "engenheiro de dados pleno"
    );

    assert_eq!(
        normalize_location("Araraquara, São Paulo, Brazil"),
        "araraquara-sp"
    );
    assert_eq!(normalize_location("Araraquara / SP"), "araraquara-sp");
    assert_eq!(normalize_location("100% Remoto - Todo o Brasil"), "remoto");

    assert_eq!(
        canonicalize_url("https://raizen.gupy.io/jobs/12345/?utm_source=linkedin&trk=public_jobs/"),
        "https://raizen.gupy.io/jobs/12345"
    );

    assert!(titles_are_near_duplicates(
        "Analista de Suporte TI Jr (M/F)",
        "Analista de Suporte de TI Júnior"
    ));
    assert!(!titles_are_near_duplicates(
        "Operador de Empilhadeira",
        "Desenvolvedor Backend Rust"
    ));
}

#[test]
fn test_sprint_09_vagas_com_html_parser() {
    let sample_html = r#"
    <ul>
      <li class="vaga odd" data-id-vaga="2691001">
        <a class="link-detalhes-vaga" title="Operador de Sistemas Industriais" href="/vagas/v2691001/operador-de-sistemas">Operador</a>
        <span class="emprVaga">Usina Santa Cruz</span>
        <div class="vaga-local">Araraquara - SP</div>
        <div class="detalhes"><p>Obrigatório: Ensino médio completo e disponibilidade para escala 6x2. Diferencial: Curso técnico em automação.</p></div>
      </li>
    </ul>
    "#;

    let parsed = parse_vagas_com_html(sample_html, Some("Araraquara"), 10);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].external_id, "vagas_2691001");
    assert_eq!(parsed[0].source, "vagas_com");
    assert_eq!(parsed[0].title, "Operador de Sistemas Industriais");
    assert_eq!(parsed[0].company, "Usina Santa Cruz");
    assert_eq!(parsed[0].location, "Araraquara - SP");
}

#[tokio::test]
async fn test_sprints_09_10_11_12_13_end_to_end() {
    let temp_db_path = format!(
        "sqlite://jobs_test_sprints_{}.db",
        chrono::Utc::now().timestamp_micros()
    );
    let db = Database::connect(&temp_db_path)
        .await
        .expect("Falha ao criar banco SQLite de teste");

    // 1. Sprint 09: Verificar seed de coletores e toggle
    let collectors = db.list_collectors().await.unwrap();
    assert!(collectors.len() >= 10);
    assert!(collectors.iter().any(|c| c.source_id == "gupy" && c.enabled == 1));
    assert!(collectors.iter().any(|c| c.source_id == "vagas_com" && c.enabled == 1));

    db.toggle_collector("solides", false).await.unwrap();
    let updated_cols = db.list_collectors().await.unwrap();
    let solides = updated_cols.iter().find(|c| c.source_id == "solides").unwrap();
    assert_eq!(solides.enabled, 0);

    // 2. Sprint 11: Cadastrar empresa preferida e alias
    let comp_id = db
        .upsert_company(
            "Raízen S.A.",
            "https://www.raizen.com.br",
            "Energia",
            "Araraquara - SP",
            "Bioenergia",
            true,
            true,
            false,
        )
        .await
        .unwrap();
    db.add_company_alias(comp_id, "Raizen Energia").await.unwrap();

    // 3. Sprint 10: Inserir a mesma vaga vinda de 3 fontes diferentes (Gupy, LinkedIn, Vagas.com)
    let job_from_gupy = RawJobListing {
        external_id: "gupy_9991".to_string(),
        source: "gupy".to_string(),
        title: "Técnico(a) de Automação Pleno".to_string(),
        company: "Raízen S.A.".to_string(),
        location: "Araraquara - SP".to_string(),
        url: "https://raizen.gupy.io/jobs/9991?utm_source=gupy".to_string(),
        raw_description: "Obrigatório: CLP e instrumentação. Diferencial: Inglês intermediário."
            .to_string(),
        published_at: Some("2026-11-09".to_string()),
    };

    let job_from_linkedin = RawJobListing {
        external_id: "li_8882".to_string(),
        source: "jobspy:linkedin".to_string(),
        title: "Tec. de Automação Pl (Presencial)".to_string(),
        company: "Raizen Energia".to_string(),
        location: "Araraquara, São Paulo, Brazil".to_string(),
        url: "https://www.linkedin.com/jobs/view/8882?trk=public".to_string(),
        raw_description: "Obrigatório: Experiência com redes industriais, CLP, SCADA e instrumentação em usina sucroenergética. Diferencial: Curso NR10 atualizado.".to_string(),
        published_at: Some("2026-11-09".to_string()),
    };

    let inserted_first = db.insert_raw_job(&job_from_gupy).await.unwrap();
    assert!(inserted_first, "A primeira inserção deve criar a vaga");

    let inserted_second = db.insert_raw_job(&job_from_linkedin).await.unwrap();
    assert!(
        !inserted_second,
        "A segunda inserção (mesma vaga em fonte distinta) deve ser unificada sem duplicar registro"
    );

    let all_jobs = db.list_jobs(&Default::default()).await.unwrap();
    assert_eq!(all_jobs.len(), 1, "Deve existir exatamente 1 registro consolidado");
    let consolidated = &all_jobs[0];
    assert_eq!(consolidated.company_id, Some(comp_id));

    // Verifica rastreabilidade de múltiplas origens em job_sources
    let sources = db.get_job_sources(consolidated.id).await.unwrap();
    assert_eq!(sources.len(), 2, "Ambas as fontes originais devem estar preservadas");
    assert!(sources.iter().any(|s| s.source == "gupy" && s.external_id == "gupy_9991"));
    assert!(sources
        .iter()
        .any(|s| s.source == "jobspy:linkedin" && s.external_id == "li_8882"));

    // 4. Sprint 12: Verificar triagem determinística (PASS, sem penalizar salário/escala ausentes)
    assert_eq!(consolidated.screening_verdict.as_deref(), Some("PASS"));
    let scr = consolidated.screening().expect("Screening JSON deve existir");
    assert!(scr.eligible_for_ai);
    assert!(scr.score >= 80.0);
    assert!(!scr.missing_info.is_empty(), "Deve listar informações omissas sem reprovar");
    assert!(scr
        .classified_requirements
        .iter()
        .any(|r| r.tier == "MUST_HAVE"));
    assert!(scr
        .classified_requirements
        .iter()
        .any(|r| r.tier == "NICE_TO_HAVE"));

    // Harmonização com Match Score
    let mut match_res = MatchAnalysisResult::default();
    match_res.match_score = 88.0;
    match_res.verdict = "apply".to_string();
    db.update_job_match(consolidated.id, &match_res).await.unwrap();

    let refreshed = db.get_job_by_id(consolidated.id).await.unwrap().unwrap();
    let refreshed_scr = refreshed.screening().unwrap();
    assert!(refreshed_scr.harmonized_score.unwrap_or(0.0) >= 85.0);

    // Testa vaga com termo proibido -> deve receber REJECT na triagem determinística
    let rejected_job = RawJobListing {
        external_id: "rej_1".to_string(),
        source: "vagas_com".to_string(),
        title: "Vendedor de Loja Shopping".to_string(),
        company: "Loja X".to_string(),
        location: "Araraquara - SP".to_string(),
        url: "https://vagas.com.br/v1".to_string(),
        raw_description: "Vendas em loja.".to_string(),
        published_at: None,
    };
    db.insert_raw_job(&rejected_job).await.unwrap();
    let rej_record = db
        .list_jobs(&Default::default())
        .await
        .unwrap()
        .into_iter()
        .find(|j| j.external_id.as_deref() == Some("rej_1"))
        .unwrap();
    assert_eq!(rej_record.screening_verdict.as_deref(), Some("REJECT"));
    assert!(!rej_record.screening().unwrap().eligible_for_ai);

    // 5. Sprint 13: Cache BLAKE3 e Telemetria de IA
    db.put_ai_cache(
        "test_cache_key_1",
        "EXTRACTION_BATCH",
        "gemini-2.5-flash-lite",
        r#"{"jobs":[]}"#,
        450,
        120,
    )
    .await
    .unwrap();
    let cached = db.get_ai_cache("test_cache_key_1").await.unwrap();
    assert_eq!(cached.as_deref(), Some(r#"{"jobs":[]}"#));

    db.record_ai_operation(
        None,
        "EXTRACTION_BATCH",
        "gemini-2.5-flash-lite",
        false,
        1,
        "SUCCESS",
        5,
        450,
        120,
        680,
        None,
    )
    .await
    .unwrap();

    let telemetry = db.get_ai_telemetry_summary().await.unwrap();
    assert_eq!(telemetry.total_operations, 1);
    assert_eq!(telemetry.total_tokens, 570);
    assert_eq!(telemetry.cache_entries, 1);
    assert_eq!(telemetry.total_cache_hits, 1);

    // 6. Verificar rotas Web API (build_router)
    let engine = ExecutionEngine::new(db.clone());
    let state = Arc::new(WebState {
        cfg: AppConfig {
            gemini_api_key: String::new(),
            database_url: temp_db_path.clone(),
            python_bin: ".venv/bin/python".to_string(),
            resume_path: "curriculo.md".to_string(),
        },
        db: db.clone(),
        engine,
        auto_status: automation::new_shared_status(),
    });
    let app = build_router(state);

    for uri in [
        "/api/collectors",
        "/api/companies",
        "/api/ai/telemetry",
        &format!("/api/jobs/{}/sources", consolidated.id),
    ] {
        let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK, "Falha em GET {}", uri);
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        let _json: Value = serde_json::from_slice(&bytes).unwrap();
    }

    let clean_path = temp_db_path.trim_start_matches("sqlite://");
    let _ = std::fs::remove_file(clean_path);
}
