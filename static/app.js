'use strict';
/* Job Hunter RS — UI v2 (Refinada conforme revisão visual, ruído de informação e UX). */

const STAGES = [
  ['descoberta', 'Descoberta'],
  ['salva', 'Salva'],
  ['aplicada', 'Aplicada'],
  ['entrevista', 'Entrevista'],
  ['descartada', 'Descartada'],
];
const FUNIL = ['salva', 'aplicada', 'entrevista'];

const EMPTY_STAGE_GUIDE = {
  descoberta: 'Coleta inicial vazia. Use "Coletar vagas" no Painel.',
  salva: 'Salve vagas promissoras na tela Vagas (tecla s) para acompanhá-las aqui.',
  aplicada: 'Marque vagas como aplicadas após enviar sua candidatura.',
  entrevista: 'Registre aqui as vagas em que você avançou para entrevista.',
  descartada: 'Vagas descartadas ficam arquivadas nesta coluna.',
};

async function req(method, url, body) {
  let r;
  try {
    r = await fetch(url, {
      method,
      headers: body ? { 'Content-Type': 'application/json' } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
  } catch (netErr) {
    throw new Error('Falha de comunicação com o servidor local. Recarregue a página (F5) e tente novamente.');
  }
  const t = await r.text();
  if (!r.ok) throw new Error(t || 'Erro ' + r.status);
  try {
    return JSON.parse(t);
  } catch {
    return t;
  }
}

const API = {
  jobs: () => req('GET', '/api/jobs'),
  setStatus: (id, st) => req('POST', `/api/jobs/${id}/status`, { status: st }),
  match: (id) => req('POST', `/api/jobs/${id}/match`),
  collect: (b) => req('POST', '/api/collect', b),
  enrich: () => req('POST', '/api/enrich'),
  resume: () => req('GET', '/api/resume'),
  saveResume: (content) => req('PUT', '/api/resume', { content }),
};

const nk = (s) =>
  String(s ?? '')
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase();
const esc = (s) =>
  String(s ?? '').replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' }[c])
  );
const parse = (v) => {
  try {
    return JSON.parse(v);
  } catch {
    return null;
  }
};

/* Filtra valores vazios ou marcadores "Nao Informado" / "N/I" para evitar chips inúteis */
function cleanVal(v) {
  const s = String(v ?? '').trim();
  if (!s || /^(n[aã]o\s*informad[oa]|n\/i|n\/d|null|none|undefined|-|—)$/i.test(s)) return '';
  return s;
}

const arr = (v) => {
  const raw = Array.isArray(v)
    ? v
    : typeof v === 'string' && v
      ? Array.isArray(parse(v))
        ? parse(v)
        : v.split(',')
      : [];
  return raw.map((x) => cleanVal(x)).filter(Boolean);
};

const stage = (s) => {
  const k = nk(s);
  return (STAGES.find((x) => k.startsWith(x[0].slice(0, 5))) || STAGES[0])[0];
};

function formatSource(src) {
  const s = cleanVal(src).toLowerCase();
  if (!s || s === 'db') return '';
  if (s === 'gupy') return 'Gupy';
  if (s.includes('linkedin')) return 'LinkedIn';
  if (s.includes('indeed')) return 'Indeed';
  if (s.includes('glassdoor')) return 'Glassdoor';
  return s.charAt(0).toUpperCase() + s.slice(1);
}

function cleanLocation(loc, modo) {
  let l = cleanVal(loc);
  if (!l) return '';
  if (nk(modo) === 'remoto') {
    l = l.replace(/\s*\(\s*remoto\s*\)\s*/gi, '').trim();
    if (nk(l) === 'remoto') return '';
  }
  return l;
}

function companyHue(name) {
  let h = 0;
  const s = String(name || 'Empresa');
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) % 360;
  return h;
}

function parseMatchMd(md) {
  if (!md || typeof md !== 'string') return {};
  const j = parse(md);
  if (j && typeof j === 'object') return j;
  const sec = (title) => {
    const m = md.match(new RegExp(`##\\s+${title}\\n([\\s\\S]*?)(?=\\n##\\s+|$)`));
    return m ? m[1].trim() : '';
  };
  const items = (txt) =>
    txt
      .split('\n')
      .map((l) => l.replace(/^[-•*]\s*/, '').trim())
      .filter((l) => l && !/^Nenhuma|^Nenhum|^Sem /i.test(l));
  return {
    shift_and_location_verdict: sec('Veredito de Escala e Localização'),
    matching_skills: items(sec('Skills Compatíveis')),
    missing_skills: items(sec('Gaps / Skills Ausentes')),
    ats_suggestions: items(sec('Sugestões de Otimização ATS')),
    detailed_analysis_markdown: sec('Análise Detalhada') || md,
  };
}

