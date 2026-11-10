use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::models::{
    AiOperationRecord, AiTelemetrySummary, CandidateProfile, CollectorConfig, DbStats,
    ExtractedJobMetadata, JobFilter, JobRecord, JobSourceRecord, MatchAnalysisResult,
    RawJobListing, SavedSearch,
};
use crate::normalizer::{
    canonicalize_url, compute_enhanced_dedup_hash, locations_are_compatible, normalize_company_name,
    normalize_job_title, normalize_location, titles_are_near_duplicates,
};
use crate::screening::{evaluate_job_screening, ScreeningResult};

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");
const MIGRATION_002: &str = include_str!("../migrations/002_dossie.sql");
const MIGRATION_003: &str = include_str!("../migrations/003_automation.sql");
const MIGRATION_004: &str = include_str!("../migrations/004_execution_engine.sql");
const MIGRATION_005: &str =
    include_str!("../migrations/005_multi_source_company_screening_ai.sql");

pub const JOB_SELECT_COLUMNS: &str = "id, dedup_hash, external_id, source, title, company, location, url, raw_description, is_enriched, work_model, contract_type, seniority, shift_schedule, salary_estimate_brl, required_skills_json, red_flags_json, ai_summary, embedding_json, application_status, match_score, match_analysis_md, published_at, created_at, dossier_json, fit_json, normalized_company, normalized_title, canonical_url, company_id, screening_verdict, screening_score, screening_json, source_status";

/// Gera o hash blake3 determinístico para deduplicação de vagas:
/// `format!("{}|{}|{}", company.trim().to_lowercase(), title.trim().to_lowercase(), location.trim().to_lowercase())`
pub fn compute_dedup_hash(company: &str, title: &str, location: &str) -> String {
    let normalized = format!(
        "{}|{}|{}",
        company.trim().to_lowercase(),
        title.trim().to_lowercase(),
        location.trim().to_lowercase()
    );
    blake3::hash(normalized.as_bytes()).to_hex().to_string()
}

/// Calcula a similaridade de cosseno entre dois vetores de ponto flutuante em memória.
/// Retorna `0.0` caso os comprimentos divirjam, estejam vazios ou alguma norma seja zero.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.is_empty() || a.len() != b.len() {
        return 0.0;
    }

    let mut dot = 0.0_f32;
    let mut norm_a = 0.0_f32;
    let mut norm_b = 0.0_f32;

    for (&x, &y) in a.iter().zip(b.iter()) {
        dot += x * y;
        norm_a += x * x;
        norm_b += y * y;
    }

    let denom = norm_a.sqrt() * norm_b.sqrt();
    if denom <= f32::EPSILON {
        0.0
    } else {
        dot / denom
    }
}

