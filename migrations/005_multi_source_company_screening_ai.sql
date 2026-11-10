-- Migration 005: Multi-source Collectors, Job Sources, Company Intelligence, Screening Engine & AI Telemetry

-- 1. Configuração de Coletores
CREATE TABLE IF NOT EXISTS collector_configs (
    source_id TEXT PRIMARY KEY,
    display_name TEXT NOT NULL,
    group_name TEXT NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    timeout_secs INTEGER NOT NULL DEFAULT 35,
    default_limit INTEGER NOT NULL DEFAULT 20,
    strategy_notes TEXT NOT NULL DEFAULT '',
    last_status TEXT,
    last_run_at TEXT,
    last_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT
);

-- 2. Rastreabilidade de Fontes por Vaga (Múltiplas Origens para a mesma Oportunidade)
CREATE TABLE IF NOT EXISTS job_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    job_id INTEGER NOT NULL,
    source TEXT NOT NULL,
    external_id TEXT NOT NULL DEFAULT '',
    original_url TEXT NOT NULL DEFAULT '',
    first_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_seen_at TEXT NOT NULL DEFAULT (datetime('now')),
    source_status TEXT NOT NULL DEFAULT 'ACTIVE',
    FOREIGN KEY(job_id) REFERENCES jobs(id) ON DELETE CASCADE,
    UNIQUE(job_id, source, external_id)
);

CREATE INDEX IF NOT EXISTS idx_job_sources_job_id ON job_sources(job_id);
CREATE INDEX IF NOT EXISTS idx_job_sources_source_ext ON job_sources(source, external_id);

-- 3. Company Intelligence: Cadastro de Empresas
CREATE TABLE IF NOT EXISTS companies (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    official_name TEXT NOT NULL,
    normalized_name TEXT UNIQUE NOT NULL,
    website TEXT NOT NULL DEFAULT '',
    segment TEXT NOT NULL DEFAULT '',
    location TEXT NOT NULL DEFAULT '',
    description TEXT NOT NULL DEFAULT '',
    is_monitored INTEGER NOT NULL DEFAULT 1,
    is_preferred INTEGER NOT NULL DEFAULT 0,
    is_excluded INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_companies_norm_name ON companies(normalized_name);
CREATE INDEX IF NOT EXISTS idx_companies_monitored ON companies(is_monitored);

-- 4. Aliases e Variações de Nome de Empresas
CREATE TABLE IF NOT EXISTS company_aliases (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id INTEGER NOT NULL,
    alias_name TEXT NOT NULL,
    normalized_alias TEXT NOT NULL,
    FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE,
    UNIQUE(company_id, normalized_alias)
);

CREATE INDEX IF NOT EXISTS idx_company_aliases_norm ON company_aliases(normalized_alias);

-- 5. Fontes Corporativas e Páginas de Carreira (ATS)
CREATE TABLE IF NOT EXISTS company_sources (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    company_id INTEGER NOT NULL,
    source_type TEXT NOT NULL DEFAULT 'ATS_BOARD',
    career_url TEXT NOT NULL,
    ats_provider TEXT NOT NULL DEFAULT 'gupy',
    portal_id TEXT NOT NULL DEFAULT '',
    integration_status TEXT NOT NULL DEFAULT 'ACTIVE',
    last_checked_at TEXT,
    next_check_at TEXT,
    last_found_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    FOREIGN KEY(company_id) REFERENCES companies(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_company_sources_comp_id ON company_sources(company_id);

-- 6. Telemetria, Explicabilidade e Custos de Operações de IA (Sprint 13)
CREATE TABLE IF NOT EXISTS ai_operations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT,
    operation_type TEXT NOT NULL,
    model_used TEXT NOT NULL,
    fallback_used INTEGER NOT NULL DEFAULT 0,
    attempts INTEGER NOT NULL DEFAULT 1,
    status TEXT NOT NULL DEFAULT 'SUCCESS',
    records_count INTEGER NOT NULL DEFAULT 1,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    total_tokens INTEGER NOT NULL DEFAULT 0,
    duration_ms INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_ai_ops_type ON ai_operations(operation_type);
CREATE INDEX IF NOT EXISTS idx_ai_ops_started ON ai_operations(started_at DESC);

-- 7. Cache de IA para Prevenção de Chamadas Repetidas
CREATE TABLE IF NOT EXISTS ai_cache (
    cache_key TEXT PRIMARY KEY,
    operation_type TEXT NOT NULL,
    model_used TEXT NOT NULL,
    response_json TEXT NOT NULL,
    prompt_tokens INTEGER NOT NULL DEFAULT 0,
    completion_tokens INTEGER NOT NULL DEFAULT 0,
    hit_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    last_hit_at TEXT NOT NULL DEFAULT (datetime('now'))
);