function norm(j) {
  let a = j.analysis || j.match_analysis || parseMatchMd(j.match_analysis_md) || {};
  if (typeof a === 'string') a = parse(a) || parseMatchMd(a) || {};
  const pick = (k) => j[k] ?? a[k];
  const modo = cleanVal(j.work_model);
  const turno = cleanVal(j.shift_schedule);
  const contrato = cleanVal(j.contract_type);
  const nivel = cleanVal(j.seniority);
  const salario = cleanVal(j.salary_estimate_brl);
  const resumo = cleanVal(j.summary || j.ai_summary);
  return {
    id: j.id,
    title: cleanVal(j.title) || 'Sem título',
    company: cleanVal(j.company) || 'Empresa',
    location: cleanLocation(j.location, modo),
    source: formatSource(j.source),
    url: /^https?:/i.test(j.url || j.job_url || '') ? j.url || j.job_url : '',
    modo,
    turno,
    contrato,
    nivel,
    salario,
    resumo,
    skills: arr(j.required_skills ?? j.required_skills_json),
    flags: arr(j.red_flags ?? j.red_flags_json),
    desc: cleanVal(j.description || j.raw_description),
    etapa: stage(j.status || j.application_status),
    score: j.match_score != null ? Math.round(j.match_score) : null,
    enriched: Boolean(j.is_enriched ?? j.enriched ?? resumo),
    an: {
      ok: arr(pick('matching_skills')),
      falta: arr(pick('missing_skills')),
      ats: arr(pick('ats_suggestions')),
      veredito: cleanVal(pick('shift_and_location_verdict')),
      md: cleanVal(pick('detailed_analysis_markdown')),
    },
  };
}

/* ---------- Estado e Utilitários ---------- */
const $ = (s, r = document) => r.querySelector(s);

function loadSavedFilters() {
  try {
    const saved = JSON.parse(localStorage.getItem('jh-filters') || '{}');
    return {
      q: saved.q || '',
      local: saved.local || '',
      modo: saved.modo || '',
      turno: saved.turno || '',
      etapa: saved.etapa || '',
    };
  } catch {
    return { q: '', local: '', modo: '', turno: '', etapa: '' };
  }
}

const S = {
  jobs: [],
  sel: null,
  tab: 'geral',
  f: loadSavedFilters(),
  cv: null,
  cvStale: false,
  matchingId: null,
  expandedCols: { descoberta: false, descartada: false },
};

function saveFilters() {
  try {
    localStorage.setItem('jh-filters', JSON.stringify(S.f));
  } catch {}
}

