-- Migration 004: Execution Engine e Observabilidade
-- Cria tabelas para orquestração, acompanhamento de etapas, log de eventos estruturados e métricas de desempenho.

CREATE TABLE IF NOT EXISTS executions (
    id TEXT PRIMARY KEY,
    execution_type TEXT NOT NULL,
    request_origin TEXT NOT NULL DEFAULT 'SYSTEM',
    status TEXT NOT NULL DEFAULT 'PENDING',
    current_stage TEXT,
    progress_pct REAL NOT NULL DEFAULT 0.0,
    processed_count INTEGER NOT NULL DEFAULT 0,
    failed_count INTEGER NOT NULL DEFAULT 0,
    diagnostic_message TEXT,
    metadata_json TEXT,
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    duration_ms INTEGER
);

CREATE INDEX IF NOT EXISTS idx_executions_type ON executions(execution_type);
CREATE INDEX IF NOT EXISTS idx_executions_status ON executions(status);
CREATE INDEX IF NOT EXISTS idx_executions_started_at ON executions(started_at DESC);

CREATE TABLE IF NOT EXISTS execution_stages (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    stage_name TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'PENDING',
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    completed_at TEXT,
    duration_ms INTEGER,
    items_in INTEGER NOT NULL DEFAULT 0,
    items_out INTEGER NOT NULL DEFAULT 0,
    error_message TEXT,
    FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_stages_exec_id ON execution_stages(execution_id);
CREATE INDEX IF NOT EXISTS idx_stages_status ON execution_stages(status);

CREATE TABLE IF NOT EXISTS execution_events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    event_id TEXT NOT NULL,
    stage TEXT NOT NULL DEFAULT 'general',
    source TEXT,
    level TEXT NOT NULL DEFAULT 'INFO',
    status TEXT,
    message TEXT NOT NULL,
    details TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    duration_ms INTEGER,
    FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_events_exec_id ON execution_events(execution_id);
CREATE INDEX IF NOT EXISTS idx_events_level ON execution_events(level);
CREATE INDEX IF NOT EXISTS idx_events_created_at ON execution_events(created_at ASC);

CREATE TABLE IF NOT EXISTS execution_metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    execution_id TEXT NOT NULL,
    metric_name TEXT NOT NULL,
    metric_value REAL NOT NULL,
    metric_unit TEXT NOT NULL DEFAULT '',
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY(execution_id) REFERENCES executions(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_metrics_exec_id ON execution_metrics(execution_id);
