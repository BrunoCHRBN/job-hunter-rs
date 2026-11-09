use std::str::FromStr;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::Row;

use crate::models::{
    DbStats, ExtractedJobMetadata, JobFilter, JobRecord, MatchAnalysisResult, RawJobListing,
};

const MIGRATION_001: &str = include_str!("../migrations/001_init.sql");

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

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Insere uma vaga bruta caso seu `dedup_hash` ainda não exista.
    /// Retorna `Ok(true)` se a vaga foi inserida ou `Ok(false)` se foi ignorada por duplicidade.
    pub async fn insert_raw_job(&self, job: &RawJobListing) -> Result<bool> {
        let dedup_hash = compute_dedup_hash(&job.company, &job.title, &job.location);

        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO jobs (
                dedup_hash, external_id, source, title, company, location, url, raw_description, published_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
        )
        .bind(&dedup_hash)
        .bind(&job.external_id)
        .bind(&job.source)
        .bind(&job.title)
        .bind(&job.company)
        .bind(&job.location)
        .bind(&job.url)
        .bind(&job.raw_description)
        .bind(&job.published_at)
        .execute(&self.pool)
        .await
        .context("Erro ao inserir vaga no SQLite")?;

        Ok(result.rows_affected() > 0)
    }

    /// Insere um lote de vagas brutas deduplicadas por hash blake3.
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
        let rows = sqlx::query_as::<_, JobRecord>(
            r#"
            SELECT * FROM jobs
            WHERE is_enriched = 0
            ORDER BY id ASC
            LIMIT ?1
            "#,
        )
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
        let skills_json = serde_json::to_string(&meta.required_skills)
            .unwrap_or_else(|_| "[]".to_string());
        let red_flags_json = serde_json::to_string(&meta.red_flags)
            .unwrap_or_else(|_| "[]".to_string());
        let embedding_json = embedding
            .and_then(|vec| serde_json::to_string(vec).ok());

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
                embedding_json = ?9
            WHERE id = ?10
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
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("Erro ao atualizar enriquecimento da vaga ID {}", job_id))?;

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

        sqlx::query(
            r#"
            UPDATE jobs
            SET match_score = ?1,
                match_analysis_md = ?2
            WHERE id = ?3
            "#,
        )
        .bind(analysis.match_score as f64)
        .bind(&formatted_md)
        .bind(job_id)
        .execute(&self.pool)
        .await
        .with_context(|| format!("Erro ao salvar análise de match para a vaga ID {}", job_id))?;

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
        let record = sqlx::query_as::<_, JobRecord>("SELECT * FROM jobs WHERE id = ?1")
            .bind(job_id)
            .fetch_optional(&self.pool)
            .await
            .with_context(|| format!("Erro ao buscar vaga ID {}", job_id))?;
        Ok(record)
    }

    pub async fn list_jobs(&self, filter: &JobFilter) -> Result<Vec<JobRecord>> {
        let all_jobs = sqlx::query_as::<_, JobRecord>(
            r#"
            SELECT * FROM jobs
            ORDER BY COALESCE(match_score, -1.0) DESC, is_enriched DESC, id DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Erro ao listar vagas do SQLite")?;

        let filtered = all_jobs
            .into_iter()
            .filter(|job| {
                if filter.only_enriched && job.is_enriched == 0 {
                    return false;
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
        let jobs = sqlx::query_as::<_, JobRecord>(
            r#"
            SELECT * FROM jobs
            WHERE is_enriched = 1 AND embedding_json IS NOT NULL
            "#,
        )
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
}
