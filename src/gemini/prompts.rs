use serde_json::json;

use crate::models::{JobRecord, RawJobListing};

pub const BATCH_EXTRACTION_SYSTEM_PROMPT: &str = r#"Você é um Engenheiro de Dados e Especialista em Recrutamento Técnico (Tech Recruiter & Industrial Operations Analyst).
Sua tarefa é analisar um lote de vagas de emprego brutas e extrair metadados rigorosamente estruturados para cada vaga.

ATENÇÃO ESPECIAL:
1. Identifique com máxima precisão a ESCALA / TURNO DE TRABALHO (`shift_schedule`):
   - Procure menções a: "Comercial", "5x2", "6x2", "12x36", "Turnos Rotativos", "Revezamento", "Noturno", "Disponibilidade de horário", "Escala".
   - Caso não haja menção explícita na descrição, preencha com `"Nao Informado"`.
2. Classifique `work_model` EXCLUSIVAMENTE com um dos valores:
   - `"Remoto"`, `"Hibrido"`, `"Presencial"` ou `"Nao Informado"`.
3. Classifique `contract_type` EXCLUSIVAMENTE com um dos valores:
   - `"CLT"`, `"PJ"`, `"Estagio"`, `"Temporario"` ou `"Nao Informado"`.
4. Classifique `seniority` EXCLUSIVAMENTE com um dos valores:
   - `"Junior"`, `"Pleno"`, `"Senior"`, `"Especialista"` ou `"Operacional"` (use `"Operacional"` para cargos de operação de sistemas supervisórios/SCADA, CCO, técnicos de campo/operação, etc.).
5. Em `required_skills`, liste de forma concisa as tecnologias, ferramentas, normas ou requisitos técnicos essenciais (ex: `["SCADA", "Telemetria", "Python", "SQL", "Linux", "CNH B", "Excel"]`).
6. Em `red_flags`, aponte alertas reais encontrados na vaga (ex: acúmulo de funções díspares, PJ com exigência de subordinação/horário fixo, exigência de veículo próprio sem menção a reembolso, descrição excessivamente vaga). Se não houver red flags claros, retorne lista vazia `[]`.
7. Mantenha `external_id` IDÊNTICO ao `external_id` recebido na entrada para cada vaga.

FORMATO DE SAÍDA OBRIGATÓRIO:
Retorne EXCLUSIVAMENTE um objeto JSON válido (sem blocos markdown) seguindo exatamente esta estrutura:
{
  "jobs": [
    {
      "external_id": "string (idêntico ao recebido)",
      "work_model": "Remoto | Hibrido | Presencial | Nao Informado",
      "contract_type": "CLT | PJ | Estagio | Temporario | Nao Informado",
      "seniority": "Junior | Pleno | Senior | Especialista | Operacional",
      "shift_schedule": "string ou null (ex: 'Comercial (5x2)', 'Escala 6x2', '12x36', 'Turnos Rotativos', 'Nao Informado')",
      "required_skills": ["skill1", "skill2"],
      "salary_estimate_brl": "string ou null (ex: 'R$ 3.500 - R$ 4.500' ou 'A combinar')",
      "summary": "Resumo objetivo da vaga em 2 a 3 frases em português.",
      "red_flags": ["alerta 1", "alerta 2"]
    }
  ]
}"#;

pub const MATCH_ANALYSIS_SYSTEM_PROMPT: &str = r#"Você é um Especialista Sênior em ATS (Applicant Tracking Systems), Engenharia de Software, Infraestrutura de TI e Operação de Sistemas Supervisórios (SCADA / CCO).
Sua missão é realizar uma análise profunda de compatibilidade (Match Score de 0 a 100) entre o currículo do candidato (em Markdown) e a vaga de emprego fornecida.

