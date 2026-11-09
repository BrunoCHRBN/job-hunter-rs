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
    pub newly_inserted: usize,
    pub duplicates_skipped: usize,
}

/// Dispara os coletores Gupy e JobSpy concorrentemente com `tokio::join!`,
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
    let (newly_inserted, duplicates_skipped) = db.insert_raw_jobs_batch(&combined).await?;

    info!(
        "Coleta concluída: {} obtidas (Gupy: {}, JobSpy: {}), {} novas salvas, {} duplicadas ignoradas.",
        total_fetched, gupy_count, jobspy_count, newly_inserted, duplicates_skipped
    );

    Ok(CollectionReport {
        gupy_count,
        jobspy_count,
        total_fetched,
        newly_inserted,
        duplicates_skipped,
    })
}