const cls = (s) => (s >= 70 ? 'hi' : s >= 40 ? 'mid' : 'lo');
const chip = (t, c = '') => (cleanVal(t) ? `<span class="chip ${c}">${esc(cleanVal(t))}</span>` : '');
const ul = (a) => (a.length ? `<ul>${a.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : '');
const byScore = (a, b) => (b.score ?? -1) - (a.score ?? -1) || b.id - a.id;
const stageLabel = (k) => (STAGES.find((s) => s[0] === k) || STAGES[0])[1];

let tt;
function toast(msg, err = false, undoFn = null) {
  const t = $('#toast');
  t.innerHTML = `<span>${esc(msg)}</span>${undoFn ? '<button type="button" id="toast-undo">Desfazer</button>' : ''}`;
  t.className = err ? 'err' : '';
  t.hidden = false;
  if (undoFn) {
    $('#toast-undo')?.addEventListener(
      'click',
      () => {
        t.hidden = true;
        undoFn();
      },
      { once: true }
    );
  }
  clearTimeout(tt);
  tt = setTimeout(() => (t.hidden = true), 4500);
}

async function run(btn, fn, ok) {
  if (btn) btn.disabled = true;
  try {
    await fn();
    if (ok) toast(ok);
  } catch (e) {
    toast(String(e.message || e).slice(0, 160), true);
  } finally {
    if (btn) btn.disabled = false;
  }
}

async function load() {
  const [jobsData, cvData] = await Promise.all([
    API.jobs(),
    S.cv === null ? API.resume().catch(() => null) : Promise.resolve(null),
  ]);
  if (cvData !== null && S.cv === null) {
    S.cv = typeof cvData === 'string' ? cvData : cvData?.content ?? '';
    const cvEl = $('#cv');
    if (cvEl && !cvEl.value) cvEl.value = S.cv;
  }
  S.jobs = (Array.isArray(jobsData) ? jobsData : jobsData.jobs || []).map(norm);
  syncFilterInputs();
  render();
}

function syncFilterInputs() {
  const qEl = $('[data-f="q"]');
  const lEl = $('[data-f="local"]');
  const mEl = $('[data-f="modo"]');
  const eEl = $('[data-f="etapa"]');
  if (qEl && qEl.value !== S.f.q) qEl.value = S.f.q;
  if (lEl) lEl.value = S.f.local;
  if (mEl) mEl.value = S.f.modo;
  if (eEl) eEl.value = S.f.etapa;
  const hasFilter = Boolean(S.f.q || S.f.local || S.f.modo || S.f.turno || S.f.etapa);
  const clearBtn = $('#clear-f');
  if (clearBtn) clearBtn.hidden = !hasFilter;
}

/* Verifica se uma competência exigida já aparece no currículo ou nas skills compatíveis */
function skillStatusClass(skill, job) {
  const sk = nk(skill);
  if (!sk) return '';
  if (job.an.ok.some((x) => nk(x).includes(sk) || sk.includes(nk(x)))) return 'ok';
  if (job.an.falta.some((x) => nk(x).includes(sk) || sk.includes(nk(x)))) return 'bad';
  if (S.cv) {
    const cvNorm = nk(S.cv);
    if (cvNorm.includes(sk)) return 'ok';
  }
  return '';
}

/* ---------- Tela: Vagas ---------- */
const REGIAO_ARARAQUARA = ['araraquara', 'sao carlos', 'matao', 'americo brasiliense', 'ibate'];

function matchesLocalFilter(job, localFilter) {
  if (!localFilter) return true;
  const jobLoc = nk(job.location);
  const jobModo = nk(job.modo);
  if (localFilter === '__regiao__') {
    return REGIAO_ARARAQUARA.some((c) => jobLoc.includes(c));
  }
  if (localFilter === '__remoto__') {
    return jobModo === 'remoto' || jobLoc.includes('remoto');
  }
  return jobLoc.startsWith(nk(localFilter)) || jobLoc.includes(nk(localFilter));
}

function filtered() {
  const { q, local, modo, turno, etapa } = S.f;
  const tokens = nk(q).split(/\s+/).filter(Boolean);
  return S.jobs
    .filter((j) => {
      if (tokens.length) {
        const hay = nk([j.title, j.company, j.location, j.modo, j.turno, j.resumo, j.skills.join(' ')].join(' '));
        if (!tokens.every((t) => hay.includes(t))) return false;
      }
      if (!matchesLocalFilter(j, local)) return false;
      if (modo && !nk(j.modo).startsWith(modo)) return false;
      if (turno && j.turno !== turno) return false;
      if (etapa && j.etapa !== etapa) return false;
      return true;
    })
    .sort(byScore);
}

/* Sem avatar repetitivo na lista; destaque azul apenas em hover/selecionada; etapa só quando != Descoberta */
function row(j, inPainel = false) {
  const isSelected = !inPainel && j.id === S.sel;
  const stageChip = j.etapa !== 'descoberta' ? chip(stageLabel(j.etapa), 'stage-tag') : '';
  const metaLine = [j.company, j.location].filter(Boolean).join(' · ');
  const attr = inPainel ? `data-open="${j.id}"` : `data-id="${j.id}"`;
  return `<li><button type="button" class="job${isSelected ? ' on' : ''}" ${attr}>
    <span class="job-t">
      <strong>${esc(j.title)}</strong>
      <span class="mut">${esc(metaLine)}</span>
      <span class="chips">${chip(j.modo)}${chip(j.turno)}${stageChip}</span>
    </span>
    ${j.score != null ? `<span class="score ${cls(j.score)}">${j.score}%</span>` : ''}
  </button></li>`;
}

function primaryStageAction(j) {
  if (j.etapa === 'descoberta') return ['salva', 'Salvar vaga'];
  if (j.etapa === 'salva') return ['aplicada', 'Marcar como aplicada'];
  if (j.etapa === 'aplicada') return ['entrevista', 'Registrar entrevista'];
  if (j.etapa === 'descartada') return ['salva', 'Restaurar para Salva'];
  return null;
}

function tabBody(j) {
  if (S.tab === 'desc') {
    return `<p class="raw">${esc(j.desc) || '<span class="mut">Sem descrição disponível.</span>'}</p>`;
  }
  const a = j.an;
  if (S.tab === 'match') {
    if (j.score == null && !a.md && !a.ok.length) {
      return '<p class="empty">Clique em “Calcular match” acima para comparar esta vaga com o seu currículo e gerar recomendações para o ATS.</p>';
    }
    return `${a.veredito ? `<div class="verdict-box"><h3>Veredito de escala e localização</h3><p>${esc(a.veredito)}</p></div>` : ''}
      <div class="two">
        <div><h3>Você já tem</h3><div class="chips">${a.ok.map((x) => chip(x, 'ok')).join('') || '<span class="mut">Nada identificado</span>'}</div></div>
        <div><h3>Falta no currículo</h3><div class="chips">${a.falta.map((x) => chip(x, 'bad')).join('') || '<span class="mut">Nenhum gap crítico</span>'}</div></div>
      </div>
      ${a.ats.length ? `<div><h3>Ajustes sugeridos para o ATS</h3>${ul(a.ats)}</div>` : ''}
      ${a.md ? `<div><h3>Análise detalhada</h3>${a.md.split(/\n{2,}/).map((p) => `<p class="raw" style="margin-top:8px">${esc(p)}</p>`).join('')}</div>` : ''}`;
  }

  /* Visão Geral: já inclui o veredito de match e competências coloridas conforme o currículo */
  const hasMatch = j.score != null || a.veredito || a.falta.length > 0;
  return `${j.enriched ? '' : '<p class="mut">Esta vaga ainda não foi enriquecida pela IA. Use “Enriquecer pendentes” no Painel.</p>'}
    ${a.veredito ? `<div class="verdict-box"><h3>Compatibilidade de escala e localização</h3><p>${esc(a.veredito)}</p></div>` : ''}
    ${j.resumo ? `<div><h3>Resumo da vaga</h3><p>${esc(j.resumo)}</p></div>` : ''}
    ${j.flags.length ? `<div class="alert"><h3>Pontos de atenção</h3>${ul(j.flags)}</div>` : ''}
    ${
      j.skills.length
        ? `<div>
            <h3>Competências exigidas <span class="mut" style="font-weight:400;font-size:12px">(verde: consta no currículo · vermelho: gap identificado)</span></h3>
            <div class="chips">${j.skills.map((x) => chip(x, skillStatusClass(x, j))).join('')}</div>
          </div>`
        : ''
    }
    ${
      hasMatch && a.falta.length
        ? `<div><h3>Falta no seu currículo para esta vaga</h3><div class="chips">${a.falta.map((x) => chip(x, 'bad')).join('')}</div></div>`
        : ''
    }`;
}

function detail(j) {
  if (!j) return '<p class="empty">Selecione uma vaga na lista à esquerda para inspecionar os detalhes.</p>';
  const tabs = [
    ['geral', 'Visão geral'],
    ['match', 'Análise ATS'],
    ['desc', 'Descrição original'],
  ];
  const hue = companyHue(j.company);
  const nextAct = primaryStageAction(j);
  const isCalculating = S.matchingId === j.id;
  const matchLabel = isCalculating
    ? 'Calculando…'
    : j.score != null
      ? 'Recalcular match'
      : 'Calcular match';
  const subMeta = [j.location, j.source ? `Fonte: ${j.source}` : ''].filter(Boolean).join(' · ');

  return `<div class="d-head">
      <span class="logo" style="background:hsl(${hue} 45% 92%);color:hsl(${hue} 65% 28%);border-color:hsl(${hue} 40% 80%)" aria-hidden="true">${esc(j.company[0] || '?')}</span>
      <div class="grow">
        <h2>${esc(j.title)}</h2>
        <p>${esc(j.company)}</p>
        ${subMeta ? `<p class="mut">${esc(subMeta)}</p>` : ''}
      </div>
      ${j.score != null ? `<div class="big ${cls(j.score)}"><b>${j.score}%</b><span>match</span></div>` : ''}
    </div>
    <div class="chips">
      ${[j.modo, j.turno, j.contrato, j.nivel, j.salario].map((x) => chip(x)).join('')}
      ${j.etapa !== 'descoberta' ? chip(`Etapa: ${stageLabel(j.etapa)}`, 'stage-tag') : ''}
    </div>
    <div class="d-act">
      ${nextAct ? `<button type="button" class="btn primary" data-stage-to="${nextAct[0]}">${nextAct[1]}</button>` : ''}
      ${j.url ? `<a class="btn${nextAct ? '' : ' primary'}" href="${esc(j.url)}" target="_blank" rel="noopener">Abrir vaga ↗</a>` : ''}
      <button type="button" class="btn" data-act="match" ${isCalculating ? 'disabled' : ''}>${matchLabel}</button>
      <select data-act="etapa" aria-label="Mudar etapa manualmente">
        ${STAGES.map(([k, l]) => `<option value="${k}"${k === j.etapa ? ' selected' : ''}>Etapa: ${l}</option>`).join('')}
      </select>
      ${j.etapa !== 'descartada' ? `<button type="button" class="btn ghost danger" data-stage-to="descartada">Descartar</button>` : ''}
    </div>
    <div class="tabs" role="tablist">
      ${tabs.map(([k, l]) => `<button type="button" role="tab" aria-selected="${S.tab === k}" data-tab="${k}">${l}</button>`).join('')}
    </div>
    <div class="d-body">${tabBody(j)}</div>`;
}

function renderVagas() {
  const list = filtered();
  if (!list.some((j) => j.id === S.sel)) S.sel = list[0]?.id ?? null;
  $('#count').textContent = `${list.length} ${list.length === 1 ? 'vaga' : 'vagas'}`;
  $('#list').innerHTML =
    list.map((j) => row(j, false)).join('') ||
    '<li class="empty">Nenhuma vaga com esses filtros. Use “Limpar filtros” ou colete novas vagas no Painel.</li>';
  $('#detail').innerHTML = detail(S.jobs.find((j) => j.id === S.sel));

  const l = $('#f-local');
  const cities = [
    ...new Set(
      S.jobs
        .map((j) => (j.location || '').split(' - ')[0].trim())
        .filter((c) => c && nk(c) !== 'remoto' && nk(c) !== 'brasil')
    ),
  ].sort();
  if (l) {
    l.innerHTML =
      '<option value="">Localidade</option>' +
      '<option value="__regiao__"' +
      (S.f.local === '__regiao__' ? ' selected' : '') +
      '>Araraquara e região</option>' +
      cities
        .map(
          (x) =>
            `<option value="${esc(x)}"${x === S.f.local ? ' selected' : ''}>${esc(x)}</option>`
        )
        .join('');
  }

  const t = $('#f-turno');
  const turnos = [...new Set(S.jobs.map((j) => j.turno).filter(Boolean))].sort();
  t.innerHTML =
    '<option value="">Escala</option>' +
    turnos
      .map((x) => `<option value="${esc(x)}"${x === S.f.turno ? ' selected' : ''}>${esc(x)}</option>`)
      .join('');
  syncFilterInputs();
}

/* ---------- Tela: Painel ---------- */
function renderPainel() {
  const n = S.jobs.length;
  const enr = S.jobs.filter((j) => j.enriched).length;
  const p = n - enr;
  const withoutMatch = S.jobs.filter((j) => j.score == null && j.etapa !== 'descartada');

  $('#stats').innerHTML = [
    ['Vagas coletadas', n],
    ['Enriquecidas', enr],
    ['No funil ativo', S.jobs.filter((j) => FUNIL.includes(j.etapa)).length],
    ['Com match calculado', S.jobs.filter((j) => j.score != null).length],
  ]
    .map(([l, v]) => `<div><dd>${v}</dd><dt>${l}</dt></div>`)
    .join('');

  /* Top matches sem a classe .on (sem fundo azul indevido) */
  const top = S.jobs
    .filter((j) => j.score != null)
    .sort(byScore)
    .slice(0, 5);
  $('#top').innerHTML =
    top.map((j) => row(j, true)).join('') ||
    '<li class="empty">Nenhum match calculado ainda. Abra uma vaga e clique em “Calcular match”.</li>';

  /* Card condicional de Enriquecimento: só aparece quando há pendentes */
  const pendEl = $('#pend');
  if (p > 0) {
    pendEl.hidden = false;
    pendEl.innerHTML = `<h2>Enriquecimento pendente</h2>
      <p class="mut" style="margin:6px 0 12px">${p} ${p === 1 ? 'vaga aguarda' : 'vagas aguardam'} extração de turno, competências e alertas pela IA.</p>
      <button type="button" class="btn primary" data-act="enrich">Enriquecer ${p} ${p === 1 ? 'vaga' : 'vagas'}</button>`;
  } else {
    pendEl.hidden = true;
  }

  /* Card de Próxima Ação Recomendada */
  const nextEl = $('#next-action');
  if (n === 0) {
    nextEl.innerHTML = `<h2>Próximo passo</h2><p class="mut" style="margin-top:6px">Use o formulário ao lado para coletar suas primeiras vagas na Gupy e JobSpy.</p>`;
  } else if (withoutMatch.length > 0) {
    const target = withoutMatch[0];
    nextEl.innerHTML = `<h2>Próxima ação recomendada</h2>
      <p class="mut" style="margin:6px 0 12px"><strong>${withoutMatch.length} ${withoutMatch.length === 1 ? 'vaga está' : 'vagas estão'} sem análise de match</strong> contra o seu currículo.</p>
      <div class="row">
        <button type="button" class="btn primary" data-open="${target.id}">Avaliar “${esc(target.title.slice(0, 28))}”</button>
      </div>`;
  } else {
    nextEl.innerHTML = `<h2>Triagem em dia</h2><p class="mut" style="margin-top:6px">Todas as vagas ativas já possuem resumo estruturado e nota de compatibilidade.</p>`;
  }

  /* Distribuição por Etapa */
  const distEl = $('#stage-dist');
  distEl.innerHTML = `<h2>Distribuição por etapa</h2>
    <div class="dist-list">
      ${STAGES.map(([k, l]) => {
        const count = S.jobs.filter((j) => j.etapa === k).length;
        const pct = n > 0 ? Math.round((count / n) * 100) : 0;
        return `<div class="dist-row">
          <span>${esc(l)}</span>
          <div class="dist-bar"><div class="dist-fill" style="width:${pct}%"></div></div>
          <span class="mut" style="text-align:right;font-variant-numeric:tabular-nums">${count}</span>
        </div>`;
      }).join('')}
    </div>`;
}

/* ---------- Tela: Funil (Com recolhimento de Descoberta/Descartada e Drag-and-Drop) ---------- */
function renderFunil() {
  const COLLAPSIBLE = ['descoberta', 'descartada'];
  const LIMIT = 5;

  $('#board').innerHTML = STAGES.map(([k, l], i) => {
    const allStageJobs = S.jobs.filter((j) => j.etapa === k).sort(byScore);
    const isCollapsible = COLLAPSIBLE.includes(k);
    const isExpanded = Boolean(S.expandedCols[k]);
    const visibleJobs = isCollapsible && !isExpanded ? allStageJobs.slice(0, LIMIT) : allStageJobs;
    const hiddenCount = allStageJobs.length - visibleJobs.length;

    const cardsHtml = visibleJobs.length
      ? visibleJobs
          .map(
            (j) => `<div class="k" draggable="true" data-drag-id="${j.id}">
              <button type="button" class="k-b" data-open="${j.id}">${esc(j.title)}</button>
              <span class="mut">${esc([j.company, j.location].filter(Boolean).join(' · '))}</span>
              <div class="k-f">
                ${j.score != null ? `<span class="score ${cls(j.score)}">${j.score}%</span>` : '<span></span>'}
                <span>
                  ${i > 0 ? `<button type="button" class="btn ghost" data-move="-1" data-for="${j.id}" aria-label="Mover para ${STAGES[i - 1][1]}">←</button>` : ''}
                  ${i < 4 ? `<button type="button" class="btn" data-move="1" data-for="${j.id}" aria-label="Mover para ${STAGES[i + 1][1]}">${STAGES[i + 1][1]} →</button>` : ''}
                </span>
              </div>
            </div>`
          )
          .join('')
      : `<p class="empty" style="font-size:12px;padding:18px 8px">${esc(EMPTY_STAGE_GUIDE[k])}</p>`;

    const toggleBtn =
      isCollapsible && allStageJobs.length > LIMIT
        ? `<button type="button" class="btn ghost" data-toggle-col="${k}" style="width:100%;font-size:12px">
            ${isExpanded ? 'Recolher lista' : `Ver todas (${allStageJobs.length} vagas, +${hiddenCount} ocultas)`}
          </button>`
        : '';

    return `<div class="col" data-col-stage="${k}">
      <div class="col-h">${l}<span>${allStageJobs.length}</span></div>
      ${cardsHtml}
      ${toggleBtn}
    </div>`;
  }).join('');

  wireKanbanDragAndDrop();
}

function wireKanbanDragAndDrop() {
  let draggedJobId = null;
  document.querySelectorAll('[data-drag-id]').forEach((card) => {
    card.addEventListener('dragstart', (e) => {
      draggedJobId = Number(card.dataset.dragId);
      e.dataTransfer.effectAllowed = 'move';
    });
  });

  document.querySelectorAll('[data-col-stage]').forEach((col) => {
    col.addEventListener('dragover', (e) => {
      e.preventDefault();
      col.classList.add('drag-over');
    });
    col.addEventListener('dragleave', () => col.classList.remove('drag-over'));
    col.addEventListener('drop', (e) => {
      e.preventDefault();
      col.classList.remove('drag-over');
      const targetStage = col.dataset.colStage;
      if (draggedJobId && targetStage) {
        run(null, () => moveStage(draggedJobId, targetStage));
      }
    });
  });
}

/* ---------- Tela: Currículo (Editor + Preview Markdown lado a lado + Dirty State) ---------- */
function renderMarkdownPreview(md) {
  const lines = String(md || '').split('\n');
  let html = '';
  let inList = false;

  for (const rawLine of lines) {
    const line = rawLine.trim();
    if (!line) {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      continue;
    }
    if (line.startsWith('# ')) {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      html += `<h1>${esc(line.slice(2))}</h1>`;
    } else if (line.startsWith('## ')) {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      html += `<h2>${esc(line.slice(3))}</h2>`;
    } else if (line.startsWith('### ')) {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      html += `<h3>${esc(line.slice(4))}</h3>`;
    } else if (/^[-*•]\s+/.test(line)) {
      if (!inList) {
        html += '<ul>';
        inList = true;
      }
      const itemText = esc(line.replace(/^[-*•]\s+/, '')).replace(
        /\*\*(.+?)\*\*/g,
        '<strong>$1</strong>'
      );
      html += `<li>${itemText}</li>`;
    } else if (/^---+$/.test(line)) {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      html += '<hr style="border:0;border-top:1px solid var(--border);margin:4px 0">';
    } else {
      if (inList) {
        html += '</ul>';
        inList = false;
      }
      const pText = esc(line).replace(/\*\*(.+?)\*\*/g, '<strong>$1</strong>');
      html += `<p>${pText}</p>`;
    }
  }
  if (inList) html += '</ul>';
  return html || '<p class="mut">Digite seu currículo em Markdown ao lado para visualizar a formatação.</p>';
}

function updateCvDirtyIndicator() {
  const cvEl = $('#cv');
  const dirtyEl = $('#cv-dirty');
  const previewEl = $('#cv-preview');
  if (!cvEl) return;
  if (previewEl) previewEl.innerHTML = renderMarkdownPreview(cvEl.value);
  if (dirtyEl) dirtyEl.hidden = S.cv === null || cvEl.value === S.cv;
}

async function renderCurriculo() {
  if (S.cv === null) {
    try {
      const d = await API.resume();
      S.cv = typeof d === 'string' ? d : d.content ?? d.resume ?? d.markdown ?? '';
      $('#cv').value = S.cv;
    } catch {
      toast('Não foi possível carregar o currículo.', true);
    }
  }
  $('#cv-stale').hidden = !S.cvStale;
  updateCvDirtyIndicator();
}

async function saveCurriculum(btn) {
  const val = $('#cv').value;
  await run(btn, async () => {
    await API.saveResume(val);
    S.cv = val;
    S.cvStale = true;
    $('#cv-stale').hidden = false;
    updateCvDirtyIndicator();
    toast('Currículo salvo. Recalcule o match nas vagas de interesse para atualizar as notas.');
  });
}

/* ---------- Roteamento (Suporte a #vagas/12) ---------- */
const VIEWS = ['painel', 'vagas', 'funil', 'curriculo'];

function parseHash() {
  const raw = location.hash.slice(1);
  const [viewPart, idPart] = raw.split('/');
  const view = VIEWS.includes(viewPart) ? viewPart : 'painel';
  const jobId = idPart && !Number.isNaN(Number(idPart)) ? Number(idPart) : null;
  return { view, jobId };
}

function cur() {
  return parseHash().view;
}

function render() {
  const { view, jobId } = parseHash();
  if (jobId != null) S.sel = jobId;
  VIEWS.forEach((x) => {
    $('#v-' + x).hidden = x !== view;
  });
  document
    .querySelectorAll('[data-nav]')
    .forEach((a) =>
      a.dataset.nav === view
        ? a.setAttribute('aria-current', 'page')
        : a.removeAttribute('aria-current')
    );
  ({ painel: renderPainel, vagas: renderVagas, funil: renderFunil, curriculo: renderCurriculo })[
    view
  ]();
}

const go = (v, id) => {
  if (id != null) {
    S.sel = id;
    const targetHash = v === 'vagas' ? `#vagas/${id}` : `#${v}`;
    if (location.hash === targetHash) render();
    else location.hash = targetHash;
  } else {
    if (location.hash === '#' + v) render();
    else location.hash = v;
  }
};