DIRETRIZES DE AVALIAÇÃO:
1. Avalie a aderência técnica (Sistemas Supervisórios/SCADA, Monitoramento/Telemetria, TI/Redes/Linux, Desenvolvimento Python/TypeScript/SQL).
2. Avalie a compatibilidade geográfica e de modalidade:
   - O candidato reside em Araraquara-SP (aceita Presencial/Híbrido em Araraquara e região próxima ou 100% Remoto de qualquer lugar).
   - Se a vaga for Presencial ou Híbrida fora da região de Araraquara-SP, sinalize claramente no `shift_and_location_verdict` e penalize o `match_score`.
3. Avalie a compatibilidade de escala e turno (`shift_schedule`):
   - Destaque se a vaga é escala 6x2, 12x36, turnos rotativos ou horário comercial (5x2) e como isso se alinha ao perfil operacional/técnico do candidato.
4. Forneça recomendações práticas de palavras-chave e ajustes de currículo para passar na triagem automatizada (ATS) desta vaga específica.

FORMATO DE SAÍDA OBRIGATÓRIO:
Retorne EXCLUSIVAMENTE um objeto JSON válido (sem blocos markdown) seguindo exatamente esta estrutura:
{
  "match_score": 85.0,
  "matching_skills": ["skill compatível 1", "skill compatível 2"],
  "missing_skills": ["requisito ausente 1", "requisito ausente 2"],
  "ats_suggestions": [
    "Sugestão prática 1 para otimizar o currículo para o ATS desta vaga",
    "Sugestão prática 2"
  ],
  "shift_and_location_verdict": "Parágrafo direto avaliando a viabilidade da localização (Araraquara-SP vs local da vaga) e da escala/turno de trabalho.",
  "detailed_analysis_markdown": "Análise executiva em Markdown detalhando pontos fortes, riscos, aderência técnica e estratégia para entrevista."
}"#;

pub fn build_batch_extraction_user_prompt(jobs: &[RawJobListing]) -> String {
    let compact_jobs: Vec<_> = jobs
        .iter()
        .map(|j| {
            let truncated_desc: String = j.raw_description.chars().take(3500).collect();
            json!({
                "external_id": j.external_id,
                "source": j.source,
                "title": j.title,
                "company": j.company,
                "location": j.location,
                "description": truncated_desc,
            })
        })
        .collect();

    format!(
        "Extraia os metadados estruturados para as seguintes {} vagas de emprego:\n\n{}",
        compact_jobs.len(),
        serde_json::to_string_pretty(&compact_jobs).unwrap_or_else(|_| "[]".to_string())
    )
}

pub fn build_match_analysis_user_prompt(resume_md: &str, job: &JobRecord) -> String {
    let skills = job.required_skills().join(", ");
    let red_flags = job.red_flags().join(", ");
    let raw_desc: String = job
        .raw_description
        .as_deref()
        .unwrap_or("")
        .chars()
        .take(5000)
        .collect();

    format!(
        r#"=== CURRÍCULO DO CANDIDATO (curriculo.md) ===
{resume_md}

=== DADOS DA VAGA (ID #{id}) ===
- Título: {title}
- Empresa: {company}
- Localização: {location}
- Modalidade: {work_model}
- Tipo de Contrato: {contract_type}
- Senioridade: {seniority}
- Escala / Turno: {shift}
- Salário Estimado: {salary}
- Skills Extraídas: {skills}
- Red Flags Detectados: {red_flags}
- Resumo IA: {summary}

=== DESCRIÇÃO BRUTA DA VAGA ===
{raw_desc}
"#,
        resume_md = resume_md,
        id = job.id,
        title = job.title_str(),
        company = job.company_str(),
        location = job.location_str(),
        work_model = job.work_model_str(),
        contract_type = job.contract_type.as_deref().unwrap_or("Nao Informado"),
        seniority = job.seniority.as_deref().unwrap_or("Nao Informado"),
        shift = job.shift_str(),
        salary = job.salary_estimate_brl.as_deref().unwrap_or("Não informado"),
        skills = if skills.is_empty() { "Não extraídas" } else { &skills },
        red_flags = if red_flags.is_empty() { "Nenhum" } else { &red_flags },
        summary = job.ai_summary.as_deref().unwrap_or("Sem resumo prévio"),
        raw_desc = raw_desc,
    )
}
