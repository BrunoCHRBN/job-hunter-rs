pub mod gupy;
pub mod jobspy;

use anyhow::Result;
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::db::Database;
use crate::models::RawJobListing;

#[derive(Debug, Clone)]
pub struct CollectionReport {
    pub gupy_count: usize,
    pub jobspy_count: usize,
    pub total_fetched: usize,
    pub excluded_by_filter: usize,
    pub newly_inserted: usize,
    pub duplicates_skipped: usize,
}

/// Dispara os coletores Gupy e JobSpy concorrentemente com `tokio::join!`,
/// filtra vagas cujos títulos batam em `exclude_terms` do perfil do candidato,
/// deduplica por hash `blake3` e persiste as vagas inéditas no SQLite.
pub async fn collect_all(
    config: &AppConfig,
    db: &Database,
    query: &str,
    location: Option<&str>,
    limit: usize,
    remote: bool,
) -> Result<CollectionReport> {
    let loc_str = location.unwrap_or("Brasil");
    let profile = db.get_candidate_profile().await.unwrap_or_default();

    let gupy_fut = gupy::fetch_gupy_jobs(query, location, limit, remote);
    let jobspy_fut = jobspy::fetch_jobspy_jobs(
        &config.python_bin,
        query,
        loc_str,
        limit,
        168,
        remote,
    );

    let (gupy_res, jobspy_jobs) = tokio::join!(gupy_fut, jobspy_fut);

    let gupy_jobs = match gupy_res {
        Ok(jobs) => jobs,
        Err(err) => {
            warn!("Erro durante coleta na Gupy: {}", err);
            Vec::new()
        }
    };

    let gupy_count = gupy_jobs.len();
    let jobspy_count = jobspy_jobs.len();

    let mut combined: Vec<RawJobListing> = Vec::with_capacity(gupy_count + jobspy_count);
    combined.extend(gupy_jobs);
    combined.extend(jobspy_jobs);

    let total_fetched = combined.len();
    let filtered: Vec<RawJobListing> = combined
        .into_iter()
        .filter(|j| !profile.matches_exclusion(&j.title))
        .collect();
    let excluded_by_filter = total_fetched.saturating_sub(filtered.len());

    let (newly_inserted, duplicates_skipped) = db.insert_raw_jobs_batch(&filtered).await?;

    info!(
        "Coleta concluída: {} obtidas (Gupy: {}, JobSpy: {}), {} filtradas por exclusão, {} novas salvas, {} duplicadas ignoradas.",
        total_fetched, gupy_count, jobspy_count, excluded_by_filter, newly_inserted, duplicates_skipped
    );

    Ok(CollectionReport {
        gupy_count,
        jobspy_count,
        total_fetched,
        excluded_by_filter,
        newly_inserted,
        duplicates_skipped,
    })
}
