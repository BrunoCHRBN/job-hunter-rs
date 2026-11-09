-- Dossiê v2: dados de origem/alertas/seções (extração) e veredito/eixos/lacunas (match).
-- Colunas antigas continuam sendo preenchidas; vagas já enriquecidas ficam com NULL
-- e o front deve tratar isso como "dossiê ainda não gerado".
ALTER TABLE jobs ADD COLUMN dossier_json TEXT;
ALTER TABLE jobs ADD COLUMN fit_json TEXT;