/* ---------- Ações ---------- */
async function moveStage(id, to, allowUndo = true) {
  const j = S.jobs.find((x) => x.id === id);
  if (!j || j.etapa === to) return;
  const old = j.etapa;
  j.etapa = to;
  render();
  try {
    await API.setStatus(id, stageLabel(to));
    if (allowUndo) {
      toast(`Movida para ${stageLabel(to)}.`, false, () => moveStage(id, old, false));
    }
  } catch (e) {
    j.etapa = old;
    render();
    throw e;
  }
}

const calcMatch = (btn) => {
  if (!S.sel) return;
  const targetId = S.sel;
  S.matchingId = targetId;
  renderVagas();
  run(
    btn,
    async () => {
      try {
        await API.match(targetId);
        S.cvStale = false;
        await load();
      } finally {
        S.matchingId = null;
        renderVagas();
      }
    },
    'Match calculado com sucesso.'
  );
};

async function enrichAllPending(btn) {
  await run(btn, async () => {
    let total = 0;
    for (let step = 0; step < 10; step++) {
      toast(`Enriquecendo lote com IA… (${total} processadas)`);
      const res = await API.enrich();
      total += res.enriched_count || 0;
      await load();
      if (!res.remaining || res.enriched_count === 0) break;
    }
    toast(`Enriquecimento concluído (${total} ${total === 1 ? 'vaga processada' : 'vagas processadas'}).`);
  });
}
const nextStage = () => {
  const j = S.jobs.find((x) => x.id === S.sel);
  if (j) {
    const nextIdx = (STAGES.findIndex((s) => s[0] === j.etapa) + 1) % STAGES.length;
    run(null, () => moveStage(j.id, STAGES[nextIdx][0]));
  }
};

