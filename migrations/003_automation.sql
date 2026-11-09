-- Perfil editável de critérios do candidato (linha única id = 1)
CREATE TABLE IF NOT EXISTS candidate_profile (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    base_city TEXT NOT NULL DEFAULT 'Araraquara',
    accepted_region TEXT NOT NULL DEFAULT 'Araraquara, São Carlos, Matão, Américo Brasiliense, Ibaté',
    accept_remote INTEGER NOT NULL DEFAULT 1,
    accepted_shifts TEXT NOT NULL DEFAULT 'Comercial (5x2), Escala 6x2, 12x36, Turnos Rotativos',
    min_salary_brl TEXT NOT NULL DEFAULT '',
    exclude_terms TEXT NOT NULL DEFAULT 'vendedor de loja, açougueiro, confeiteiro, manicure, corretor de imóveis, pedagogo, farmacêutico',
    auto_enrich INTEGER NOT NULL DEFAULT 1,
    auto_match_threshold REAL NOT NULL DEFAULT 0.62,
    notify_on_apply INTEGER NOT NULL DEFAULT 1
);

INSERT OR IGNORE INTO candidate_profile (id) VALUES (1);

-- Rondas de busca automática (Saved Searches)
CREATE TABLE IF NOT EXISTS saved_searches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    query TEXT NOT NULL,
    location TEXT NOT NULL DEFAULT 'Araraquara',
    remote INTEGER NOT NULL DEFAULT 0,
    enabled INTEGER NOT NULL DEFAULT 1,
    interval_minutes INTEGER NOT NULL DEFAULT 180,
    last_run_at TEXT,
    last_found_count INTEGER NOT NULL DEFAULT 0
);
