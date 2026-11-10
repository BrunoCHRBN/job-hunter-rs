use std::collections::HashSet;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::collectors::ats::fetch_company_ats_jobs;
use crate::collectors::brasil::{SourceDiagnosticReport, SourceDiagnosticStatus};
use crate::db::{Database, JOB_SELECT_COLUMNS};
use crate::execution::ExecutionHandle;
use crate::models::{
    CompanyAliasRecord, CompanyRecord, CompanySourceRecord, JobRecord,
};
use crate::normalizer::normalize_company_name;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyDetailsView {
    pub company: CompanyRecord,
    pub aliases: Vec<CompanyAliasRecord>,
    pub sources: Vec<CompanySourceRecord>,
    pub jobs: Vec<JobRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyScanReport {
    pub company_id: i64,
    pub company_name: String,
    pub sources_checked: usize,
    pub total_fetched: usize,
    pub newly_inserted: usize,
    pub duplicates_updated: usize,
    pub closed_detected: usize,
    pub diagnostics: Vec<SourceDiagnosticReport>,
}

impl Database {
    pub async fn list_companies(&self, only_monitored: bool) -> Result<Vec<CompanyRecord>> {
        let where_clause = if only_monitored {
            "WHERE c.is_monitored = 1"
        } else {
            ""
        };
        let sql = format!(
            r#"
            SELECT
                c.id, c.official_name, c.normalized_name, c.website, c.segment, c.location,
                c.description, c.is_monitored, c.is_preferred, c.is_excluded, c.created_at, c.updated_at,
                (SELECT COUNT(*) FROM jobs j WHERE j.company_id = c.id AND COALESCE(j.source_status, 'ACTIVE') = 'ACTIVE') as active_jobs_count
            FROM companies c
            {}
            ORDER BY c.is_monitored DESC, c.is_preferred DESC, active_jobs_count DESC, c.official_name ASC
            "#,
            where_clause
        );
        let rows = sqlx::query_as::<_, CompanyRecord>(&sql)
            .fetch_all(self.pool())
            .await
            .context("Erro ao listar empresas")?;
        Ok(rows)
    }

    /// Retorna IDs e nomes de empresas monitoradas que possuem fontes prontas para verificação (Sprint 15).
    pub async fn get_due_monitored_companies(&self, limit: usize) -> Result<Vec<(i64, String)>> {
        let sql = r#"
            SELECT DISTINCT c.id, c.official_name
            FROM companies c
            JOIN company_sources cs ON cs.company_id = c.id
            WHERE c.is_monitored = 1
              AND (cs.next_check_at IS NULL OR cs.next_check_at <= datetime('now'))
            LIMIT ?1
        "#;
        let rows = sqlx::query(sql)
            .bind(limit as i64)
            .fetch_all(self.pool())
            .await?;
        let res = rows
            .into_iter()
            .map(|r| (r.get::<i64, _>("id"), r.get::<String, _>("official_name")))
            .collect();
        Ok(res)
    }

    pub async fn get_company_details(&self, company_id: i64) -> Result<Option<CompanyDetailsView>> {
        let sql = r#"
            SELECT
                c.id, c.official_name, c.normalized_name, c.website, c.segment, c.location,
                c.description, c.is_monitored, c.is_preferred, c.is_excluded, c.created_at, c.updated_at,
                (SELECT COUNT(*) FROM jobs j WHERE j.company_id = c.id AND COALESCE(j.source_status, 'ACTIVE') = 'ACTIVE') as active_jobs_count
            FROM companies c
            WHERE c.id = ?1
        "#;
        let Some(company) = sqlx::query_as::<_, CompanyRecord>(sql)
            .bind(company_id)
            .fetch_optional(self.pool())
            .await?
        else {
            return Ok(None);
        };

        let aliases = sqlx::query_as::<_, CompanyAliasRecord>(
            "SELECT id, company_id, alias_name, normalized_alias FROM company_aliases WHERE company_id = ?1 ORDER BY alias_name ASC",
        )
        .bind(company_id)
        .fetch_all(self.pool())
        .await?;

        let sources = sqlx::query_as::<_, CompanySourceRecord>(
            "SELECT id, company_id, source_type, career_url, ats_provider, portal_id, integration_status, last_checked_at, next_check_at, last_found_count, last_error FROM company_sources WHERE company_id = ?1 ORDER BY id ASC",
        )
        .bind(company_id)
        .fetch_all(self.pool())
        .await?;

        let jobs_sql = format!(
            "SELECT {} FROM jobs WHERE company_id = ?1 OR normalized_company = ?2 ORDER BY id DESC",
            JOB_SELECT_COLUMNS
        );
        let jobs = sqlx::query_as::<_, JobRecord>(&jobs_sql)
            .bind(company_id)
            .bind(&company.normalized_name)
            .fetch_all(self.pool())
            .await?;

        Ok(Some(CompanyDetailsView {
            company,
            aliases,
            sources,
            jobs,
        }))
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_company(
        &self,
        official_name: &str,
        website: &str,
        segment: &str,
        location: &str,
        description: &str,
        is_monitored: bool,
        is_preferred: bool,
        is_excluded: bool,
    ) -> Result<i64> {
        let norm = normalize_company_name(official_name);
        sqlx::query(
            r#"
            INSERT INTO companies
            (official_name, normalized_name, website, segment, location, description, is_monitored, is_preferred, is_excluded, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, datetime('now'))
            ON CONFLICT(normalized_name) DO UPDATE SET
                official_name = excluded.official_name,
                website = excluded.website,
                segment = excluded.segment,
                location = excluded.location,
                description = excluded.description,
                is_monitored = excluded.is_monitored,
                is_preferred = excluded.is_preferred,
                is_excluded = excluded.is_excluded,
                updated_at = datetime('now')
            "#,
        )
        .bind(official_name.trim())
        .bind(&norm)
        .bind(website.trim())
        .bind(segment.trim())
        .bind(location.trim())
        .bind(description.trim())
        .bind(if is_monitored { 1_i64 } else { 0_i64 })
        .bind(if is_preferred { 1_i64 } else { 0_i64 })
        .bind(if is_excluded { 1_i64 } else { 0_i64 })
        .execute(self.pool())
        .await?;

        let row = sqlx::query("SELECT id FROM companies WHERE normalized_name = ?1")
            .bind(&norm)
            .fetch_one(self.pool())
            .await?;
        let company_id: i64 = row.try_get("id")?;

        // Vincula vagas já existentes com mesmo normalized_company
        let _ = sqlx::query("UPDATE jobs SET company_id = ?1 WHERE normalized_company = ?2")
            .bind(company_id)
            .bind(&norm)
            .execute(self.pool())
            .await;

        Ok(company_id)
    }

    pub async fn add_company_alias(&self, company_id: i64, alias_name: &str) -> Result<i64> {
        let norm_alias = normalize_company_name(alias_name);
        let res = sqlx::query(
            "INSERT OR IGNORE INTO company_aliases (company_id, alias_name, normalized_alias) VALUES (?1, ?2, ?3)",
        )
        .bind(company_id)
        .bind(alias_name.trim())
        .bind(&norm_alias)
        .execute(self.pool())
        .await?;

        // Vincula vagas que usavam esse alias
        let _ = sqlx::query("UPDATE jobs SET company_id = ?1 WHERE normalized_company = ?2")
            .bind(company_id)
            .bind(&norm_alias)
            .execute(self.pool())
            .await;

        Ok(res.last_insert_rowid())
    }

    pub async fn add_company_source(
        &self,
        company_id: i64,
        source_type: &str,
        career_url: &str,
        ats_provider: &str,
        portal_id: &str,
    ) -> Result<i64> {
        let res = sqlx::query(
            r#"
            INSERT INTO company_sources
            (company_id, source_type, career_url, ats_provider, portal_id, integration_status)
            VALUES (?1, ?2, ?3, ?4, ?5, 'ACTIVE')
            "#,
        )
        .bind(company_id)
        .bind(source_type.trim())
        .bind(career_url.trim())
        .bind(ats_provider.trim().to_lowercase())
        .bind(portal_id.trim())
        .execute(self.pool())
        .await?;
        Ok(res.last_insert_rowid())
    }

    /// Executa a varredura de oportunidades em todas as fontes corporativas de uma empresa.
    /// Regra Crítica (Sprint 11): uma vaga existente NUNCA é marcada como encerrada (`CLOSED`)
    /// caso a consulta à fonte tenha resultado em `ERROR`, `BLOCKED` ou `TIMEOUT`.
    pub async fn scan_company_opportunities(
        &self,
        company_id: i64,
        query_filter: Option<&str>,
        limit: usize,
        exec_handle: Option<&ExecutionHandle>,
    ) -> Result<CompanyScanReport> {
        let Some(details) = self.get_company_details(company_id).await? else {
            anyhow::bail!("Empresa ID {} não encontrada", company_id);
        };

        let company_name = details.company.official_name.clone();
        let mut total_fetched = 0usize;
        let mut newly_inserted = 0usize;
        let mut duplicates_updated = 0usize;
        let mut closed_detected = 0usize;
        let mut diagnostics = Vec::new();

        if let Some(h) = exec_handle {
            h.emit_info(
                "company_scan",
                Some("company_intel"),
                &format!(
                    "Iniciando varredura corporativa para '{}' ({} fontes cadastradas)",
                    company_name,
                    details.sources.len()
                ),
            )
            .await;
        }

        for src in &details.sources {
            let (jobs, diag) = fetch_company_ats_jobs(
                &src.ats_provider,
                &src.portal_id,
                &src.career_url,
                &company_name,
                query_filter,
                limit,
            )
            .await;

            total_fetched += jobs.len();

            let err_msg = if matches!(
                diag.status,
                SourceDiagnosticStatus::Error
                    | SourceDiagnosticStatus::Blocked
                    | SourceDiagnosticStatus::Timeout
            ) {
                Some(diag.message.clone())
            } else {
                None
            };

            let _ = sqlx::query(
                r#"
                UPDATE company_sources
                SET integration_status = ?1,
                    last_checked_at = datetime('now'),
                    next_check_at = datetime('now', '+6 hours'),
                    last_found_count = ?2,
                    last_error = ?3
                WHERE id = ?4
                "#,
            )
            .bind(diag.status.as_str())
            .bind(jobs.len() as i64)
            .bind(err_msg.as_deref())
            .bind(src.id)
            .execute(self.pool())
            .await;

            // Somente se a fonte respondeu com SUCCESS (e sem filtro parcial de query),
            // podemos comparar quais external_ids daquela fonte não constam mais como abertos.
            // Se status for ERROR, BLOCKED ou TIMEOUT, preservamos todas as vagas como ACTIVE!
            if diag.status == SourceDiagnosticStatus::Success
                && query_filter.map(|q| q.trim().is_empty()).unwrap_or(true)
            {
                let active_ext_ids: HashSet<String> =
                    jobs.iter().map(|j| j.external_id.clone()).collect();

                let existing_src_rows = sqlx::query(
                    "SELECT id, job_id, external_id FROM job_sources WHERE source = ?1 AND source_status = 'ACTIVE'",
                )
                .bind(&diag.source_id)
                .fetch_all(self.pool())
                .await
                .unwrap_or_default();

                for row in existing_src_rows {
                    let js_id: i64 = row.try_get("id").unwrap_or_default();
                    let j_id: i64 = row.try_get("job_id").unwrap_or_default();
                    let ext_id: String = row.try_get("external_id").unwrap_or_default();
                    if !ext_id.is_empty() && !active_ext_ids.contains(&ext_id) {
                        let _ = sqlx::query(
                            "UPDATE job_sources SET source_status = 'CLOSED' WHERE id = ?1",
                        )
                        .bind(js_id)
                        .execute(self.pool())
                        .await;

                        // Se todas as origens dessa vaga estiverem CLOSED, atualiza source_status da vaga
                        let remaining_active: i64 = sqlx::query(
                            "SELECT COUNT(*) as cnt FROM job_sources WHERE job_id = ?1 AND source_status = 'ACTIVE'",
                        )
                        .bind(j_id)
                        .fetch_one(self.pool())
                        .await
                        .and_then(|r| r.try_get("cnt"))
                        .unwrap_or(1);

                        if remaining_active == 0 {
                            let _ = sqlx::query(
                                "UPDATE jobs SET source_status = 'CLOSED' WHERE id = ?1",
                            )
                            .bind(j_id)
                            .execute(self.pool())
                            .await;
                            closed_detected += 1;
                        }
                    }
                }
            }

            let (ins, dup) = self.insert_raw_jobs_batch(&jobs).await?;
            newly_inserted += ins;
            duplicates_updated += dup;
            diagnostics.push(diag);
        }

        if let Some(h) = exec_handle {
            h.emit_info(
                "company_scan",
                Some("company_intel"),
                &format!(
                    "Varredura de '{}' finalizada: {} obtidas, {} novas vagas, {} atualizadas, {} encerradas",
                    company_name, total_fetched, newly_inserted, duplicates_updated, closed_detected
                ),
            )
            .await;
        }

        Ok(CompanyScanReport {
            company_id,
            company_name,
            sources_checked: details.sources.len(),
            total_fetched,
            newly_inserted,
            duplicates_updated,
            closed_detected,
            diagnostics,
        })
    }
}