document.addEventListener('click', (e) => {
  const t = e.target.closest(
    '[data-id],[data-open],[data-tab],[data-act],[data-move],[data-stage-to],[data-toggle-col]'
  );
  if (!t) return;
  const d = t.dataset;
  if (d.id) {
    S.sel = +d.id;
    history.replaceState(null, '', `#vagas/${S.sel}`);
    renderVagas();
  } else if (d.open) {
    go('vagas', +d.open);
  } else if (d.tab) {
    S.tab = d.tab;
    renderVagas();
  } else if (d.stageTo) {
    if (S.sel) run(t, () => moveStage(S.sel, d.stageTo));
  } else if (d.toggleCol) {
    S.expandedCols[d.toggleCol] = !S.expandedCols[d.toggleCol];
    renderFunil();
  } else if (d.move) {
    const j = S.jobs.find((x) => x.id === +d.for);
    if (!j) return;
    const i = STAGES.findIndex((s) => s[0] === j.etapa) + +d.move;
    if (STAGES[i]) run(null, () => moveStage(j.id, STAGES[i][0]));
  } else if (d.act === 'match') {
    calcMatch(t);
  } else if (d.act === 'enrich') {
    enrichAllPending(t);
  }
});

document.querySelectorAll('[data-preset-q]').forEach((btn) => {
  btn.addEventListener('click', () => {
    $('#c-query').value = btn.dataset.presetQ || '';
    $('#c-loc').value = btn.dataset.presetLoc || '';
    $('#c-rem').checked = btn.dataset.presetRem === '1';
  });
});

