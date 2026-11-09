CREATE TABLE IF NOT EXISTS jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    dedup_hash TEXT UNIQUE NOT NULL,
    external_id TEXT,
    source TEXT,
    title TEXT,
    company TEXT,
    location TEXT,
    url TEXT,
    raw_description TEXT,
    is_enriched INTEGER DEFAULT 0,
    work_model TEXT,
    contract_type TEXT,
    seniority TEXT,
    shift_schedule TEXT,
    salary_estimate_brl TEXT,
    required_skills_json TEXT,
    red_flags_json TEXT,
    ai_summary TEXT,
    embedding_json TEXT,
    application_status TEXT DEFAULT 'Descoberta',
    match_score REAL,
    match_analysis_md TEXT,
    published_at TEXT,
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_jobs_dedup_hash ON jobs(dedup_hash);
CREATE INDEX IF NOT EXISTS idx_jobs_is_enriched ON jobs(is_enriched);
CREATE INDEX IF NOT EXISTS idx_jobs_work_model ON jobs(work_model);
CREATE INDEX IF NOT EXISTS idx_jobs_application_status ON jobs(application_status);