#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let normalized_url = if database_url.starts_with("sqlite:") {
            database_url.to_string()
        } else {
            format!("sqlite://{}", database_url)
        };

        let options = SqliteConnectOptions::from_str(&normalized_url)
            .with_context(|| format!("URL do banco SQLite inválida: {}", normalized_url))?
            .create_if_missing(true);

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .context("Falha ao abrir conexão com o banco SQLite")?;

        sqlx::raw_sql(MIGRATION_001)
            .execute(&pool)
            .await
            .context("Falha ao aplicar migration inicial (001_init.sql)")?;

        let existing_cols: Vec<String> = sqlx::query("PRAGMA table_info(jobs)")
            .fetch_all(&pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|r| r.try_get::<String, _>("name").ok())
            .collect();

        if !existing_cols.iter().any(|c| c == "dossier_json")
            || !existing_cols.iter().any(|c| c == "fit_json")
        {
            sqlx::raw_sql(MIGRATION_002)
                .execute(&pool)
                .await
                .context("Falha ao aplicar migration (002_dossie.sql)")?;
        }

        sqlx::raw_sql(MIGRATION_003)
            .execute(&pool)
            .await
            .context("Falha ao aplicar migration (003_automation.sql)")?;

        sqlx::raw_sql(MIGRATION_004)
            .execute(&pool)
            .await
            .context("Falha ao aplicar migration (004_execution_engine.sql)")?;

        sqlx::raw_sql(MIGRATION_005)
            .execute(&pool)
            .await
            .context("Falha ao aplicar migration (005_multi_source_company_screening_ai.sql)")?;

        let new_job_cols = [
            ("normalized_company", "TEXT"),
            ("normalized_title", "TEXT"),
            ("canonical_url", "TEXT"),
            ("company_id", "INTEGER"),
            ("screening_verdict", "TEXT"),
            ("screening_score", "REAL"),
            ("screening_json", "TEXT"),
            ("source_status", "TEXT DEFAULT 'ACTIVE'"),
        ];

        for (col_name, col_type) in new_job_cols {
            if !existing_cols.iter().any(|c| c == col_name) {
                let alter_sql = format!("ALTER TABLE jobs ADD COLUMN {} {}", col_name, col_type);
                let _ = sqlx::query(&alter_sql).execute(&pool).await;
            }
        }

        let db = Self { pool };
        db.seed_defaults_and_backfill().await?;

        Ok(db)
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    async fn seed_defaults_and_backfill(&self) -> Result<()> {
        let count_row = sqlx::query("SELECT COUNT(*) as cnt FROM saved_searches")
            .fetch_one(&self.pool)
            .await?;
        let saved_cnt: i64 = count_row.try_get("cnt").unwrap_or(0);
        if saved_cnt == 0 {
            let defaults = [
                ("Operador", "Araraquara, São Carlos, Matão", 0_i64, 180_i64),
                ("Técnico", "Araraquara, São Carlos", 0_i64, 180_i64),
                ("Python", "", 1_i64, 240_i64),
            ];
            for (q, loc, rem, interval) in defaults {
                let _ = sqlx::query(
                    "INSERT INTO saved_searches (query, location, remote, enabled, interval_minutes) VALUES (?1, ?2, ?3, 1, ?4)",
                )
                .bind(q)
                .bind(loc)
                .bind(rem)
                .bind(interval)
                .execute(&self.pool)
                .await;
            }
        }

        // Seed dos coletores padronizados (Sprint 09: Grupos A, B e C)
        let default_collectors = [
            (
                "gupy",
                "Gupy Portal & SSR",
                "Grupo C — ATS & Carreiras",
                1_i64,
                25_i64,
                25_i64,
                "Coleta direta no portal.gupy.io via SSR JSON (__NEXT_DATA__) e portais corporativos {slug}.gupy.io.",
            ),
            (
                "linkedin",
                "LinkedIn (JobSpy)",
                "Grupo A — JobSpy",
                1_i64,
                35_i64,
                20_i64,
                "Ponte Python JobSpy com busca multi-cidade isolada e extração completa de descrição.",
            ),
            (
                "indeed",
                "Indeed Brasil (JobSpy)",
                "Grupo A — JobSpy",
                1_i64,
                35_i64,
                20_i64,
                "Ponte Python JobSpy configurada com country_indeed=Brazil.",
            ),
            (
                "glassdoor",
                "Glassdoor Brasil (JobSpy)",
                "Grupo A — JobSpy",
                1_i64,
                35_i64,
                20_i64,
                "Ponte Python JobSpy com resolução de localidade municipal.",
            ),
            (
                "vagas_com",
                "Vagas.com.br",
                "Grupo B — Plataformas Brasileiras",
                1_i64,
                20_i64,
                20_i64,
                "Coletor HTTP nativo em Rust consultando a listagem pública vagas.com.br/vagas-de-{termo}.",
            ),
            (
                "solides",
                "Sólides Vagas",
                "Grupo B — Plataformas Brasileiras",
                1_i64,
                20_i64,
                15_i64,
                "Coletor de páginas públicas Sólides / JSON-LD JobPosting com diagnóstico explícito de bloqueio WAF.",
            ),
            (
                "infojobs",
                "InfoJobs Brasil",
                "Grupo B — Plataformas Brasileiras",
                0_i64,
                20_i64,
                15_i64,
                "Monitoramento de páginas públicas com detecção de anti-bot/Cloudflare (status BLOCKED quando restrito).",
            ),
            (
                "sine",
                "Emprega Brasil / SINE",
                "Grupo B — Plataformas Brasileiras",
                0_i64,
                20_i64,
                15_i64,
                "Portal gov.br exige autenticação Gov.br para listagem completa; mantido para diagnóstico explícito.",
            ),
            (
                "greenhouse",
                "Greenhouse ATS",
                "Grupo C — ATS & Carreiras",
                1_i64,
                20_i64,
                30_i64,
                "API JSON pública oficial boards-api.greenhouse.io/v1/boards/{portal_id}/jobs.",
            ),
            (
                "lever",
                "Lever ATS",
                "Grupo C — ATS & Carreiras",
                1_i64,
                20_i64,
                30_i64,
                "API JSON pública oficial api.lever.co/v0/postings/{portal_id}?mode=json.",
            ),
            (
                "ashby",
                "Ashby ATS",
                "Grupo C — ATS & Carreiras",
                1_i64,
                20_i64,
                30_i64,
                "API JSON pública oficial api.ashbyhq.com/posting-api/job-board/{portal_id}.",
            ),
            (
                "workday",
                "Workday / JSON-LD Careers",
                "Grupo C — ATS & Carreiras",
                1_i64,
                25_i64,
                25_i64,
                "Extração via endpoint CXS Workday e fallback de schema.org/JobPosting em páginas corporativas.",
            ),
        ];

        for (id, name, group, enabled, timeout, limit, notes) in default_collectors {
            let _ = sqlx::query(
                r#"
                INSERT OR IGNORE INTO collector_configs
                (source_id, display_name, group_name, enabled, timeout_secs, default_limit, strategy_notes)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
            )
            .bind(id)
            .bind(name)
            .bind(group)
            .bind(enabled)
            .bind(timeout)
            .bind(limit)
            .bind(notes)
            .execute(&self.pool)
            .await;
        }

        // Seed inicial de empresas monitoradas (Sprint 11) caso a tabela esteja vazia
        let comp_cnt_row = sqlx::query("SELECT COUNT(*) as cnt FROM companies")
            .fetch_one(&self.pool)
            .await?;
        let comp_cnt: i64 = comp_cnt_row.try_get("cnt").unwrap_or(0);
        if comp_cnt == 0 {
            let seed_companies = [
                (
                    "Raízen",
                    "https://www.raizen.com.br",
                    "Energia & Bioenergia",
                    "Araraquara / Piracicaba - SP",
                    "Líder em bioenergia e transição energética com forte presença na região central de SP.",
                    1_i64,
                    "gupy",
                    "raizen",
                    "https://raizen.gupy.io",
                ),
                (
                    "Citrosuco",
                    "https://www.citrosuco.com.br",
                    "Agroindústria & Alimentos",
                    "Matão / Araraquara - SP",
                    "Maior produtora global de suco de laranja com sede industrial em Matão/SP.",
                    1_i64,
                    "gupy",
                    "citrosuco",
                    "https://citrosuco.gupy.io",
                ),
                (
                    "Randoncorp / Fras-le",
                    "https://www.randoncorp.com",
                    "Indústria Automotiva & Autopeças",
                    "Araraquara - SP",
                    "Grupo industrial metalmecânico e autopeças com operações em Araraquara.",
                    1_i64,
                    "gupy",
                    "randoncorp",
                    "https://randoncorp.gupy.io",
                ),
                (
                    "Heineken Brasil",
                    "https://www.heinekenbrasil.com.br",
                    "Bebidas & Manufatura",
                    "Araraquara - SP",
                    "Unidade fabril cervejeira em Araraquara/SP.",
                    1_i64,
                    "gupy",
                    "heineken",
                    "https://heineken.gupy.io",
                ),
                (
                    "Nubank",
                    "https://nubank.com.br",
                    "Tecnologia & Finanças",
                    "São Paulo - SP / Remoto",
                    "Plataforma digital de serviços financeiros usando Greenhouse ATS.",
                    0_i64,
                    "greenhouse",
                    "nubank",
                    "https://boards.greenhouse.io/nubank",
                ),
            ];

            for (name, site, seg, loc, desc, pref, ats, portal_id, career_url) in seed_companies {
                let norm = normalize_company_name(name);
                if let Ok(res) = sqlx::query(
                    r#"
                    INSERT OR IGNORE INTO companies
                    (official_name, normalized_name, website, segment, location, description, is_monitored, is_preferred)
                    VALUES (?1, ?2, ?3, ?4, ?5, ?6, 1, ?7)
                    "#,
                )
                .bind(name)
                .bind(&norm)
                .bind(site)
                .bind(seg)
                .bind(loc)
                .bind(desc)
                .bind(pref)
                .execute(&self.pool)
                .await
                {
                    if res.rows_affected() > 0 {
                        let comp_id = res.last_insert_rowid();
                        let _ = sqlx::query(
                            r#"
                            INSERT INTO company_sources
                            (company_id, source_type, career_url, ats_provider, portal_id, integration_status)
                            VALUES (?1, 'ATS_BOARD', ?2, ?3, ?4, 'ACTIVE')
                            "#,
                        )
                        .bind(comp_id)
                        .bind(career_url)
                        .bind(ats)
                        .bind(portal_id)
                        .execute(&self.pool)
                        .await;
                    }
                }
            }
        }

        // Backfill incremental de normalização e job_sources para vagas já existentes no banco
        let unnormalized = sqlx::query(
            "SELECT id, external_id, source, title, company, url FROM jobs WHERE normalized_company IS NULL LIMIT 500",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        for row in unnormalized {
            let job_id: i64 = row.try_get("id").unwrap_or_default();
            let ext_id: String = row
                .try_get::<Option<String>, _>("external_id")
                .ok()
                .flatten()
                .unwrap_or_default();
            let src: String = row
                .try_get::<Option<String>, _>("source")
                .ok()
                .flatten()
                .unwrap_or_else(|| "db".to_string());
            let title: String = row
                .try_get::<Option<String>, _>("title")
                .ok()
                .flatten()
                .unwrap_or_default();
            let company: String = row
                .try_get::<Option<String>, _>("company")
                .ok()
                .flatten()
                .unwrap_or_default();
            let url: String = row
                .try_get::<Option<String>, _>("url")
                .ok()
                .flatten()
                .unwrap_or_default();

            let norm_comp = normalize_company_name(&company);
            let norm_title = normalize_job_title(&title);
            let canon_url = canonicalize_url(&url);
            let comp_id = self.resolve_or_register_company_id(&company, &norm_comp).await;

            let _ = sqlx::query(
                "UPDATE jobs SET normalized_company = ?1, normalized_title = ?2, canonical_url = ?3, company_id = COALESCE(company_id, ?4) WHERE id = ?5",
            )
            .bind(&norm_comp)
            .bind(&norm_title)
            .bind(&canon_url)
            .bind(comp_id)
            .bind(job_id)
            .execute(&self.pool)
            .await;

            let _ = sqlx::query(
                r#"
                INSERT OR IGNORE INTO job_sources (job_id, source, external_id, original_url)
                VALUES (?1, ?2, ?3, ?4)
                "#,
            )
            .bind(job_id)
            .bind(&src)
            .bind(&ext_id)
            .bind(&url)
            .execute(&self.pool)
            .await;
        }

        // Backfill de triagem determinística (Sprint 12) para vagas ainda sem screening_verdict
        let unscreened_cnt: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM jobs WHERE screening_verdict IS NULL")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("cnt"))
            .unwrap_or(0);
        if unscreened_cnt > 0 {
            let _ = self.screen_all_jobs().await;
        }

        // Consolidação automática de duplicatas cross-fonte (Sprint 10)
        let _ = self.consolidate_duplicate_jobs().await;

        Ok(())
    }

    pub async fn resolve_or_register_company_id(
        &self,
        raw_company: &str,
        norm_comp: &str,
    ) -> Option<i64> {
        let trimmed = raw_company.trim();
        if norm_comp.is_empty()
            || norm_comp == "empresa n d"
            || norm_comp == "n d"
            || norm_comp == "confidencial"
        {
            return None;
        }

        if let Ok(Some(row)) = sqlx::query("SELECT id FROM companies WHERE normalized_name = ?1")
            .bind(norm_comp)
            .fetch_optional(&self.pool)
            .await
        {
            if let Ok(id) = row.try_get::<i64, _>("id") {
                return Some(id);
            }
        }

        if let Ok(Some(row)) =
            sqlx::query("SELECT company_id FROM company_aliases WHERE normalized_alias = ?1")
                .bind(norm_comp)
                .fetch_optional(&self.pool)
                .await
        {
            if let Ok(id) = row.try_get::<i64, _>("company_id") {
                return Some(id);
            }
        }

        let res = sqlx::query(
            r#"
            INSERT OR IGNORE INTO companies (official_name, normalized_name, is_monitored)
            VALUES (?1, ?2, 0)
            "#,
        )
        .bind(trimmed)
        .bind(norm_comp)
        .execute(&self.pool)
        .await
        .ok()?;

        if res.rows_affected() > 0 {
            Some(res.last_insert_rowid())
        } else {
            sqlx::query("SELECT id FROM companies WHERE normalized_name = ?1")
                .bind(norm_comp)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten()
                .and_then(|r| r.try_get::<i64, _>("id").ok())
        }
    }

    async fn find_existing_duplicate_job_id(
        &self,
        job: &RawJobListing,
        norm_comp: &str,
        company_id: Option<i64>,
        _norm_loc: &str,
        canon_url: &str,
        enhanced_hash: &str,
        legacy_hash: &str,
    ) -> Result<Option<i64>> {
        // Nível 1: Identificador estável na respectiva fonte
        if !job.external_id.trim().is_empty() {
            if let Some(row) = sqlx::query(
                "SELECT job_id FROM job_sources WHERE source = ?1 AND external_id = ?2 LIMIT 1",
            )
            .bind(&job.source)
            .bind(&job.external_id)
            .fetch_optional(&self.pool)
            .await?
            {
                return Ok(Some(row.try_get("job_id")?));
            }

            if let Some(row) = sqlx::query(
                "SELECT id FROM jobs WHERE source = ?1 AND external_id = ?2 LIMIT 1",
            )
            .bind(&job.source)
            .bind(&job.external_id)
            .fetch_optional(&self.pool)
            .await?
            {
                return Ok(Some(row.try_get("id")?));
            }
        }

        // Nível 2: URL canônica confiável
        if canon_url.len() > 24 {
            if let Some(row) =
                sqlx::query("SELECT id FROM jobs WHERE canonical_url = ?1 LIMIT 1")
                    .bind(canon_url)
                    .fetch_optional(&self.pool)
                    .await?
            {
                return Ok(Some(row.try_get("id")?));
            }
        }

        // Nível 3: Hash BLAKE3 composto (normalizado ou legado)
        if let Some(row) = sqlx::query(
            "SELECT id FROM jobs WHERE dedup_hash = ?1 OR dedup_hash = ?2 LIMIT 1",
        )
        .bind(enhanced_hash)
        .bind(legacy_hash)
        .fetch_optional(&self.pool)
        .await?
        {
            return Ok(Some(row.try_get("id")?));
        }

        // Nível 4: Similaridade textual de título dentro da mesma empresa (ou alias via company_id) e localidade
        if !norm_comp.is_empty() && norm_comp != "empresa n d" {
            let candidates = sqlx::query(
                "SELECT id, title, location FROM jobs WHERE normalized_company = ?1 OR (?2 IS NOT NULL AND company_id = ?2) ORDER BY id DESC LIMIT 40",
            )
            .bind(norm_comp)
            .bind(company_id)
            .fetch_all(&self.pool)
            .await?;

            for cand in candidates {
                let cand_id: i64 = cand.try_get("id")?;
                let cand_title: String = cand
                    .try_get::<Option<String>, _>("title")
                    .ok()
                    .flatten()
                    .unwrap_or_default();
                let cand_loc: String = cand
                    .try_get::<Option<String>, _>("location")
                    .ok()
                    .flatten()
                    .unwrap_or_default();

                let loc_compatible = locations_are_compatible(&cand_loc, &job.location);

                if loc_compatible && titles_are_near_duplicates(&cand_title, &job.title) {
                    return Ok(Some(cand_id));
                }
            }
        }

        Ok(None)
    }

    async fn upsert_job_source(
        &self,
        job_id: i64,
        source: &str,
        external_id: &str,
        original_url: &str,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO job_sources (job_id, source, external_id, original_url, first_seen_at, last_seen_at, source_status)
            VALUES (?1, ?2, ?3, ?4, datetime('now'), datetime('now'), 'ACTIVE')
            ON CONFLICT(job_id, source, external_id) DO UPDATE SET
                last_seen_at = datetime('now'),
                original_url = CASE WHEN excluded.original_url != '' THEN excluded.original_url ELSE job_sources.original_url END,
                source_status = 'ACTIVE'
            "#,
        )
        .bind(job_id)
        .bind(source)
        .bind(external_id)
        .bind(original_url)
        .execute(&self.pool)
        .await
        .context("Erro ao registrar rastreabilidade em job_sources")?;
        Ok(())
    }

    /// Insere uma vaga bruta utilizando deduplicação em 4 níveis e registrando sua origem em `job_sources`.
    /// Retorna `Ok(true)` se uma nova oportunidade foi criada ou `Ok(false)` se foi unificada a uma vaga existente.
    pub async fn insert_raw_job(&self, job: &RawJobListing) -> Result<bool> {
        let norm_comp = normalize_company_name(&job.company);
        let norm_title = normalize_job_title(&job.title);
        let norm_loc = normalize_location(&job.location);
        let canon_url = canonicalize_url(&job.url);
        let enhanced_hash = compute_enhanced_dedup_hash(&job.company, &job.title, &job.location);
        let legacy_hash = compute_dedup_hash(&job.company, &job.title, &job.location);

        let company_id = self
            .resolve_or_register_company_id(&job.company, &norm_comp)
            .await;

        if let Some(existing_id) = self
            .find_existing_duplicate_job_id(
                job,
                &norm_comp,
                company_id,
                &norm_loc,
                &canon_url,
                &enhanced_hash,
                &legacy_hash,
            )
            .await?
        {
            self.upsert_job_source(existing_id, &job.source, &job.external_id, &job.url)
                .await?;

            // Se a nova fonte trouxe uma descrição mais completa, enriquece o texto bruto preservando a vaga
            if job.raw_description.len() > 120 {
                let _ = sqlx::query(
                    r#"
                    UPDATE jobs
                    SET raw_description = CASE
                        WHEN raw_description IS NULL OR length(raw_description) < ?1 THEN ?2
                        ELSE raw_description
                    END,
                    source_status = 'ACTIVE'
                    WHERE id = ?3
                    "#,
                )
                .bind(job.raw_description.len() as i64)
                .bind(&job.raw_description)
                .bind(existing_id)
                .execute(&self.pool)
                .await;
            }

            return Ok(false);
        }

        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO jobs (
                dedup_hash, external_id, source, title, company, location, url, raw_description, published_at,
                normalized_company, normalized_title, canonical_url, company_id, source_status
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, 'ACTIVE')
            "#,
        )
        .bind(&enhanced_hash)
        .bind(&job.external_id)
        .bind(&job.source)
        .bind(&job.title)
        .bind(&job.company)
        .bind(&job.location)
        .bind(&job.url)
        .bind(&job.raw_description)
        .bind(&job.published_at)
        .bind(&norm_comp)
        .bind(&norm_title)
        .bind(&canon_url)
        .bind(company_id)
        .execute(&self.pool)
        .await
        .context("Erro ao inserir vaga no SQLite")?;

        if result.rows_affected() > 0 {
            let new_id = result.last_insert_rowid();
            self.upsert_job_source(new_id, &job.source, &job.external_id, &job.url)
                .await?;
            let _ = self.screen_job_by_id(new_id).await;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Varre todas as vagas existentes no banco de dados e consolida duplicatas entre fontes distintas
    /// (ex: mesma vaga vinda do Indeed, Glassdoor e LinkedIn), transferindo os registros de `job_sources`
    /// para a vaga canônica e deletando registros redundantes. Retorna `(duplicatas_unificadas, vagas_restantes)`.
    pub async fn consolidate_duplicate_jobs(&self) -> Result<(usize, usize)> {
        let rows = sqlx::query(
            "SELECT id, title, company, location, is_enriched, match_score, raw_description FROM jobs ORDER BY is_enriched DESC, (match_score IS NOT NULL) DESC, id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Erro ao carregar vagas para consolidação de duplicatas")?;

        let mut consolidated_count = 0;
        let mut deleted_ids = std::collections::HashSet::new();

        for i in 0..rows.len() {
            let primary_id: i64 = rows[i].try_get("id")?;
            if deleted_ids.contains(&primary_id) {
                continue;
            }

            let primary_title: String = rows[i]
                .try_get::<Option<String>, _>("title")?
                .unwrap_or_default();
            let primary_comp: String = rows[i]
                .try_get::<Option<String>, _>("company")?
                .unwrap_or_default();
            let primary_loc: String = rows[i]
                .try_get::<Option<String>, _>("location")?
                .unwrap_or_default();
            let norm_primary_comp = normalize_company_name(&primary_comp);

            for j in (i + 1)..rows.len() {
                let dup_id: i64 = rows[j].try_get("id")?;
                if deleted_ids.contains(&dup_id) {
                    continue;
                }

                let dup_title: String = rows[j]
                    .try_get::<Option<String>, _>("title")?
                    .unwrap_or_default();
                let dup_comp: String = rows[j]
                    .try_get::<Option<String>, _>("company")?
                    .unwrap_or_default();
                let dup_loc: String = rows[j]
                    .try_get::<Option<String>, _>("location")?
                    .unwrap_or_default();
                let norm_dup_comp = normalize_company_name(&dup_comp);

                let same_company = norm_primary_comp == norm_dup_comp
                    || (!norm_primary_comp.is_empty()
                        && norm_primary_comp != "empresa n d"
                        && ((norm_primary_comp.contains(&norm_dup_comp) && norm_dup_comp.len() >= 4)
                            || (norm_dup_comp.contains(&norm_primary_comp) && norm_primary_comp.len() >= 4)));

                if same_company
                    && locations_are_compatible(&primary_loc, &dup_loc)
                    && titles_are_near_duplicates(&primary_title, &dup_title)
                {
                    // Transfere origens de job_sources da duplicata para a vaga principal
                    let _ = sqlx::query(
                        "UPDATE OR IGNORE job_sources SET job_id = ?1 WHERE job_id = ?2",
                    )
                    .bind(primary_id)
                    .bind(dup_id)
                    .execute(&self.pool)
                    .await;

                    // Limpa quaisquer sobras de job_sources da duplicata
                    let _ = sqlx::query("DELETE FROM job_sources WHERE job_id = ?1")
                        .bind(dup_id)
                        .execute(&self.pool)
                        .await;

                    // Remove a vaga duplicada
                    let _ = sqlx::query("DELETE FROM jobs WHERE id = ?1")
                        .bind(dup_id)
                        .execute(&self.pool)
                        .await;

                    deleted_ids.insert(dup_id);
                    consolidated_count += 1;
                }
            }
        }

        let remaining: i64 = sqlx::query("SELECT COUNT(*) as cnt FROM jobs")
            .fetch_one(&self.pool)
            .await
            .and_then(|r| r.try_get("cnt"))
            .unwrap_or(0);

        Ok((consolidated_count, remaining as usize))
    }

    /// Insere um lote de vagas brutas deduplicadas.
    /// Retorna `(inseridas, duplicadas_ignoradas)`.
    pub async fn insert_raw_jobs_batch(&self, jobs: &[RawJobListing]) -> Result<(usize, usize)> {
        let mut inserted = 0usize;
        let mut skipped = 0usize;

        for job in jobs {
            if self.insert_raw_job(job).await? {
                inserted += 1;
            } else {
                skipped += 1;
            }
        }

        Ok((inserted, skipped))
    }

    pub async fn get_unenriched_jobs(&self, limit: i64) -> Result<Vec<JobRecord>> {
        let sql = format!(
            "SELECT {} FROM jobs WHERE is_enriched = 0 AND COALESCE(screening_verdict, 'PASS') != 'REJECT' ORDER BY id ASC LIMIT ?1",
            JOB_SELECT_COLUMNS
        );
        let rows = sqlx::query_as::<_, JobRecord>(&sql)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .context("Erro ao buscar vagas pendentes de enriquecimento")?;

        Ok(rows)
    }

    pub async fn update_job_enrichment(
        &self,
        job_id: i64,
        meta: &ExtractedJobMetadata,
        embedding: Option<&[f32]>,
    ) -> Result<()> {
        let skills_json =
            serde_json::to_string(&meta.required_skills).unwrap_or_else(|_| "[]".to_string());
        let red_flags_json =
            serde_json::to_string(&meta.red_flags).unwrap_or_else(|_| "[]".to_string());
        let embedding_json = embedding.and_then(|vec| serde_json::to_string(vec).ok());
        let dossier_json = serde_json::to_string(&meta.dossier).ok();

        sqlx::query(
            r#"
            UPDATE jobs
            SET is_enriched = 1,
                work_model = ?1,
                contract_type = ?2,
                seniority = ?3,
                shift_schedule = ?4,
                salary_estimate_brl = ?5,
                required_skills_json = ?6,
                red_flags_json = ?7,
                ai_summary = ?8,
                embedding_json = ?9,
                dossier_json = ?10
            WHERE id = ?11
            "#,
        )
        .bind(&meta.work_model)
        .bind(&meta.contract_type)
        .bind(&meta.seniority)
        .bind(&meta.shift_schedule)
        .bind(&meta.salary_estimate_brl)
        .bind(&skills_json)
        .bind(&red_flags_json)
        .bind(&meta.summary)
        .bind(&embedding_json)
        .bind(&dossier_json)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("Erro ao atualizar enriquecimento da vaga ID {}", job_id))?;

        let _ = self.screen_job_by_id(job_id).await;
        Ok(())
    }

    pub async fn update_job_match(
        &self,
        job_id: i64,
        analysis: &MatchAnalysisResult,
    ) -> Result<()> {
        let formatted_md = format!(
            "## Veredito de Escala e Localização\n{}\n\n## Skills Compatíveis\n{}\n\n## Gaps / Skills Ausentes\n{}\n\n## Sugestões de Otimização ATS\n{}\n\n## Análise Detalhada\n{}",
            analysis.shift_and_location_verdict,
            if analysis.matching_skills.is_empty() {
                "- Nenhuma identificada especificamente".to_string()
            } else {
                analysis
                    .matching_skills
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if analysis.missing_skills.is_empty() {
                "- Nenhum gap crítico identificado".to_string()
            } else {
                analysis
                    .missing_skills
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            if analysis.ats_suggestions.is_empty() {
                "- Sem sugestões adicionais".to_string()
            } else {
                analysis
                    .ats_suggestions
                    .iter()
                    .map(|s| format!("- {}", s))
                    .collect::<Vec<_>>()
                    .join("\n")
            },
            analysis.detailed_analysis_markdown
        );

        let fit_json = serde_json::to_string(analysis).ok();

        sqlx::query(
            r#"
            UPDATE jobs
            SET match_score = ?1,
                match_analysis_md = ?2,
                fit_json = ?3
            WHERE id = ?4
            "#,
        )
        .bind(analysis.match_score as f64)
        .bind(&formatted_md)
        .bind(&fit_json)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("Erro ao salvar análise de match para a vaga ID {}", job_id))?;

        let _ = self.screen_job_by_id(job_id).await;
        Ok(())
    }

    pub async fn update_job_status(&self, job_id: i64, status: &str) -> Result<()> {
        sqlx::query("UPDATE jobs SET application_status = ?1 WHERE id = ?2")
            .bind(status)
            .bind(job_id)
            .execute(&self.pool)
            .await
            .with_context(|| format!("Erro ao atualizar status da vaga ID {}", job_id))?;
        Ok(())
    }

    pub async fn get_job_by_id(&self, job_id: i64) -> Result<Option<JobRecord>> {
        let sql = format!("SELECT {} FROM jobs WHERE id = ?1", JOB_SELECT_COLUMNS);
        let record = sqlx::query_as::<_, JobRecord>(&sql)
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("Erro ao buscar vaga ID {}", job_id))?;
        Ok(record)
    }

    pub async fn get_job_sources(&self, job_id: i64) -> Result<Vec<JobSourceRecord>> {
        let rows = sqlx::query_as::<_, JobSourceRecord>(
            "SELECT id, job_id, source, external_id, original_url, first_seen_at, last_seen_at, source_status FROM job_sources WHERE job_id = ?1 ORDER BY first_seen_at ASC",
        )
        .bind(job_id)
        .fetch_all(&self.pool)
        .await
        .context("Erro ao listar origens da vaga")?;
        Ok(rows)
    }

    pub async fn list_jobs(&self, filter: &JobFilter) -> Result<Vec<JobRecord>> {
        let sql = format!(
            "SELECT {} FROM jobs ORDER BY COALESCE(match_score, -1.0) DESC, COALESCE(screening_score, -1.0) DESC, is_enriched DESC, id DESC",
            JOB_SELECT_COLUMNS
        );
        let all_jobs = sqlx::query_as::<_, JobRecord>(&sql)
            .fetch_all(&self.pool)
            .await
            .context("Erro ao listar vagas do SQLite")?;

        let filtered = all_jobs
            .into_iter()
            .filter(|job| {
                if filter.only_enriched && job.is_enriched == 0 {
                    return false;
                }
                if let Some(comp_id) = filter.company_id {
                    if job.company_id != Some(comp_id) {
                        return false;
                    }
                }
                if let Some(ref sv) = filter.screening_verdict {
                    if !sv.is_empty() && !sv.eq_ignore_ascii_case("Todas") {
                        let job_sv = job.screening_verdict.as_deref().unwrap_or("");
                        if !job_sv.eq_ignore_ascii_case(sv) {
                            return false;
                        }
                    }
                }
                if let Some(ref wm) = filter.work_model {
                    if !wm.is_empty() && !wm.eq_ignore_ascii_case("Todas") {
                        let job_wm = job.work_model.as_deref().unwrap_or("");
                        if !job_wm.eq_ignore_ascii_case(wm) {
                            return false;
                        }
                    }
                }
                if let Some(ref sen) = filter.seniority {
                    if !sen.is_empty() {
                        let job_sen = job.seniority.as_deref().unwrap_or("");
                        if !job_sen.eq_ignore_ascii_case(sen) {
                            return false;
                        }
                    }
                }
                if let Some(ref shift) = filter.shift {
                    if !shift.is_empty() {
                        let job_shift = job.shift_schedule.as_deref().unwrap_or("").to_lowercase();
                        if !job_shift.contains(&shift.to_lowercase()) {
                            return false;
                        }
                    }
                }
                if let Some(ref st) = filter.status {
                    if !st.is_empty() {
                        let job_st = job.application_status.as_deref().unwrap_or("Descoberta");
                        if !job_st.eq_ignore_ascii_case(st) {
                            return false;
                        }
                    }
                }
                if let Some(ref src) = filter.source {
                    if !src.is_empty() && !src.eq_ignore_ascii_case("Todas") {
                        let job_src = job.source.as_deref().unwrap_or("").to_lowercase();
                        let target = src.to_lowercase();
                        if !job_src.contains(&target) {
                            return false;
                        }
                    }
                }
                if let Some(ref q) = filter.query {
                    if !q.trim().is_empty() {
                        let needle = q.to_lowercase();
                        let hay_title = job.title.as_deref().unwrap_or("").to_lowercase();
                        let hay_comp = job.company.as_deref().unwrap_or("").to_lowercase();
                        let hay_loc = job.location.as_deref().unwrap_or("").to_lowercase();
                        let hay_sum = job.ai_summary.as_deref().unwrap_or("").to_lowercase();
                        if !hay_title.contains(&needle)
                            && !hay_comp.contains(&needle)
                            && !hay_loc.contains(&needle)
                            && !hay_sum.contains(&needle)
                        {
                            return false;
                        }
                    }
                }
                true
            })
            .collect();

        Ok(filtered)
    }

    pub async fn rank_jobs_by_embedding(
        &self,
        resume_embedding: &[f32],
        top_k: usize,
    ) -> Result<Vec<(JobRecord, f32)>> {
        let sql = format!(
            "SELECT {} FROM jobs WHERE is_enriched = 1 AND embedding_json IS NOT NULL",
            JOB_SELECT_COLUMNS
        );
        let jobs = sqlx::query_as::<_, JobRecord>(&sql)
            .fetch_all(&self.pool)
            .await
            .context("Erro ao carregar vagas enriquecidas para ranking semântico")?;

        let mut scored: Vec<(JobRecord, f32)> = jobs
            .into_iter()
            .filter_map(|job| {
                let emb = job.embedding()?;
                let sim = cosine_similarity(resume_embedding, &emb);
                Some((job, sim))
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);

        Ok(scored)
    }

    pub async fn get_stats(&self) -> Result<DbStats> {
        let row = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_jobs,
                SUM(CASE WHEN is_enriched = 1 THEN 1 ELSE 0 END) as enriched_jobs,
                SUM(CASE WHEN application_status IN ('Salva', 'Aplicada', 'Entrevista') THEN 1 ELSE 0 END) as saved_or_applied_jobs
            FROM jobs
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .context("Erro ao consultar estatísticas do banco")?;

        let total_jobs: i64 = row.try_get("total_jobs").unwrap_or(0);
        let enriched_jobs: i64 = row
            .try_get::<Option<i64>, _>("enriched_jobs")
            .ok()
            .flatten()
            .unwrap_or(0);
        let saved_or_applied_jobs: i64 = row
            .try_get::<Option<i64>, _>("saved_or_applied_jobs")
            .ok()
            .flatten()
            .unwrap_or(0);

        Ok(DbStats {
            total_jobs,
            enriched_jobs,
            saved_or_applied_jobs,
        })
    }

    pub async fn get_candidate_profile(&self) -> Result<CandidateProfile> {
        let profile = sqlx::query_as::<_, CandidateProfile>(
            "SELECT id, base_city, accepted_region, accept_remote, accepted_shifts, min_salary_brl, exclude_terms, auto_enrich, auto_match_threshold, notify_on_apply FROM candidate_profile WHERE id = 1",
        )
        .fetch_optional(&self.pool)
        .await
        .context("Erro ao ler perfil de critérios do candidato")?
        .unwrap_or_default();

        Ok(profile)
    }

    pub async fn update_candidate_profile(&self, p: &CandidateProfile) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE candidate_profile
            SET base_city = ?1,
                accepted_region = ?2,
                accept_remote = ?3,
                accepted_shifts = ?4,
                min_salary_brl = ?5,
                exclude_terms = ?6,
                auto_enrich = ?7,
                auto_match_threshold = ?8,
                notify_on_apply = ?9
            WHERE id = 1
            "#,
        )
        .bind(&p.base_city)
        .bind(&p.accepted_region)
        .bind(p.accept_remote)
        .bind(&p.accepted_shifts)
        .bind(&p.min_salary_brl)
        .bind(&p.exclude_terms)
        .bind(p.auto_enrich)
        .bind(p.auto_match_threshold)
        .bind(p.notify_on_apply)
        .execute(&self.pool)
        .await
        .context("Erro ao salvar perfil de critérios do candidato")?;
        Ok(())
    }

    pub async fn list_saved_searches(&self) -> Result<Vec<SavedSearch>> {
        let rows = sqlx::query_as::<_, SavedSearch>(
            "SELECT id, query, location, remote, enabled, interval_minutes, last_run_at, last_found_count FROM saved_searches ORDER BY id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Erro ao listar rondas de busca salvas")?;
        Ok(rows)
    }

    pub async fn get_due_saved_searches(&self) -> Result<Vec<SavedSearch>> {
        let rows = sqlx::query_as::<_, SavedSearch>(
            r#"
            SELECT id, query, location, remote, enabled, interval_minutes, last_run_at, last_found_count
            FROM saved_searches
            WHERE enabled = 1
              AND (
                last_run_at IS NULL
                OR datetime(last_run_at, '+' || interval_minutes || ' minutes') <= datetime('now')
              )
            ORDER BY id ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Erro ao buscar rondas pendentes de execução")?;
        Ok(rows)
    }

    pub async fn insert_saved_search(
        &self,
        query: &str,
        location: &str,
        remote: bool,
        interval_minutes: i64,
    ) -> Result<i64> {
        let res = sqlx::query(
            "INSERT INTO saved_searches (query, location, remote, enabled, interval_minutes) VALUES (?1, ?2, ?3, 1, ?4)",
        )
        .bind(query.trim())
        .bind(location.trim())
        .bind(if remote { 1_i64 } else { 0_i64 })
        .bind(interval_minutes.clamp(15, 1440))
        .execute(&self.pool)
        .await
        .context("Erro ao cadastrar ronda de busca automática")?;
        Ok(res.last_insert_rowid())
    }

    pub async fn toggle_saved_search(&self, id: i64, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE saved_searches SET enabled = ?1 WHERE id = ?2")
            .bind(if enabled { 1_i64 } else { 0_i64 })
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Erro ao alternar ronda de busca automática")?;
        Ok(())
    }

    pub async fn delete_saved_search(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM saved_searches WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .context("Erro ao excluir ronda de busca automática")?;
        Ok(())
    }

    pub async fn mark_saved_search_ran(&self, id: i64, found_count: usize) -> Result<()> {
        sqlx::query(
            "UPDATE saved_searches SET last_run_at = datetime('now'), last_found_count = ?1 WHERE id = ?2",
        )
        .bind(found_count as i64)
        .bind(id)
        .execute(&self.pool)
        .await
        .context("Erro ao registrar execução da ronda de busca")?;
        Ok(())
    }

    pub async fn get_unmatched_enriched_jobs(&self, limit: i64) -> Result<Vec<JobRecord>> {
        let sql = format!(
            "SELECT {} FROM jobs WHERE is_enriched = 1 AND match_score IS NULL AND embedding_json IS NOT NULL AND COALESCE(application_status, 'Descoberta') != 'Descartada' AND COALESCE(screening_verdict, 'PASS') != 'REJECT' ORDER BY id DESC LIMIT ?1",
            JOB_SELECT_COLUMNS
        );
        let rows = sqlx::query_as::<_, JobRecord>(&sql)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
            .context("Erro ao buscar vagas enriquecidas sem match")?;
        Ok(rows)
    }

    // ==================================================================
    // Sprint 09: Gerenciamento de Coletores
    // ==================================================================

    pub async fn list_collectors(&self) -> Result<Vec<CollectorConfig>> {
        let rows = sqlx::query_as::<_, CollectorConfig>(
            "SELECT source_id, display_name, group_name, enabled, timeout_secs, default_limit, strategy_notes, last_status, last_run_at, last_count, last_error FROM collector_configs ORDER BY group_name ASC, source_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .context("Erro ao listar configurações de coletores")?;
        Ok(rows)
    }

    pub async fn toggle_collector(&self, source_id: &str, enabled: bool) -> Result<()> {
        sqlx::query("UPDATE collector_configs SET enabled = ?1 WHERE source_id = ?2")
            .bind(if enabled { 1_i64 } else { 0_i64 })
            .bind(source_id)
            .execute(&self.pool)
            .await
            .context("Erro ao alterar estado do coletor")?;
        Ok(())
    }

    pub async fn update_collector_run_status(
        &self,
        source_id: &str,
        status: &str,
        count: usize,
        error: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE collector_configs
            SET last_status = ?1,
                last_run_at = datetime('now'),
                last_count = ?2,
                last_error = ?3
            WHERE source_id = ?4
            "#,
        )
        .bind(status)
        .bind(count as i64)
        .bind(error)
        .bind(source_id)
        .execute(&self.pool)
        .await
        .ok();
        Ok(())
    }

    // ==================================================================
    // Sprint 12: Triagem Determinística + Explicabilidade
    // ==================================================================

    pub async fn get_preferred_and_excluded_companies(&self) -> (Vec<String>, Vec<String>) {
        let rows = sqlx::query(
            "SELECT normalized_name, is_preferred, is_excluded FROM companies WHERE is_preferred = 1 OR is_excluded = 1",
        )
        .fetch_all(&self.pool)
        .await
        .unwrap_or_default();

        let mut preferred = Vec::new();
        let mut excluded = Vec::new();
        for r in rows {
            let norm: String = r.try_get("normalized_name").unwrap_or_default();
            let pref: i64 = r.try_get("is_preferred").unwrap_or(0);
            let excl: i64 = r.try_get("is_excluded").unwrap_or(0);
            if excl == 1 {
                excluded.push(norm);
            } else if pref == 1 {
                preferred.push(norm);
            }
        }
        (preferred, excluded)
    }

    pub async fn screen_job_by_id(&self, job_id: i64) -> Result<Option<ScreeningResult>> {
        let Some(job) = self.get_job_by_id(job_id).await? else {
            return Ok(None);
        };
        let profile = self.get_candidate_profile().await.unwrap_or_default();
        let (preferred, excluded) = self.get_preferred_and_excluded_companies().await;

        let res = evaluate_job_screening(&job, &profile, &preferred, &excluded);
        let json_str = serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string());

        sqlx::query(
            "UPDATE jobs SET screening_verdict = ?1, screening_score = ?2, screening_json = ?3 WHERE id = ?4",
        )
        .bind(&res.verdict)
        .bind(res.score)
        .bind(&json_str)
        .bind(job_id)
        .execute(&self.pool)
        .await?;

        Ok(Some(res))
    }

    pub async fn screen_all_jobs(&self) -> Result<usize> {
        let profile = self.get_candidate_profile().await.unwrap_or_default();
        let (preferred, excluded) = self.get_preferred_and_excluded_companies().await;

        let sql = format!("SELECT {} FROM jobs", JOB_SELECT_COLUMNS);
        let jobs = sqlx::query_as::<_, JobRecord>(&sql)
            .fetch_all(&self.pool)
            .await?;

        let mut count = 0usize;
        for job in &jobs {
            let res = evaluate_job_screening(job, &profile, &preferred, &excluded);
            let json_str = serde_json::to_string(&res).unwrap_or_else(|_| "{}".to_string());
            sqlx::query(
                "UPDATE jobs SET screening_verdict = ?1, screening_score = ?2, screening_json = ?3 WHERE id = ?4",
            )
            .bind(&res.verdict)
            .bind(res.score)
            .bind(&json_str)
            .bind(job.id)
            .execute(&self.pool)
            .await?;
            count += 1;
        }
        Ok(count)
    }

    // ==================================================================
    // Sprint 13: Cache de IA e Telemetria de Operações/Tokens
    // ==================================================================

    pub async fn get_ai_cache(&self, cache_key: &str) -> Result<Option<String>> {
        let row = sqlx::query("SELECT response_json FROM ai_cache WHERE cache_key = ?1")
            .bind(cache_key)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(r) = row {
            let json_str: String = r.try_get("response_json")?;
            let _ = sqlx::query(
                "UPDATE ai_cache SET hit_count = hit_count + 1, last_hit_at = datetime('now') WHERE cache_key = ?1",
            )
            .bind(cache_key)
            .execute(&self.pool)
            .await;
            Ok(Some(json_str))
        } else {
            Ok(None)
        }
    }

    pub async fn put_ai_cache(
        &self,
        cache_key: &str,
        operation_type: &str,
        model_used: &str,
        response_json: &str,
        prompt_tokens: i64,
        completion_tokens: i64,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO ai_cache
            (cache_key, operation_type, model_used, response_json, prompt_tokens, completion_tokens, hit_count, created_at, last_hit_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, datetime('now'), datetime('now'))
            "#,
        )
        .bind(cache_key)
        .bind(operation_type)
        .bind(model_used)
        .bind(response_json)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn record_ai_operation(
        &self,
        execution_id: Option<&str>,
        operation_type: &str,
        model_used: &str,
        fallback_used: bool,
        attempts: i64,
        status: &str,
        records_count: usize,
        prompt_tokens: i64,
        completion_tokens: i64,
        duration_ms: i64,
        error_message: Option<&str>,
    ) -> Result<()> {
        let total_tokens = prompt_tokens + completion_tokens;
        sqlx::query(
            r#"
            INSERT INTO ai_operations
            (execution_id, operation_type, model_used, fallback_used, attempts, status, records_count, prompt_tokens, completion_tokens, total_tokens, duration_ms, error_message, started_at, completed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, datetime('now'), datetime('now'))
            "#,
        )
        .bind(execution_id)
        .bind(operation_type)
        .bind(model_used)
        .bind(if fallback_used { 1_i64 } else { 0_i64 })
        .bind(attempts)
        .bind(status)
        .bind(records_count as i64)
        .bind(prompt_tokens)
        .bind(completion_tokens)
        .bind(total_tokens)
        .bind(duration_ms)
        .bind(error_message)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_ai_telemetry_summary(&self) -> Result<AiTelemetrySummary> {
        let agg = sqlx::query(
            r#"
            SELECT
                COUNT(*) as total_ops,
                COALESCE(SUM(prompt_tokens), 0) as sum_prompt,
                COALESCE(SUM(completion_tokens), 0) as sum_comp,
                COALESCE(SUM(total_tokens), 0) as sum_total,
                COALESCE(SUM(CASE WHEN fallback_used = 1 THEN 1 ELSE 0 END), 0) as fallbacks,
                COALESCE(SUM(CASE WHEN status = 'ERROR' THEN 1 ELSE 0 END), 0) as errors,
                COALESCE(AVG(duration_ms), 0.0) as avg_dur
            FROM ai_operations
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        let cache_agg = sqlx::query(
            "SELECT COUNT(*) as entries, COALESCE(SUM(hit_count), 0) as hits FROM ai_cache",
        )
        .fetch_one(&self.pool)
        .await?;

        let recent_operations = sqlx::query_as::<_, AiOperationRecord>(
            r#"
            SELECT id, execution_id, operation_type, model_used, fallback_used, attempts, status, records_count, prompt_tokens, completion_tokens, total_tokens, duration_ms, error_message, started_at, completed_at
            FROM ai_operations
            ORDER BY id DESC
            LIMIT 30
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(AiTelemetrySummary {
            total_operations: agg.try_get("total_ops").unwrap_or(0),
            total_prompt_tokens: agg.try_get("sum_prompt").unwrap_or(0),
            total_completion_tokens: agg.try_get("sum_comp").unwrap_or(0),
            total_tokens: agg.try_get("sum_total").unwrap_or(0),
            cache_entries: cache_agg.try_get("entries").unwrap_or(0),
            total_cache_hits: cache_agg.try_get("hits").unwrap_or(0),
            fallback_count: agg.try_get("fallbacks").unwrap_or(0),
            error_count: agg.try_get("errors").unwrap_or(0),
            avg_duration_ms: agg.try_get("avg_dur").unwrap_or(0.0),
            recent_operations,
        })
    }
}