$('#clear-f')?.addEventListener('click', () => {
  S.f = { q: '', local: '', modo: '', turno: '', etapa: '' };
  saveFilters();
  syncFilterInputs();
  renderVagas();
});

document.addEventListener('change', (e) => {
  if (e.target.dataset.act === 'etapa') {
    run(null, () => moveStage(S.sel, e.target.value));
  } else if (e.target.dataset.f) {
    S.f[e.target.dataset.f] = e.target.value;
    saveFilters();
    renderVagas();
  }
});

document.addEventListener('input', (e) => {
  if (e.target.dataset.f === 'q') {
    S.f.q = e.target.value;
    saveFilters();
    renderVagas();
  } else if (e.target.id === 'cv') {
    updateCvDirtyIndicator();
  }
});

$('#f-etapa').insertAdjacentHTML(
  'beforeend',
  STAGES.map(([k, l]) => `<option value="${k}">${l}</option>`).join('')
);

$('#f-coleta').addEventListener('submit', (e) => {
  e.preventDefault();
  const f = new FormData(e.target);
  const query = String(f.get('query') || '').trim();
  let location = String(f.get('location') || '').trim();
  const expandRegion = Boolean(f.get('region'));
  if (expandRegion && nk(location).includes('araraquara') && !location.includes(',')) {
    location = 'Araraquara, São Carlos, Matão, Américo Brasiliense';
  }
  const wantEnrich = Boolean(f.get('enrich'));
  const remote = Boolean(f.get('remote'));

  run(e.submitter, async () => {
    toast(`Buscando vagas de “${query}” (${remote ? 'Remoto' : location || 'Brasil'})…`);
    const rep = await API.collect({
      query,
      location,
      remote,
      enrich: false,
      limit: 20,
    });
    await load();
    const ins = rep.newly_inserted || 0;
    const dup = rep.duplicates_skipped || 0;
    if (ins === 0) {
      toast(
        dup > 0
          ? `Nenhuma vaga inédita (${dup} já estavam no seu banco).`
          : `Nenhuma vaga encontrada para “${query}” em ${location || 'Brasil'}.`
      );
      return;
    }
    toast(
      `${ins} ${ins === 1 ? 'nova vaga coletada' : 'novas vagas coletadas'}.`,
      false,
      null
    );
    if (wantEnrich) {
      await enrichAllPending(null);
    }
  });
});

