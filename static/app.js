(() => {
  const KANBAN_STAGES = ["Descoberta", "Salva", "Aplicada", "Entrevista", "Descartada"];

  const state = {
    jobs: [],
    selectedJobId: null,
    viewMode: "split", // "split" | "kanban"
    theme: "light",
    filters: {
      workModel: "Todas",
      shift: "",
      status: "",
      query: "",
    },
    similarityMap: new Map(), // job.id -> similarity float (0..1)
    isRankingActive: false,
  };

  // Elementos do DOM
  const el = {
    statTotal: document.getElementById("stat-total"),
    statEnriched: document.getElementById("stat-enriched"),
    statActive: document.getElementById("stat-active"),
    statPending: document.getElementById("stat-pending"),
    jobsTbody: document.getElementById("jobs-tbody"),
    tableHeaderTitle: document.getElementById("table-header-title"),
    tableEmptyState: document.getElementById("table-empty-state"),
    rankingIndicator: document.getElementById("ranking-indicator"),
    inspectorContent: document.getElementById("inspector-content"),
    inspectorActions: document.getElementById("inspector-actions"),
    inspectorStatusSelect: document.getElementById("inspector-status-select"),
    inspectorExternalLink: document.getElementById("inspector-external-link"),
    btnRunMatch: document.getElementById("btn-run-match"),
    statusMessage: document.getElementById("status-message"),
    collectForm: document.getElementById("collect-form"),
    collectQuery: document.getElementById("collect-query"),
    collectLocation: document.getElementById("collect-location"),
    collectRemote: document.getElementById("collect-remote"),
    collectAutoEnrich: document.getElementById("collect-auto-enrich"),
    btnCollect: document.getElementById("btn-collect"),
    btnEnrichPending: document.getElementById("btn-enrich-pending"),
    btnSemanticRank: document.getElementById("btn-semantic-rank"),
    btnOpenResume: document.getElementById("btn-open-resume"),
    btnToggleView: document.getElementById("btn-toggle-view"),
    viewToggleLabel: document.getElementById("view-toggle-label"),
    btnToggleTheme: document.getElementById("btn-toggle-theme"),
    themeToggleLabel: document.getElementById("theme-toggle-label"),
    viewSplit: document.getElementById("view-split"),
    viewKanban: document.getElementById("view-kanban"),
    filterSearchInput: document.getElementById("filter-search-input"),
    filterShiftSelect: document.getElementById("filter-shift-select"),
    filterWorkModelGroup: document.getElementById("filter-work-model-group"),
    filterStatusGroup: document.getElementById("filter-status-group"),
    btnResetFilters: document.getElementById("btn-reset-filters"),
    resumeModal: document.getElementById("resume-modal"),
    resumeTextarea: document.getElementById("resume-editor-textarea"),
    btnSaveResume: document.getElementById("btn-save-resume"),
    btnSaveAndRematch: document.getElementById("btn-save-and-rematch"),
    btnCloseResume: document.getElementById("btn-close-resume"),
  };

  function setStatus(msg) {
    el.statusMessage.textContent = msg;
  }

  function escapeHtml(raw) {
    if (!raw) return "";
    return String(raw)
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
  }

  function parseJsonArray(rawJson) {
    if (!rawJson) return [];
    try {
      const parsed = JSON.parse(rawJson);
      return Array.isArray(parsed) ? parsed : [];
    } catch (_) {
      return [];
    }
  }

  function getScoreBadgeClass(score) {
    if (score === null || score === undefined) return "badge";
    if (score >= 70) return "badge badge-good tnum";
    if (score >= 50) return "badge badge-warn tnum";
    return "badge badge-bad tnum";
  }

  async function apiFetch(url, options = {}) {
    const resp = await fetch(url, {
      headers: { "Content-Type": "application/json" },
      ...options,
    });
    if (!resp.ok) {
      const errBody = await resp.text();
      throw new Error(errBody || `HTTP ${resp.status}`);
    }
    return resp.json();
  }

  async function refreshAll() {
    try {
      const params = new URLSearchParams();
      if (state.filters.workModel && state.filters.workModel !== "Todas") {
        params.set("work_model", state.filters.workModel);
      }
      if (state.filters.shift) {
        params.set("shift", state.filters.shift);
      }
      if (state.filters.status) {
        params.set("status", state.filters.status);
      }
      if (state.filters.query) {
        params.set("q", state.filters.query);
      }

      const [stats, jobs] = await Promise.all([
        apiFetch("/api/stats"),
        apiFetch(`/api/jobs?${params.toString()}`),
      ]);

      el.statTotal.textContent = stats.total_jobs;
      el.statEnriched.textContent = stats.enriched_jobs;
      el.statActive.textContent = stats.saved_or_applied_jobs;
      el.statPending.textContent = Math.max(0, stats.total_jobs - stats.enriched_jobs);

      if (state.isRankingActive && state.similarityMap.size > 0) {
        jobs.sort((a, b) => {
          const sa = state.similarityMap.get(a.id) ?? -1;
          const sb = state.similarityMap.get(b.id) ?? -1;
          return sb - sa;
        });
      }

      state.jobs = jobs;

      if (state.jobs.length > 0) {
        const stillExists = state.jobs.some((j) => j.id === state.selectedJobId);
        if (!stillExists) {
          state.selectedJobId = state.jobs[0].id;
        }
      } else {
        state.selectedJobId = null;
      }

      renderTable();
      renderInspector();
      renderKanban();
    } catch (err) {
      setStatus(`Erro ao carregar dados: ${err.message}`);
    }
  }

  function renderTable() {
    el.tableHeaderTitle.textContent = `Vagas Registradas (${state.jobs.length})`;
    el.rankingIndicator.classList.toggle("hidden", !state.isRankingActive);

    if (state.jobs.length === 0) {
      el.jobsTbody.innerHTML = "";
      el.tableEmptyState.classList.remove("hidden");
      return;
    }

    el.tableEmptyState.classList.add("hidden");

    const rowsHtml = state.jobs
      .map((job) => {
        const isSelected = job.id === state.selectedJobId;
        const sim = state.similarityMap.get(job.id);
        let scoreHtml = `<span class="badge tnum">--</span>`;

        if (job.match_score !== null && job.match_score !== undefined) {
          const cls = getScoreBadgeClass(job.match_score);
          scoreHtml = `<span class="${cls}">${job.match_score.toFixed(0)}%</span>`;
        } else if (sim !== undefined) {
          const pct = (sim * 100).toFixed(0);
          scoreHtml = `<span class="badge badge-accent tnum" title="Similaridade de Cosseno">${pct}%</span>`;
        }

        const workModel = escapeHtml(job.work_model || "Nao Informado");
        const shift = escapeHtml(job.shift_schedule || "N/I");
        const status = escapeHtml(job.application_status || "Descoberta");

        return `
          <tr data-job-id="${job.id}" class="${isSelected ? "selected" : ""}">
            <td class="col-num">#${job.id}</td>
            <td class="col-num">${scoreHtml}</td>
            <td title="${escapeHtml(job.title)}"><strong>${escapeHtml(job.title || "Sem Título")}</strong></td>
            <td title="${escapeHtml(job.company)}">${escapeHtml(job.company || "N/D")}</td>
            <td><span class="badge">${workModel}</span></td>
            <td title="${shift}">${shift}</td>
            <td><span class="badge">${status}</span></td>
          </tr>
        `;
      })
      .join("");

    el.jobsTbody.innerHTML = rowsHtml;

    el.jobsTbody.querySelectorAll("tr[data-job-id]").forEach((tr) => {
      tr.addEventListener("click", () => {
        const id = Number(tr.getAttribute("data-job-id"));
        selectJob(id);
      });
    });
  }

  function selectJob(jobId) {
    state.selectedJobId = jobId;
    el.jobsTbody.querySelectorAll("tr[data-job-id]").forEach((tr) => {
      const id = Number(tr.getAttribute("data-job-id"));
      tr.classList.toggle("selected", id === jobId);
    });
    renderInspector();
  }

  function renderInspector() {
    const job = state.jobs.find((j) => j.id === state.selectedJobId);
    if (!job) {
      el.inspectorActions.classList.add("hidden");
      el.inspectorContent.innerHTML = `
        <div class="state-box">
          <strong>Nenhuma vaga selecionada.</strong>
          <span>Selecione uma linha na tabela à esquerda (ou use as teclas j / k) para inspecionar os dados estruturados e a análise ATS.</span>
        </div>
      `;
      return;
    }

    el.inspectorActions.classList.remove("hidden");
    el.inspectorStatusSelect.value = job.application_status || "Descoberta";
    if (job.url) {
      el.inspectorExternalLink.href = job.url;
      el.inspectorExternalLink.classList.remove("hidden");
    } else {
      el.inspectorExternalLink.classList.add("hidden");
    }

    const skills = parseJsonArray(job.required_skills_json);
    const redFlags = parseJsonArray(job.red_flags_json);
    const sim = state.similarityMap.get(job.id);

    const scoreBadge =
      job.match_score !== null && job.match_score !== undefined
        ? `<span class="${getScoreBadgeClass(job.match_score)}">Match ATS: ${job.match_score.toFixed(1)}/100</span>`
        : `<span class="badge">Match ATS: Não calculado (pressione m)</span>`;

    const simBadge =
      sim !== undefined
        ? `<span class="badge badge-accent tnum">Similaridade Semântica: ${(sim * 100).toFixed(1)}%</span>`
        : "";

    const skillsHtml =
      skills.length > 0
        ? skills.map((s) => `<span class="badge">${escapeHtml(s)}</span>`).join("")
        : `<span style="color: var(--text-tertiary);">Nenhuma competência extraída (pressione 'e' para enriquecer).</span>`;

    const redFlagsBlock =
      redFlags.length > 0
        ? `
          <div class="inspector-block inspector-block-alert">
            <h3 class="inspector-block-title" style="color: var(--sem-bad-fg);">
              <span>Alertas / Red Flags Detectados (${redFlags.length})</span>
            </h3>
            <ul style="margin: 0; padding-left: 18px; color: var(--sem-bad-fg);">
              ${redFlags.map((rf) => `<li>${escapeHtml(rf)}</li>`).join("")}
            </ul>
          </div>
        `
        : "";

    const matchAnalysisBlock = job.match_analysis_md
      ? `
        <div class="inspector-block">
          <h3 class="inspector-block-title">
            <span>Relatório de Compatibilidade Curricular &amp; ATS</span>
            <button type="button" class="btn" id="btn-inline-open-resume">Inserir Keywords no curriculo.md</button>
          </h3>
          <div class="raw-description-box" style="max-height: none; background: var(--bg-surface); color: var(--text-primary);">${escapeHtml(
            job.match_analysis_md
          )}</div>
        </div>
      `
      : `
        <div class="state-box">
          <strong>Análise de Match Score &amp; ATS ainda não executada para esta vaga.</strong>
          <span>Clique em "Calcular Match ATS" (ou pressione a tecla <code>m</code>) para cruzar os requisitos desta vaga com o seu <code>curriculo.md</code> usando o Gemini.</span>
        </div>
      `;

    el.inspectorContent.innerHTML = `
      <div class="inspector-header-card">
        <div style="display: flex; align-items: center; gap: 8px; margin-bottom: 6px;">
          <span class="mono" style="color: var(--text-tertiary);">#${job.id} (${escapeHtml(job.source || "db")})</span>
          ${scoreBadge}
          ${simBadge}
        </div>
        <h3 class="inspector-job-title">${escapeHtml(job.title || "Sem Título")}</h3>
        <p class="inspector-company-line">
          <strong>${escapeHtml(job.company || "Empresa N/D")}</strong> · ${escapeHtml(job.location || "Local N/D")}
        </p>
        <div class="meta-grid">
          <div class="meta-item">
            <span class="meta-label">Modalidade</span>
            <span class="meta-val">${escapeHtml(job.work_model || "Nao Informado")}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">Escala / Turno</span>
            <span class="meta-val">${escapeHtml(job.shift_schedule || "Nao Informado")}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">Salário Estimado</span>
            <span class="meta-val tnum">${escapeHtml(job.salary_estimate_brl || "Não informado")}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">Tipo de Contrato</span>
            <span class="meta-val">${escapeHtml(job.contract_type || "Nao Informado")}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">Senioridade</span>
            <span class="meta-val">${escapeHtml(job.seniority || "Nao Informado")}</span>
          </div>
          <div class="meta-item">
            <span class="meta-label">Estágio no Funil</span>
            <span class="meta-val">${escapeHtml(job.application_status || "Descoberta")}</span>
          </div>
        </div>
      </div>

      ${redFlagsBlock}

      <div class="inspector-block">
        <h3 class="inspector-block-title">Resumo Estruturado (IA)</h3>
        <p style="margin: 0; color: var(--text-primary);">
          ${escapeHtml(job.ai_summary || "Vaga ainda não enriquecida pela IA.")}
        </p>
      </div>

      <div class="inspector-block">
        <h3 class="inspector-block-title">
          <span>Competências Técnicas Extraídas (${skills.length})</span>
          <span style="font-weight: normal; color: var(--text-tertiary);">Clique em uma skill para anexar ao curriculo.md</span>
        </h3>
        <div class="tag-list">
          ${
            skills.length > 0
              ? skills
                  .map(
                    (s) =>
                      `<button type="button" class="skill-chip-btn" data-skill="${escapeHtml(
                        s
                      )}" title="Adicionar '${escapeHtml(s)}' ao curriculo.md">+ ${escapeHtml(s)}</button>`
                  )
                  .join("")
              : skillsHtml
          }
        </div>
      </div>

      ${matchAnalysisBlock}

      <div class="inspector-block">
        <h3 class="inspector-block-title">Descrição Original da Vaga</h3>
        <div class="raw-description-box">${escapeHtml(job.raw_description || "Sem descrição bruta.")}</div>
      </div>
    `;

    el.inspectorContent.querySelectorAll(".skill-chip-btn").forEach((btn) => {
      btn.addEventListener("click", async () => {
        const skill = btn.getAttribute("data-skill");
        await appendSkillToResume(skill);
      });
    });

    const inlineResumeBtn = document.getElementById("btn-inline-open-resume");
    if (inlineResumeBtn) {
      inlineResumeBtn.addEventListener("click", () => openResumeModal());
    }
  }

  function renderKanban() {
    const colsHtml = KANBAN_STAGES.map((stage) => {
      const stageJobs = state.jobs.filter(
        (j) => (j.application_status || "Descoberta").toLowerCase() === stage.toLowerCase()
      );

      const cardsHtml =
        stageJobs.length > 0
          ? stageJobs
              .map((job) => {
                const scoreBadge =
                  job.match_score !== null && job.match_score !== undefined
                    ? `<span class="${getScoreBadgeClass(job.match_score)}">${job.match_score.toFixed(0)}%</span>`
                    : `<span class="badge tnum">#${job.id}</span>`;

                return `
                  <div class="kanban-card" data-kanban-job-id="${job.id}">
                    <div style="display: flex; justify-content: space-between; align-items: center; gap: 4px;">
                      ${scoreBadge}
                      <span class="badge">${escapeHtml(job.work_model || "N/I")}</span>
                    </div>
                    <div class="kanban-card-title">${escapeHtml(job.title || "Sem Título")}</div>
                    <div class="kanban-card-sub">${escapeHtml(job.company || "N/D")} · ${escapeHtml(
                      job.location || ""
                    )}</div>
                    <div class="kanban-card-sub">Turno: ${escapeHtml(job.shift_schedule || "N/I")}</div>
                  </div>
                `;
              })
              .join("")
          : `<div style="color: var(--text-tertiary); font-size: 12px; padding: 8px;">Nenhuma vaga neste estágio.</div>`;

      return `
        <div class="kanban-col">
          <div class="kanban-col-header">
            <span>${escapeHtml(stage)}</span>
            <span class="badge tnum">${stageJobs.length}</span>
          </div>
          <div class="kanban-col-body">${cardsHtml}</div>
        </div>
      `;
    }).join("");

    el.viewKanban.innerHTML = colsHtml;

    el.viewKanban.querySelectorAll("[data-kanban-job-id]").forEach((card) => {
      card.addEventListener("click", () => {
        const id = Number(card.getAttribute("data-kanban-job-id"));
        state.selectedJobId = id;
        toggleViewMode("split");
        renderTable();
        renderInspector();
      });
    });
  }

  function toggleViewMode(forcedMode) {
    state.viewMode = forcedMode || (state.viewMode === "split" ? "kanban" : "split");
    const isKanban = state.viewMode === "kanban";
    el.viewSplit.classList.toggle("hidden", isKanban);
    el.viewKanban.classList.toggle("hidden", !isKanban);
    el.viewToggleLabel.textContent = isKanban ? "Visão: Kanban" : "Visão: Tabela";
    el.btnToggleView.classList.toggle("btn-active-tab", isKanban);
  }

  function toggleTheme() {
    state.theme = state.theme === "light" ? "dark" : "light";
    document.documentElement.setAttribute("data-theme", state.theme);
    el.themeToggleLabel.textContent = state.theme === "light" ? "Tema Escuro" : "Tema Claro";
  }

  async function runMatchForSelected() {
    if (!state.selectedJobId) {
      setStatus("Selecione uma vaga na tabela antes de calcular o Match ATS.");
      return;
    }
    const id = state.selectedJobId;
    el.btnRunMatch.disabled = true;
    setStatus(`Calculando Match Score & ATS via Gemini para a vaga #${id}...`);
    try {
      const res = await apiFetch(`/api/jobs/${id}/match`, { method: "POST" });
      setStatus(
        `Match Score calculado para a vaga #${id}: ${res.analysis.match_score.toFixed(1)}/100.`
      );
      await refreshAll();
    } catch (err) {
      setStatus(`Falha ao calcular Match ATS: ${err.message}`);
    } finally {
      el.btnRunMatch.disabled = false;
    }
  }

  async function runEnrichPending() {
    el.btnEnrichPending.disabled = true;
    setStatus("Enriquecendo vagas pendentes em lote via Gemini + gerando embeddings...");
    try {
      const res = await apiFetch("/api/enrich", {
        method: "POST",
        body: JSON.stringify({ batch_size: 5 }),
      });
      setStatus(`Enriquecimento finalizado: ${res.enriched_count} vaga(s) processada(s).`);
      await refreshAll();
    } catch (err) {
      setStatus(`Falha no enriquecimento: ${err.message}`);
    } finally {
      el.btnEnrichPending.disabled = false;
    }
  }

  async function runSemanticRank() {
    el.btnSemanticRank.disabled = true;
    setStatus("Calculando similaridade de cosseno em memória contra curriculo.md...");
    try {
      const res = await apiFetch("/api/rank", { method: "POST" });
      state.similarityMap.clear();
      res.ranked.forEach((item) => {
        state.similarityMap.set(item.job_id, item.similarity);
      });
      state.isRankingActive = true;
      setStatus(`Ranking semântico concluído (${res.ranked.length} vagas comparadas).`);
      await refreshAll();
    } catch (err) {
      setStatus(`Falha no ranking semântico: ${err.message}`);
    } finally {
      el.btnSemanticRank.disabled = false;
    }
  }

  async function updateSelectedStatus(newStatus) {
    if (!state.selectedJobId) return;
    const id = state.selectedJobId;
    try {
      await apiFetch(`/api/jobs/${id}/status`, {
        method: "PATCH",
        body: JSON.stringify({ status: newStatus }),
      });
      setStatus(`Status da vaga #${id} alterado para '${newStatus}'.`);
      await refreshAll();
    } catch (err) {
      setStatus(`Erro ao atualizar status: ${err.message}`);
    }
  }

  async function cycleSelectedStatus() {
    const job = state.jobs.find((j) => j.id === state.selectedJobId);
    if (!job) return;
    const current = job.application_status || "Descoberta";
    const idx = KANBAN_STAGES.findIndex((s) => s.toLowerCase() === current.toLowerCase());
    const next = KANBAN_STAGES[(idx + 1) % KANBAN_STAGES.length];
    await updateSelectedStatus(next);
  }

  async function openResumeModal() {
    try {
      const data = await apiFetch("/api/resume");
      el.resumeTextarea.value = data.content || "";
      el.resumeModal.classList.remove("hidden");
      el.resumeTextarea.focus();
    } catch (err) {
      setStatus(`Erro ao carregar curriculo.md: ${err.message}`);
    }
  }

  function closeResumeModal() {
    el.resumeModal.classList.add("hidden");
  }

  async function saveResume(rematchAfter = false) {
    try {
      await apiFetch("/api/resume", {
        method: "PUT",
        body: JSON.stringify({ content: el.resumeTextarea.value }),
      });
      setStatus("Arquivo curriculo.md salvo com sucesso.");
      closeResumeModal();
      if (rematchAfter && state.selectedJobId) {
        await runMatchForSelected();
      }
    } catch (err) {
      setStatus(`Erro ao salvar curriculo.md: ${err.message}`);
    }
  }

  async function appendSkillToResume(skill) {
    try {
      const data = await apiFetch("/api/resume");
      let content = data.content || "";
      if (content.toLowerCase().includes(skill.toLowerCase())) {
        setStatus(`A competência '${skill}' já consta no curriculo.md.`);
        return;
      }
      content = `${content.trimEnd()}\n- **Competência Adicional (ATS):** ${skill}\n`;
      await apiFetch("/api/resume", {
        method: "PUT",
        body: JSON.stringify({ content }),
      });
      setStatus(`Competência '${skill}' anexada ao curriculo.md. Pressione 'm' para recalcular o Match.`);
    } catch (err) {
      setStatus(`Erro ao anexar competência: ${err.message}`);
    }
  }

  // Eventos de formulário e botões
  el.collectForm.addEventListener("submit", async (e) => {
    e.preventDefault();
    const query = el.collectQuery.value.trim();
    if (!query) return;
    const location = el.collectLocation.value.trim() || "Brasil";
    const remote = el.collectRemote.checked;
    const autoEnrich = el.collectAutoEnrich.checked;

    el.btnCollect.disabled = true;
    setStatus(`Coletando vagas para '${query}' (${location})...`);
    try {
      const report = await apiFetch("/api/search", {
        method: "POST",
        body: JSON.stringify({ query, location, limit: 15, remote }),
      });
      setStatus(
        `Coleta concluída: ${report.newly_inserted} novas salvas (${report.duplicates_skipped} duplicadas blake3 ignoradas).`
      );
      await refreshAll();

      if (autoEnrich && report.newly_inserted > 0) {
        await runEnrichPending();
      }
    } catch (err) {
      setStatus(`Erro na coleta: ${err.message}`);
    } finally {
      el.btnCollect.disabled = false;
    }
  });

  el.btnEnrichPending.addEventListener("click", runEnrichPending);
  el.btnSemanticRank.addEventListener("click", runSemanticRank);
  el.btnOpenResume.addEventListener("click", openResumeModal);
  el.btnCloseResume.addEventListener("click", closeResumeModal);
  el.btnSaveResume.addEventListener("click", () => saveResume(false));
  el.btnSaveAndRematch.addEventListener("click", () => saveResume(true));
  el.btnToggleView.addEventListener("click", () => toggleViewMode());
  el.btnToggleTheme.addEventListener("click", toggleTheme);
  el.btnRunMatch.addEventListener("click", runMatchForSelected);

  el.inspectorStatusSelect.addEventListener("change", (e) => {
    updateSelectedStatus(e.target.value);
  });

  el.filterWorkModelGroup.querySelectorAll("button[data-work-model]").forEach((btn) => {
    btn.addEventListener("click", () => {
      el.filterWorkModelGroup
        .querySelectorAll("button")
        .forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      state.filters.workModel = btn.getAttribute("data-work-model");
      refreshAll();
    });
  });

  el.filterStatusGroup.querySelectorAll("button[data-status]").forEach((btn) => {
    btn.addEventListener("click", () => {
      el.filterStatusGroup
        .querySelectorAll("button")
        .forEach((b) => b.classList.remove("active"));
      btn.classList.add("active");
      state.filters.status = btn.getAttribute("data-status");
      refreshAll();
    });
  });

  el.filterShiftSelect.addEventListener("change", (e) => {
    state.filters.shift = e.target.value;
    refreshAll();
  });

  el.filterSearchInput.addEventListener("input", (e) => {
    state.filters.query = e.target.value.trim();
    refreshAll();
  });

  el.btnResetFilters.addEventListener("click", () => {
    state.filters = { workModel: "Todas", shift: "", status: "", query: "" };
    state.isRankingActive = false;
    el.filterSearchInput.value = "";
    el.filterShiftSelect.value = "";
    el.filterWorkModelGroup.querySelectorAll("button").forEach((b, i) => {
      b.classList.toggle("active", i === 0);
    });
    el.filterStatusGroup.querySelectorAll("button").forEach((b, i) => {
      b.classList.toggle("active", i === 0);
    });
    refreshAll();
  });

  // Navegação Keyboard-First
  window.addEventListener("keydown", (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
      e.preventDefault();
      el.filterSearchInput.focus();
      el.filterSearchInput.select();
      return;
    }

    if (e.key === "Escape") {
      if (!el.resumeModal.classList.contains("hidden")) {
        closeResumeModal();
      }
      return;
    }

    const activeTag = document.activeElement ? document.activeElement.tagName : "";
    if (activeTag === "INPUT" || activeTag === "TEXTAREA" || activeTag === "SELECT") {
      return;
    }

    if (e.key === "j" || e.key === "ArrowDown") {
      e.preventDefault();
      if (state.jobs.length === 0) return;
      const idx = state.jobs.findIndex((j) => j.id === state.selectedJobId);
      const nextIdx = Math.min(state.jobs.length - 1, idx + 1);
      selectJob(state.jobs[nextIdx].id);
    } else if (e.key === "k" || e.key === "ArrowUp") {
      e.preventDefault();
      if (state.jobs.length === 0) return;
      const idx = state.jobs.findIndex((j) => j.id === state.selectedJobId);
      const prevIdx = Math.max(0, idx - 1);
      selectJob(state.jobs[prevIdx].id);
    } else if (e.key === "m") {
      e.preventDefault();
      runMatchForSelected();
    } else if (e.key === "s") {
      e.preventDefault();
      cycleSelectedStatus();
    } else if (e.key === "e") {
      e.preventDefault();
      runEnrichPending();
    } else if (e.key === "v") {
      e.preventDefault();
      toggleViewMode();
    } else if (e.key === "r") {
      e.preventDefault();
      openResumeModal();
    }
  });

  refreshAll();
})();