$('#save-cv').addEventListener('click', (e) => saveCurriculum(e.currentTarget));

$('#theme').addEventListener('click', () => {
  const d = document.documentElement;
  const n = d.dataset.theme === 'dark' ? 'light' : 'dark';
  d.dataset.theme = n;
  $('#theme').textContent = n === 'dark' ? 'Tema claro' : 'Tema escuro';
  try {
    localStorage.setItem('jh-theme', n);
  } catch {}
});
$('#theme').textContent =
  document.documentElement.dataset.theme === 'dark' ? 'Tema claro' : 'Tema escuro';

document.addEventListener('keydown', (e) => {
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 's') {
    if (cur() === 'curriculo') {
      e.preventDefault();
      saveCurriculum($('#save-cv'));
      return;
    }
  }
  if (e.key === 'Escape') return document.activeElement.blur();
  const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement.tagName);
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
    e.preventDefault();
    go('vagas');
    return $('[data-f="q"]').focus();
  }
  if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
  const v = cur();
  const ids = filtered().map((j) => j.id);
  const i = ids.indexOf(S.sel);
  if (e.key === '/') {
    e.preventDefault();
    go('vagas');
    $('[data-f="q"]').focus();
  } else if (e.key === 'r') go('curriculo');
  else if (e.key === 'v') go(v === 'funil' ? 'vagas' : 'funil');
  else if (e.key === 'e') {
    enrichAllPending(null);
  } else if (v === 'vagas') {
    if (e.key === 'j' || e.key === 'ArrowDown') {
      e.preventDefault();
      if (!ids.length) return;
      S.sel = ids[Math.min(i + 1, ids.length - 1)];
      history.replaceState(null, '', `#vagas/${S.sel}`);
      renderVagas();
      $('.job.on')?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'k' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!ids.length) return;
      S.sel = ids[Math.max(i - 1, 0)];
      history.replaceState(null, '', `#vagas/${S.sel}`);
      renderVagas();
      $('.job.on')?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'm') calcMatch(null);
    else if (e.key === 's') nextStage();
  }
});

addEventListener('hashchange', render);
load().catch((e) => {
  toast('Não foi possível carregar as vagas: ' + e.message, true);
  render();
});
