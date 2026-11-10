'use strict';
/* Job Hunter RS — Career Intelligence UI (Anti-Slop, SVG Icons, Desktop Craft) */

const ICONS = {
  sliders: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="4" y1="21" x2="4" y2="14"/><line x1="4" y1="10" x2="4" y2="3"/><line x1="12" y1="21" x2="12" y2="12"/><line x1="12" y1="8" x2="12" y2="3"/><line x1="20" y1="21" x2="20" y2="16"/><line x1="20" y1="12" x2="20" y2="3"/><line x1="1" y1="14" x2="7" y2="14"/><line x1="9" y1="8" x2="15" y2="8"/><line x1="17" y1="16" x2="23" y2="16"/></svg>',
  x: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>',
  help: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><path d="M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
  copy: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>',
  download: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>',
  refresh: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>',
  stop: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect x="4" y="4" width="16" height="16" rx="2" ry="2"/></svg>',
  external: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>',
  check: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>',
  alert: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/><line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/></svg>',
  info: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"/><line x1="12" y1="16" x2="12" y2="12"/><line x1="12" y1="8" x2="12.01" y2="8"/></svg>',
  arrowRight: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><line x1="5" y1="12" x2="19" y2="12"/><polyline points="12 5 19 12 12 19"/></svg>',
  chevronDown: '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9"/></svg>',
};

function icon(name, extraClass = '') {
  const svg = ICONS[name];
  if (!svg) return '';
  return `<span class="icon${extraClass ? ' ' + extraClass : ''}" aria-hidden="true">${svg}</span>`;
}

function cleanAiText(s) {
  if (!s || typeof s !== 'string') return '';
  const trimmed = s.trim();
  if (trimmed.length > 12 && trimmed === trimmed.toUpperCase()) {
    return trimmed.charAt(0).toUpperCase() + trimmed.slice(1).toLowerCase();
  }
  return trimmed;
}

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
  const opts = {
    method,
    cache: 'no-store',
    headers: body ? { 'Content-Type': 'application/json' } : {},
    body: body ? JSON.stringify(body) : undefined,
  };
  let r;
  try {
    r = await fetch(url, opts);
  } catch (firstErr) {
    await new Promise((res) => setTimeout(res, 350));
    try {
      r = await fetch(url, opts);
    } catch (netErr) {
      throw new Error('Falha de comunicação com o servidor local. Recarregue a página (F5) e tente novamente.');
    }
  }
  const t = await r.text();
  if (!r.ok) {
    if (t.includes('429') || t.includes('RESOURCE_EXHAUSTED') || t.includes('quota') || r.status === 429) {
      throw new Error('Limite temporário de requisições da chave do Gemini atingido (Rate Limit 429). O sistema aplicou análise heurística de contingência. Aguarde alguns instantes para a cota renovar.');
    }
    throw new Error(t || 'Erro ' + r.status);
  }
  try {
    return JSON.parse(t);
  } catch {
    return t;
  }
}

const API = {
  jobs: () => req('GET', '/api/jobs'),
  job: (id) => req('GET', `/api/jobs/${id}`),
  setStatus: (id, st) => req('POST', `/api/jobs/${id}/status`, { status: st }),
  match: (id) => req('POST', `/api/jobs/${id}/match`),
  collect: (b) => req('POST', '/api/jobs/fetch', b),
  enrich: () => req('POST', '/api/jobs/enrich'),
  resume: () => req('GET', '/api/resume'),
  saveResume: (content) => req('PUT', '/api/resume', { content }),
  uploadResumePdf: async (file) => {
    const res = await fetch('/api/resume/upload-pdf', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/pdf',
      },
      body: file,
    });
    if (!res.ok) {
      const errText = await res.text();
      throw new Error(errText || `HTTP ${res.status}`);
    }
    return res.json();
  },
  profile: () => req('GET', '/api/profile'),
  saveProfile: (p) => req('PUT', '/api/profile', p),
  automation: () => req('GET', '/api/automation'),
  addSearch: (b) => req('POST', '/api/automation/searches', b),
  toggleSearch: (id, enabled) => req('POST', `/api/automation/searches/${id}/toggle`, { enabled }),
  deleteSearch: (id) => req('DELETE', `/api/automation/searches/${id}`),
  runAutoNow: () => req('POST', '/api/automation/run-now'),
  executions: (params = {}) => {
    const qs = new URLSearchParams(params).toString();
    return req('GET', '/api/executions' + (qs ? '?' + qs : ''));
  },
  executionDetail: (id) => req('GET', `/api/executions/${id}`),
  cancelExecution: (id) => req('POST', `/api/executions/${id}/cancel`),
  retryExecution: (id) => req('POST', `/api/executions/${id}/retry`),
  executionsSummary: () => req('GET', '/api/executions/summary'),
  collectors: () => req('GET', '/api/collectors'),
  toggleCollector: (id, enabled) => req('POST', `/api/collectors/${id}/toggle`, { enabled }),
  jobSources: (id) => req('GET', `/api/jobs/${id}/sources`),
  screenJob: (id) => req('POST', `/api/jobs/${id}/screen`),
  screenAllJobs: () => req('POST', '/api/jobs/screen-all'),
  consolidateDuplicates: () => req('POST', '/api/jobs/consolidate-duplicates'),
  companies: () => req('GET', '/api/companies'),
  companyDetail: (id) => req('GET', `/api/companies/${id}`),
  saveCompany: (b) => req('POST', '/api/companies', b),
  addCompanyAlias: (id, alias_name) => req('POST', `/api/companies/${id}/aliases`, { alias_name }),
  addCompanySource: (id, b) => req('POST', `/api/companies/${id}/sources`, b),
  scanCompany: (id, b = {}) => req('POST', `/api/companies/${id}/scan`, b),
  aiTelemetry: () => req('GET', '/api/ai/telemetry'),
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
  if (s.startsWith('gupy')) return 'Gupy';
  if (s.includes('linkedin')) return 'LinkedIn';
  if (s.includes('indeed')) return 'Indeed';
  if (s.includes('glassdoor')) return 'Glassdoor';
  if (s.includes('vagas_com')) return 'Vagas.com.br';
  if (s.includes('solides')) return 'Sólides';
  if (s.includes('greenhouse')) return 'Greenhouse';
  if (s.includes('lever')) return 'Lever';
  if (s.includes('ashby')) return 'Ashby';
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
  let a = j.fit || j.analysis || j.match_analysis || parse(j.fit_json) || parseMatchMd(j.match_analysis_md) || {};
  if (typeof a === 'string') a = parse(a) || parseMatchMd(a) || {};
  const dossier = j.dossier || parse(j.dossier_json) || null;
  const pick = (k) => j[k] ?? a[k];
  const modo = cleanVal(j.work_model);
  const turno = cleanVal(j.shift_schedule);
  const contrato = cleanVal(j.contract_type);
  const nivel = cleanVal(j.seniority);
  const salario = cleanVal(j.salary_estimate_brl);
  const resumo = cleanVal(j.summary || j.ai_summary);
  const missing = Array.isArray(j.missing) ? j.missing : [];
  const verdictKind = j.verdict_kind || a.verdict || null;
  const screening = j.screening || parse(j.screening_json) || null;
  const screeningVerdict = j.screening_verdict || (screening && screening.verdict) || null;
  return {
    id: j.id,
    detailLoaded: j.detail_loaded === true,
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
    score: j.match_score != null ? Math.round(Number(j.match_score)) : null,
    raw_score: j.match_score != null ? Number(j.match_score) : null,
    enriched: Boolean(j.is_enriched ?? j.enriched ?? resumo),
    dossier,
    fit: a && Object.keys(a).length ? a : null,
    verdictKind,
    screening,
    screeningVerdict,
    missing,
    an: {
      ok: arr(pick('matching_skills')),
      falta: arr(pick('missing_skills')),
      ats: arr(pick('ats_suggestions')),
      veredito: cleanVal(pick('shift_and_location_verdict')),
      md: cleanVal(pick('detailed_analysis_markdown')),
      verdictReasons: Array.isArray(a.verdict_reasons) ? a.verdict_reasons : [],
      axes: a.axes || null,
      gaps: Array.isArray(a.gaps) ? a.gaps : [],
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
      fonte: saved.fonte || '',
      triagem: saved.triagem || '',
    };
  } catch {
    return { q: '', local: '', modo: '', turno: '', etapa: '', fonte: '', triagem: '' };
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
  auto: null,
  profile: null,
  activeExec: null,
  execs: [],
  collectors: [],
  companies: [],
  selectedCompanyId: null,
  aiTelemetry: null,
};
const pendingJobDetails = new Set();
let panelDataLoaded = false;
let panelDataPromise = null;
let profilePromise = null;

function saveFilters() {
  try {
    localStorage.setItem('jh-filters', JSON.stringify(S.f));
  } catch {}
}

const cls = (s) => (s >= 70 ? 'hi' : s >= 40 ? 'mid' : 'lo');
const chip = (t, c = '') => (cleanVal(t) ? `<span class="chip ${c}">${esc(cleanVal(t))}</span>` : '');
const ul = (a) => (a.length ? `<ul>${a.map((x) => `<li>${esc(x)}</li>`).join('')}</ul>` : '');
const byScore = (a, b) => (b.raw_score ?? -1) - (a.raw_score ?? -1) || (b.score ?? -1) - (a.score ?? -1) || b.id - a.id;
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
  if (cur() === 'painel') panelDataLoaded = false;
  syncFilterInputs();
  render();
}

function ensurePanelData() {
  if (panelDataLoaded || panelDataPromise) return panelDataPromise;
  panelDataPromise = Promise.all([
    API.automation().catch(() => null),
    API.collectors().catch(() => []),
    API.executions({ limit: 5 }).catch(() => []),
  ]).then(([autoData, colsData, execsData]) => {
    if (autoData) S.auto = autoData;
    if (Array.isArray(colsData)) S.collectors = colsData;
    if (Array.isArray(execsData)) S.execs = execsData;
    panelDataLoaded = true;
    if (cur() === 'painel') renderPainel();
  }).finally(() => {
    panelDataPromise = null;
  });
  return panelDataPromise;
}

function ensureProfile() {
  if (S.profile !== null || profilePromise) return profilePromise;
  profilePromise = API.profile().then((profile) => {
    S.profile = profile;
    if (cur() === 'curriculo') renderCurriculo();
  }).catch(() => null).finally(() => {
    profilePromise = null;
  });
  return profilePromise;
}

async function ensureJobDetail(id) {
  const current = S.jobs.find((j) => j.id === id);
  if (!current || current.detailLoaded || pendingJobDetails.has(id)) return;
  pendingJobDetails.add(id);
  try {
    const full = norm(await API.job(id));
    const index = S.jobs.findIndex((j) => j.id === id);
    if (index >= 0) S.jobs[index] = full;
    if (cur() === 'vagas' && S.sel === id) renderVagas();
  } catch (e) {
    toast(`Não foi possível carregar o detalhe da vaga: ${e.message || e}`, true);
  } finally {
    pendingJobDetails.delete(id);
  }
}

function syncFilterInputs() {
  const qEl = $('[data-f="q"]');
  const lEl = $('[data-f="local"]');
  const mEl = $('[data-f="modo"]');
  const eEl = $('[data-f="etapa"]');
  const foEl = $('[data-f="fonte"]');
  const trEl = $('[data-f="triagem"]');
  if (qEl && qEl.value !== S.f.q) qEl.value = S.f.q;
  if (lEl) lEl.value = S.f.local;
  if (mEl) mEl.value = S.f.modo;
  if (eEl) eEl.value = S.f.etapa;
  if (foEl) foEl.value = S.f.fonte || '';
  if (trEl) trEl.value = S.f.triagem || '';
  const hasFilter = Boolean(
    S.f.q || S.f.local || S.f.modo || S.f.turno || S.f.etapa || S.f.fonte || S.f.triagem
  );
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
  const { q, local, modo, turno, etapa, fonte, triagem } = S.f;
  const sortMode = $('#f-sort')?.value || 'score_desc';
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
      if (fonte && !nk(j.source).includes(nk(fonte))) return false;
      if (triagem && (j.screeningVerdict || '').toUpperCase() !== triagem.toUpperCase()) return false;
      return true;
    })
    .sort((a, b) => {
      if (sortMode === 'score_desc') return byScore(a, b);
      if (sortMode === 'date_desc') return b.id - a.id;
      if (sortMode === 'company_asc') return (a.company || '').localeCompare(b.company || '', 'pt') || byScore(a, b);
      if (sortMode === 'location_asc') return (a.location || '').localeCompare(b.location || '', 'pt') || byScore(a, b);
      return byScore(a, b);
    });
}

function row(j, inPainel = false) {
  const isSelected = !inPainel && j.id === S.sel;
  const stageChip = j.etapa !== 'descoberta' ? chip(stageLabel(j.etapa), 'stage-tag') : '';
  const scrChip =
    j.screeningVerdict === 'PASS'
      ? chip('PASS', 'ok')
      : j.screeningVerdict === 'REJECT'
        ? chip('REJECT', 'bad')
        : j.screeningVerdict === 'REVIEW'
          ? chip('REVIEW', 'warn')
          : '';
  const metaLine = [j.company, j.location, j.source].filter(Boolean).join(' · ');
  const attr = inPainel ? `data-open="${j.id}"` : `data-id="${j.id}"`;
  const scoreBadge = j.score != null
    ? `<span class="score ${cls(j.score)}" style="font-family:var(--font-mono);font-size:13px;font-weight:600">${j.score}%</span>`
    : '';

  return `<li>
    <button type="button" class="job-item-card${isSelected ? ' on' : ''}" ${attr}${inPainel ? '' : ` aria-pressed="${isSelected}"`}>
      <div class="job-item-header">
        <span class="job-item-title">${esc(j.title)}</span>
        ${scoreBadge}
      </div>
      <div class="job-item-meta">${esc(metaLine)}</div>
      <div class="chips" style="margin-top:6px">
        ${scrChip}
        ${j.modo ? chip(j.modo) : ''}
        ${j.turno ? chip(j.turno) : ''}
        ${stageChip}
      </div>
    </button>
  </li>`;
}

function primaryStageAction(j) {
  if (j.etapa === 'descoberta') return ['salva', 'Salvar vaga'];
  if (j.etapa === 'salva') return ['aplicada', 'Marcar como aplicada'];
  if (j.etapa === 'aplicada') return ['entrevista', 'Registrar entrevista'];
  if (j.etapa === 'descartada') return ['salva', 'Restaurar para Salva'];
  return null;
}

function renderProvenanceSheet(j) {
  if (!j.dossier || !j.dossier.provenance) return '';
  const p = j.dossier.provenance;
  const ORIGIN_LABEL = { explicit: 'Explícito', inferred: 'Inferido', absent: 'Não dito' };
  const fields = [
    ['Modalidade', j.modo || 'Não informado', p.work_model],
    ['Escala / Turno', j.turno || 'Não informado', p.shift_schedule],
    ['Contrato', j.contrato || 'Não informado', p.contract_type],
    ['Senioridade', j.nivel || 'Não informado', p.seniority],
    ['Salário', j.salario || 'Não informado', p.salary],
  ];
  return `<div>
    <h3>Ficha da vaga e origem dos dados</h3>
    <div class="dossier-grid">
      ${fields
        .map(([label, val, src]) => {
          const orig = (src && src.origin) || 'absent';
          const quote = src && src.evidence ? cleanVal(src.evidence) : '';
          return `<div class="prov-card">
            <div class="prov-head">
              <span>${esc(label)}</span>
              <span class="prov-badge ${esc(orig)}">${esc(ORIGIN_LABEL[orig] || orig)}</span>
            </div>
            <div class="prov-val">${esc(val)}</div>
            ${quote ? `<div class="prov-quote">“${esc(quote)}”</div>` : ''}
          </div>`;
        })
        .join('')}
    </div>
  </div>`;
}

function renderMissingBlock(j) {
  if (!j.missing || !j.missing.length) return '';
  const LABELS = {
    salario: 'Salário / faixa salarial',
    escala: 'Escala / horário de trabalho',
    modalidade: 'Modalidade (Presencial/Remoto)',
    contrato: 'Regime de contratação (CLT/PJ)',
    beneficios: 'Pacote de benefícios',
  };
  return `<div class="missing-bar">
    <strong>O que o texto não diz:</strong>
    ${j.missing.map((k) => `<span class="chip">${esc(LABELS[k] || k)}</span>`).join('')}
  </div>`;
}

function renderVerdictBanner(j) {
  if (!j.verdictKind && !j.an.verdictReasons.length) return '';
  const vk = j.verdictKind || 'maybe';
  const V_LABELS = {
    apply: 'Vale candidatar-se (Apply)',
    maybe: 'Avaliar com cautela (Maybe)',
    skip: 'Melhor pular (Skip)',
  };
  return `<div class="verdict-banner ${esc(vk)}">
    <strong>Veredito: ${esc(V_LABELS[vk] || vk)}</strong>
    ${j.an.verdictReasons.length ? ul(j.an.verdictReasons) : ''}
  </div>`;
}

function renderFitAxes(j) {
  const axes = j.an.axes;
  if (!axes) return '';
  const LEVEL_BADGE = {
    strong: ['ok', 'Forte'],
    partial: ['mid', 'Parcial'],
    weak: ['lo', 'Fraco'],
    unknown: ['', 'Indefinido'],
  };
  const items = [
    ['Competências', axes.skills],
    ['Logística e Escala', axes.logistics],
    ['Senioridade', axes.seniority],
    ['Salário', axes.salary],
  ];
  return `<div>
    <h3>Encaixe por eixo</h3>
    <div class="axes-grid">
      ${items
        .map(([label, ax]) => {
          if (!ax) return '';
          const [clsName, tag] = LEVEL_BADGE[ax.level] || ['', ax.level || 'N/D'];
          return `<div class="axis-card">
            <div class="prov-head">
              <strong>${esc(label)}</strong>
              <span class="chip ${clsName}">${esc(tag)}</span>
            </div>
            <p class="mut" style="font-size:12px">${esc(ax.note || 'Sem observações')}</p>
          </div>`;
        })
        .join('')}
    </div>
  </div>`;
}

function renderGapsActions(j) {
  const gaps = j.an.gaps;
  if (!gaps || !gaps.length) return '';
  const ACT_LABEL = {
    has_unlisted: ['ok', 'Destacar no currículo (já possui indício)'],
    study: ['mid', 'Estudar / revisar antes da entrevista'],
    ignore: ['', 'Secundário (pode ignorar)'],
  };
  return `<div>
    <h3>Lacunas do currículo e ação recomendada</h3>
    <div class="gap-list">
      ${gaps
        .map((g) => {
          const [c, label] = ACT_LABEL[g.suggested] || ['', g.suggested || 'Avaliar'];
          return `<div class="gap-item">
            <div>
              <strong>${esc(g.skill)}</strong>
              ${g.why ? `<p class="mut" style="font-size:12px;margin-top:2px">${esc(g.why)}</p>` : ''}
            </div>
            <span class="chip ${c}">${esc(label)}</span>
          </div>`;
        })
        .join('')}
    </div>
  </div>`;
}

function renderDossierSections(j) {
  const sec = j.dossier && j.dossier.sections;
  if (!sec) return '';
  const blocks = [
    ['Responsabilidades', sec.responsibilities || []],
    ['Requisitos obrigatórios', sec.required || []],
    ['Diferenciais', sec.nice_to_have || []],
    ['Benefícios citados', sec.benefits || []],
  ].filter(([, list]) => list.length > 0);
  if (!blocks.length) return '';
  return `<div class="two">
    ${blocks.map(([title, list]) => `<div><h3>${esc(title)}</h3>${ul(list)}</div>`).join('')}
  </div>`;
}

function renderAlertsBlock(j) {
  const alerts = j.dossier && Array.isArray(j.dossier.alerts) ? j.dossier.alerts : [];
  if (alerts.length) {
    const SEV_TAG = { critical: 'Crítico', warning: 'Atenção', info: 'Observação' };
    return `<div>
      <h3>Alertas e pontos de atenção</h3>
      <div class="alert-list">
        ${alerts
          .map((al) => {
            const sev = al.severity || 'warning';
            const quote = cleanVal(al.evidence);
            return `<div class="alert-item ${esc(sev)}">
              <div><strong>${esc(SEV_TAG[sev] || 'Atenção')}:</strong> ${esc(al.text)}</div>
              ${quote ? `<div class="prov-quote">“${esc(quote)}”</div>` : ''}
            </div>`;
          })
          .join('')}
      </div>
    </div>`;
  }
  if (j.flags.length) {
    return `<div class="alert"><h3>Pontos de atenção</h3>${ul(j.flags)}</div>`;
  }
  return '';
}

function renderUnifiedSkills(j) {
  const seen = new Set();
  const unified = [];
  for (const sk of [...j.skills, ...j.an.falta]) {
    const k = nk(sk);
    if (!k || seen.has(k)) continue;
    seen.add(k);
    unified.push(sk);
  }
  if (!unified.length) return '';
  return `<div>
    <h3>Competências exigidas e aderência <span class="mut" style="font-weight:400;font-size:12px">(verde: no currículo · vermelho: lacuna · neutro: requisito)</span></h3>
    <div class="chips">${unified.map((x) => chip(x, skillStatusClass(x, j))).join('')}</div>
  </div>`;
}

function renderScreeningBlock(j) {
  const scr = j.screening;
  if (!scr) return '';
  const v = scr.verdict || 'PASS';
  const vCls = v === 'PASS' ? 'ok' : v === 'REJECT' ? 'bad' : 'warn';
  const reqs = Array.isArray(scr.classified_requirements) ? scr.classified_requirements : [];
  const TIER_LABEL = {
    MUST_HAVE: ['bad', 'Obrigatório (MUST_HAVE)'],
    NICE_TO_HAVE: ['mid', 'Diferencial (NICE_TO_HAVE)'],
    OPTIONAL: ['', 'Desejável (OPTIONAL)'],
    UNKNOWN: ['', 'Não especificado (UNKNOWN)'],
  };
  return `<div class="card" style="padding:12px;margin:10px 0;background:var(--bg2)">
    <div class="between row">
      <div class="row" style="align-items:center;gap:8px">
        <h3 style="margin:0">Triagem Determinística (Camada 1)</h3>
        <span class="chip ${vCls}"><strong>${esc(v)}</strong> (${Math.round(scr.score || 0)}/100)</span>
        ${scr.harmonized_score != null ? `<span class="chip ok">Harmonizado c/ IA: ${Math.round(scr.harmonized_score)}%</span>` : ''}
      </div>
      <span class="mut" style="font-size:11px">${scr.eligible_for_ai ? 'Elegível para IA (Camada 2)' : 'Filtrada antes da IA'}</span>
    </div>
    <p class="mut" style="font-size:12px;margin-top:6px">${esc(scr.justification || '')}</p>
    <div class="two" style="margin-top:8px">
      ${scr.criteria_met && scr.criteria_met.length ? `<div><strong style="font-size:12px">Critérios atendidos</strong>${ul(scr.criteria_met)}</div>` : ''}
      ${scr.criteria_failed && scr.criteria_failed.length ? `<div><strong style="font-size:12px;color:var(--bad)">Impedimentos / Alertas</strong>${ul(scr.criteria_failed)}</div>` : ''}
    </div>
    ${scr.missing_info && scr.missing_info.length ? `<div style="margin-top:6px;font-size:12px"><span class="mut">Informações ausentes (não penalizadas):</span> ${scr.missing_info.map((m) => `<span class="chip">${esc(m)}</span>`).join(' ')}</div>` : ''}
    ${reqs.length ? `<div style="margin-top:8px"><strong style="font-size:12px">Classificação de Requisitos:</strong><div class="chips" style="margin-top:4px">${reqs.map((r) => {
      const [c, lbl] = TIER_LABEL[r.tier] || ['', r.tier];
      return `<span class="chip ${c}" title="${esc(lbl)}">[${esc(r.tier)}] ${esc(r.description)}</span>`;
    }).join('')}</div></div>` : ''}
  </div>`;
}

async function loadJobSourcesBox(jobId) {
  const box = $('#job-sources-box');
  if (!box) return;
  try {
    const sources = await API.jobSources(jobId);
    if (!Array.isArray(sources) || !sources.length) {
      box.innerHTML = '';
      return;
    }
    box.innerHTML = `<div style="margin:8px 0;padding:8px 10px;border:1px solid var(--bd);border-radius:6px;font-size:12px">
      <strong>Origens Rastreáveis (${sources.length} ${sources.length === 1 ? 'fonte identificada' : 'fontes consolidadas'}):</strong>
      <div class="chips" style="margin-top:4px">
        ${sources
          .map(
            (s) =>
              `<span class="chip ${s.source_status === 'CLOSED' ? 'bad' : 'ok'}">
                <strong>${esc(s.source)}</strong>
                ${s.external_id ? ` · ID: ${esc(s.external_id)}` : ''}
                · Status: ${esc(s.source_status)}
                ${s.original_url ? ` · <a href="${esc(s.original_url)}" target="_blank" rel="noopener">Abrir ${icon('external')}</a>` : ''}
              </span>`
          )
          .join('')}
      </div>
    </div>`;
  } catch {
    box.innerHTML = '';
  }
}

function renderLevel1(j) {
  const vk = j.verdictKind || (j.score != null ? (j.score >= 75 ? 'apply' : j.score >= 55 ? 'maybe' : 'skip') : null);
  const V_LABELS = {
    apply: 'Vale candidatar-se (Alta compatibilidade)',
    maybe: 'Avaliar com cautela (Compatibilidade parcial)',
    skip: 'Melhor pular (Baixa aderência)',
  };
  const scoreCls = j.score != null ? (j.score >= 75 ? 'hi' : j.score >= 55 ? 'mid' : 'lo') : '';
  const verdictTitle = vk ? V_LABELS[vk] || cleanAiText(vk) : 'Aguardando Análise de Match';
  const rawSynthesis = j.resumo || (j.an.verdictReasons.length ? j.an.verdictReasons[0] : (j.score != null ? 'Compatibilidade calculada com base nos requisitos e histórico profissional.' : 'Clique em “Calcular match” para obter o veredito por IA contra seu perfil.'));
  const synthesis = cleanAiText(rawSynthesis);

  return `<div class="level-1-card">
    <div class="level-1-score-box">
      <span class="level-1-score-val ${scoreCls}">${j.score != null ? j.score + '%' : '--'}</span>
      <span class="mut" style="font-size:11px;font-weight:600">Match IA</span>
    </div>
    <div class="level-1-verdict grow">
      <strong style="color:var(--text);font-size:14.5px">${esc(verdictTitle)}</strong>
      <p style="margin:4px 0 0;font-size:13px;color:var(--text-2);line-height:1.5">${esc(synthesis)}</p>
      ${j.an.verdictReasons.length > 1 ? `<ul style="margin:6px 0 0 16px;padding:0;font-size:12.5px;color:var(--text-2)">${j.an.verdictReasons.slice(1).map((r) => `<li>${esc(cleanAiText(r))}</li>`).join('')}</ul>` : ''}
    </div>
  </div>`;
}

function renderLevel2(j) {
  const axes = j.an.axes;
  const LEVEL_BADGE = {
    strong: ['ok', 'Forte'],
    partial: ['mid', 'Parcial'],
    weak: ['lo', 'Fraco'],
    unknown: ['', 'Indefinido'],
  };
  const defLogistics = j.turno || j.modo ? `${j.modo || 'Presencial'} · ${j.turno || 'Comercial'}` : null;
  const items = [
    { name: 'Competências', ax: axes?.skills, defNote: j.an.ok.length ? `${j.an.ok.length} comprovadas no perfil` : 'Requisitos analisados' },
    { name: 'Escala & Local', ax: axes?.logistics, defNote: defLogistics || (j.an.veredito || 'Localização compatível') },
    { name: 'Senioridade', ax: axes?.seniority, defNote: j.nivel ? `Nível ${j.nivel}` : 'Não restritivo' },
    { name: 'Remuneração & CLT', ax: axes?.salary, defNote: j.salario || 'A combinar (não penalizado)' },
  ];

  return `<div style="margin-top:14px">
    <div class="between row" style="margin-bottom:8px">
      <span style="font-size:12px;font-weight:600;color:var(--text-2)">Nível 2 — Encaixe por Eixos</span>
    </div>
    <div class="level-2-grid">
      ${items.map((item) => {
        const [clsName, tag] = item.ax ? (LEVEL_BADGE[item.ax.level] || ['', item.ax.level]) : ['', 'Avaliado'];
        const note = item.ax?.note || item.defNote;
        return `<div class="axis-item">
          <div class="between row" style="align-items:center">
            <span class="axis-name">${esc(item.name)}</span>
            ${item.ax ? `<span class="axis-status chip ${clsName}">${esc(tag)}</span>` : ''}
          </div>
          <p class="mut" style="font-size:12px;margin:4px 0 0;line-height:1.4">${esc(cleanAiText(note))}</p>
        </div>`;
      }).join('')}
    </div>
  </div>`;
}

function renderLevel3(j) {
  const okList = j.an.ok;
  const gapList = j.an.gaps.length ? j.an.gaps.map((g) => `${g.skill}${g.why ? `: ${g.why}` : ''}`) : j.an.falta;
  const missingList = (j.missing || []).map((k) => {
    const LABELS = {
      salario: 'Salário / remuneração não informados',
      escala: 'Escala / horário não divulgados',
      modalidade: 'Modalidade presencial/remoto omitida',
      contrato: 'Regime de contratação não informado',
      beneficios: 'Benefícios não listados',
    };
    return LABELS[k] || k;
  });

  if (!okList.length && !gapList.length && !missingList.length) return '';

  return `<div style="margin-top:14px">
    <div class="between row" style="margin-bottom:8px">
      <span style="font-size:12px;font-weight:600;color:var(--text-2)">Nível 3 — Evidências &amp; Lacunas</span>
    </div>
    <div class="evidence-section">
      ${okList.length ? `<div class="evidence-block match">
        <span class="evidence-block-title">${icon('check')} Comprovado no seu perfil (${okList.length})</span>
        <div class="chips" style="margin-top:6px">${okList.map((x) => chip(x, 'ok')).join('')}</div>
      </div>` : ''}

      ${gapList.length ? `<div class="evidence-block gap">
        <span class="evidence-block-title">${icon('alert')} Lacunas identificadas (${gapList.length})</span>
        <ul class="evidence-list" style="margin-top:6px">
          ${gapList.map((x) => `<li>${esc(cleanAiText(x))}</li>`).join('')}
        </ul>
      </div>` : ''}

      ${missingList.length ? `<div class="evidence-block missing">
        <span class="evidence-block-title">${icon('info')} Informações ausentes no anúncio (sem penalidade)</span>
        <p class="mut" style="font-size:12px;margin:4px 0 6px">Os itens abaixo não constam na descrição da vaga e foram tratados de forma neutra pela triagem:</p>
        <div class="chips">${missingList.map((x) => `<span class="chip">${esc(x)}</span>`).join('')}</div>
      </div>` : ''}
    </div>
  </div>`;
}

function renderOrigDescAccordion(j) {
  return `<details class="orig-desc-accordion" style="margin-top:16px">
    <summary>Ver descrição original completa da oportunidade</summary>
    <div class="orig-desc-body">
      <p class="raw">${esc(j.desc) || '<span class="mut">Sem descrição disponível.</span>'}</p>
    </div>
  </details>`;
}

function detail(j) {
  if (!j) return '<p class="empty">Selecione uma vaga na lista à esquerda para inspecionar os detalhes.</p>';
  const hue = companyHue(j.company);
  const nextAct = primaryStageAction(j);
  const isCalculating = S.matchingId === j.id;
  const matchLabel = isCalculating
    ? 'Calculando…'
    : j.score != null
      ? 'Recalcular match'
      : 'Calcular match';
  const subMeta = [j.location, j.source ? `Fonte: ${j.source}` : ''].filter(Boolean).join(' · ');
  const VERDICT_SHORT = { apply: 'Aplicar', maybe: 'Avaliar', skip: 'Pular' };
  const bigSub = (j.verdictKind && VERDICT_SHORT[j.verdictKind]) || 'match';

  return `<div class="d-head">
      <span class="logo" style="background:hsl(${hue} 45% 92%);color:hsl(${hue} 65% 28%);border-color:hsl(${hue} 40% 80%)" aria-hidden="true">${esc(j.company[0] || '?')}</span>
      <div class="grow">
        <h2 style="font-size:18px;margin:0 0 4px">${esc(j.title)}</h2>
        <p style="font-weight:600;margin:0">${esc(j.company)}</p>
        ${subMeta ? `<p class="mut" style="font-size:12px;margin:2px 0 0">${esc(subMeta)}</p>` : ''}
      </div>
      ${j.score != null ? `<div class="big ${cls(j.score)}"><b>${j.score}%</b><span>${esc(bigSub)}</span></div>` : ''}
    </div>
    <div class="chips" style="margin:8px 0">
      ${[j.modo, j.turno, j.contrato, j.nivel, j.salario].map((x) => chip(x)).join('')}
      ${j.etapa !== 'descoberta' ? chip(`Etapa: ${stageLabel(j.etapa)}`, 'stage-tag') : ''}
    </div>
    <div class="d-act" style="margin-bottom:12px">
      ${nextAct ? `<button type="button" class="btn primary" data-stage-to="${nextAct[0]}">${nextAct[1]}</button>` : ''}
      <button type="button" class="btn${nextAct ? '' : ' primary'}" data-act="match" ${isCalculating ? 'disabled' : ''}>${matchLabel}</button>
      <details class="more-menu">
        <summary class="btn">Mais ${icon('chevronDown')}</summary>
        <div class="more-pop">
          ${j.url ? `<a class="btn" href="${esc(j.url)}" target="_blank" rel="noopener">Abrir vaga original ${icon('external')}</a>` : ''}
          <label class="mut" style="font-size:11px;padding:4px 6px 0">Mudar etapa no funil
            <select data-act="etapa" aria-label="Mudar etapa manualmente">
              ${STAGES.map(([k, l]) => `<option value="${k}"${k === j.etapa ? ' selected' : ''}>${l}</option>`).join('')}
            </select>
          </label>
          ${j.etapa !== 'descartada' ? `<button type="button" class="btn danger" data-stage-to="descartada">Descartar vaga</button>` : ''}
        </div>
      </details>
    </div>
    <div class="d-body">
      ${renderLevel1(j)}
      ${renderLevel2(j)}
      ${renderLevel3(j)}
      ${renderScreeningBlock(j)}
      <div id="job-sources-box"></div>
      ${renderAlertsBlock(j)}
      ${renderDossierSections(j)}
      ${renderUnifiedSkills(j)}
      ${renderOrigDescAccordion(j)}
    </div>`;
}

function renderVagas() {
  const list = filtered();
  if (!list.some((j) => j.id === S.sel)) S.sel = list[0]?.id ?? null;
  const showDetail = parseHash().jobId != null && S.sel != null;
  $('#v-vagas').classList.toggle('show-detail', showDetail);
  $('#count').textContent = `${list.length} ${list.length === 1 ? 'vaga' : 'vagas'}`;
  $('#list').innerHTML =
    list.map((j) => row(j, false)).join('') ||
    '<li class="empty">Nenhuma vaga com esses filtros. Use “Limpar filtros” ou colete novas vagas no Painel.</li>';
  $('#detail').innerHTML =
    '<button type="button" class="btn ghost sm vagas-back" data-back-vagas>← Voltar às vagas</button>' +
    detail(S.jobs.find((j) => j.id === S.sel));
  if (S.sel) loadJobSourcesBox(S.sel);
  if (S.sel && (showDetail || window.matchMedia('(min-width: 901px)').matches)) {
    ensureJobDetail(S.sel);
  }

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
function renderCollectorsBar() {
  const bar = $('#collectors-bar');
  if (!bar) return;
  if (!S.collectors || !S.collectors.length) {
    bar.innerHTML = '<span class="mut" style="font-size:12px">Carregando coletores…</span>';
    return;
  }
  bar.innerHTML = S.collectors
    .map((c) => {
      const active = c.enabled === 1;
      const st = c.last_status || (active ? 'READY' : 'DISABLED');
      const stCls =
        st === 'SUCCESS'
          ? 'ok'
          : st === 'BLOCKED' || st === 'ERROR' || st === 'TIMEOUT'
            ? 'bad'
            : '';
      return `<button type="button" class="chip-btn ${active ? 'on' : ''}" data-toggle-collector="${esc(c.source_id)}" data-enabled="${active ? '1' : '0'}" title="${esc(c.strategy_notes || '')}" style="${active ? 'border-color:var(--brand);font-weight:600' : 'opacity:0.65'}">
        ${active ? icon('check') + ' ' : ''}${esc(c.display_name)}
        <span class="chip ${stCls}" style="font-size:10px;padding:1px 5px;margin-left:4px">${esc(st)}${c.last_count > 0 ? ` (${c.last_count})` : ''}</span>
      </button>`;
    })
    .join('');
}

function renderSourceDiagnosticsBanner(diags) {
  const banner = $('#source-diag-banner');
  if (!banner) return;
  if (!Array.isArray(diags) || !diags.length) {
    banner.hidden = true;
    return;
  }
  banner.hidden = false;
  banner.innerHTML = `<div style="padding:8px 10px;background:var(--bg2);border:1px solid var(--bd);border-radius:6px;font-size:12px">
    <strong>Diagnóstico por Fonte da Última Busca:</strong>
    <div class="chips" style="margin-top:6px">
      ${diags
        .map((d) => {
          const c =
            d.status === 'SUCCESS'
              ? 'ok'
              : d.status === 'BLOCKED' || d.status === 'ERROR' || d.status === 'TIMEOUT'
                ? 'bad'
                : d.status === 'EMPTY'
                  ? 'warn'
                  : '';
          return `<span class="chip ${c}" title="${esc(d.message)}">
            <strong>${esc(d.display_name)}:</strong> ${esc(d.status)} · ${d.count} vagas (${d.duration_ms}ms)
          </span>`;
        })
        .join('')}
    </div>
  </div>`;
}

function renderPainel() {
  ensurePanelData();
  renderCollectorsBar();
  const n = S.jobs.length;
  const enr = S.jobs.filter((j) => j.enriched).length;
  const p = n - enr;
  const withoutMatch = S.jobs.filter((j) => j.score == null && j.etapa !== 'descartada');

  // Atualiza os 4 Indicadores Estruturais e Clicáveis
  const elTotal = $('#stat-total-jobs');
  if (elTotal) elTotal.textContent = n;
  const elMatches = $('#stat-matches-count');
  if (elMatches) elMatches.textContent = S.jobs.filter((j) => j.score != null).length;
  const elPending = $('#stat-pending-count');
  if (elPending) elPending.textContent = withoutMatch.length;
  const elFunil = $('#stat-funil-count');
  if (elFunil) elFunil.textContent = S.jobs.filter((j) => FUNIL.includes(j.etapa)).length;

  // Banner de Onboarding (persistência via localStorage)
  const obBanner = $('#onboarding-banner');
  if (obBanner) {
    const dismissed = localStorage.getItem('jh-onboarding-dismissed');
    obBanner.hidden = dismissed === '1';
  }

  // Label de coletores ativos na gaveta
  const lblCols = $('#lbl-active-collectors');
  if (lblCols && S.collectors) {
    const activeCount = S.collectors.filter((c) => c.enabled === 1).length;
    lblCols.textContent = `Coletores (${activeCount}/${S.collectors.length} ativos) & Opções`;
  }

  /* Melhores Oportunidades: estritamente ordenadas de forma decrescente pelo score */
  const top = S.jobs
    .filter((j) => j.score != null)
    .sort(byScore)
    .slice(0, 5);
  const topEl = $('#top');
  if (topEl) {
    topEl.innerHTML =
      top.map((j) => row(j, true)).join('') ||
      '<li class="empty">Nenhum match calculado ainda. Abra uma vaga em “Vagas” e clique em “Calcular match”.</li>';
  }

  /* Card condicional de Enriquecimento: só aparece quando há pendentes */
  const pendEl = $('#pend');
  if (pendEl) {
    if (p > 0) {
      pendEl.hidden = false;
      pendEl.innerHTML = `<h2>Enriquecimento pendente</h2>
        <p class="mut" style="margin:6px 0 12px">${p} ${p === 1 ? 'vaga aguarda' : 'vagas aguardam'} extração de turno, competências e alertas pela IA.</p>
        <button type="button" class="btn primary" data-act="enrich">Enriquecer ${p} ${p === 1 ? 'vaga' : 'vagas'}</button>`;
    } else {
      pendEl.hidden = true;
    }
  }

  /* Card de Próxima Ação Recomendada */
  const nextEl = $('#next-action');
  if (nextEl) {
    if (n === 0) {
      nextEl.innerHTML = `<h2>Próximo passo</h2><p class="mut" style="margin-top:6px">Use o formulário ao lado para iniciar sua primeira busca de oportunidades.</p>`;
    } else if (withoutMatch.length > 0) {
      const target = withoutMatch[0];
      nextEl.innerHTML = `<h2>Próxima ação recomendada</h2>
        <p class="mut" style="margin-top:6px"><strong>${withoutMatch.length} ${withoutMatch.length === 1 ? 'vaga está' : 'vagas estão'} sem análise de match</strong> contra o seu currículo.</p>
        <div class="next-job-preview" style="margin-top:10px">
          <strong title="${esc(target.title)}">${esc(target.title)}</strong>
          <span class="mut" style="font-size:12px;display:block">${esc([target.company, target.location].filter(Boolean).join(' · '))}</span>
        </div>
        <div class="row" style="margin-top:10px">
          <button type="button" class="btn primary sm" data-open="${target.id}">Avaliar próxima vaga</button>
        </div>`;
    } else {
      nextEl.innerHTML = `<h2>Triagem em dia</h2><p class="mut" style="margin-top:6px">Todas as vagas ativas já possuem resumo estruturado e nota de compatibilidade.</p>`;
    }
  }

  /* Distribuição por Etapa */
  const distEl = $('#stage-dist');
  if (distEl) {
    distEl.innerHTML = `<h2>Distribuição por etapa</h2>
      <div class="dist-list" style="margin-top:8px">
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

  /* Atividade Operacional Recente */
  const actBody = $('#last-activity-body');
  if (actBody) {
    if (S.execs && S.execs.length > 0) {
      const last = S.execs[0];
      const statusCls = last.status === 'COMPLETED' ? 'ok' : last.status === 'FAILED' ? 'bad' : 'warn';
      const processed = last.processed_count != null ? last.processed_count : (last.items_processed != null ? last.items_processed : 0);
      const failed = last.failed_count != null ? last.failed_count : (last.items_failed != null ? last.items_failed : 0);
      const origin = last.request_origin || last.source || 'Sistema';
      const time = (last.started_at || last.created_at || '').replace('T', ' ').slice(0, 19);
      actBody.innerHTML = `<div>
        <div class="between row" style="align-items:center">
          <strong style="font-size:13px">${esc(formatExecType(last.execution_type))}</strong>
          <span class="chip ${statusCls}">${esc(last.status)}</span>
        </div>
        <p class="mut" style="margin:4px 0 0;font-size:12px">${esc(origin)} · ${processed} processados (${failed} falhas)</p>
        <span class="mut" style="font-family:var(--font-mono);font-size:11px">${esc(time)}</span>
      </div>`;
    } else {
      actBody.innerHTML = `<p class="mut">Nenhuma operação recente registrada.</p>`;
    }
  }

  /* Automação & Rondas de Busca */
  const stMsg = $('#auto-status-msg');
  const listEl = $('#auto-searches');
  if (stMsg && S.auto && S.auto.status) {
    const st = S.auto.status;
    stMsg.textContent = `${st.is_running_cycle ? 'Executando ciclo agora…' : st.last_cycle_summary} (Coletadas: ${st.total_auto_collected} · Enriquecidas: ${st.total_auto_enriched} · Auto-Matches: ${st.total_auto_matched})`;
  }
  if (listEl && S.auto) {
    const searches = S.auto.searches || [];
    listEl.innerHTML =
      searches
        .map(
          (s) => `<li style="display:flex;justify-content:space-between;align-items:center;padding:8px 4px;border-bottom:1px solid var(--border);gap:8px">
            <div>
              <strong>${esc(s.query)}</strong>
              <span class="mut"> · ${esc(s.remote ? 'Remoto' : s.location || 'Brasil')} · a cada ${s.interval_minutes} min</span>
              ${s.last_run_at ? `<span class="mut" style="font-size:11px"> (última: ${esc(s.last_run_at)} · +${s.last_found_count} novas)</span>` : ''}
            </div>
            <div class="row">
              <button type="button" class="btn ghost sm" data-toggle-search="${s.id}" data-enabled="${s.enabled ? '1' : '0'}">${s.enabled ? icon('check') + ' Ativa' : 'Pausada'}</button>
              <button type="button" class="btn ghost sm danger" data-del-search="${s.id}">Excluir</button>
            </div>
          </li>`
        )
        .join('') || '<li class="empty">Nenhuma ronda automática cadastrada.</li>';
  }
}

/* ---------- Tela: Funil (Coluna Descoberta e Descartada recolhidas por padrão + Drag-and-Drop) ---------- */
function renderFunil() {
  const COLLAPSIBLE = ['descoberta', 'descartada'];
  const LIMIT = 3;

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
  ensureProfile();
  if (S.profile) {
    $('#p-city').value = S.profile.base_city || 'Araraquara';
    $('#p-region').value = S.profile.accepted_region || '';
    $('#p-shifts').value = S.profile.accepted_shifts || '';
    $('#p-salary').value = S.profile.min_salary_brl || '';
    $('#p-exclude').value = S.profile.exclude_terms || '';
    $('#p-threshold').value = S.profile.auto_match_threshold ?? 0.62;
    $('#p-remote').checked = Boolean(S.profile.accept_remote);
    $('#p-auto-enrich').checked = Boolean(S.profile.auto_enrich);
    $('#p-notify').checked = Boolean(S.profile.notify_on_apply);
  }
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

async function handlePdfFile(file) {
  if (!file) return;
  if (!file.name.toLowerCase().endsWith('.pdf') && file.type !== 'application/pdf') {
    toast('Selecione um arquivo no formato PDF (.pdf).', true);
    return;
  }

  const dropzone = $('#cv-dropzone');
  const loading = $('#cv-dropzone-loading');
  const statusEl = $('#cv-dropzone-status');
  const feedback = $('#cv-dropzone-feedback');
  const btnSelect = $('#btn-select-pdf');
  const btnImport = $('#btn-import-pdf');

  if (loading) loading.hidden = false;
  if (btnSelect) btnSelect.disabled = true;
  if (btnImport) btnImport.disabled = true;
  if (statusEl) statusEl.textContent = 'Extraindo texto do PDF e estruturando seções em Markdown com IA...';
  if (feedback) feedback.hidden = true;

  try {
    const res = await API.uploadResumePdf(file);

    if (res.content) {
      $('#cv').value = res.content;
      updateCvDirtyIndicator();

      if (feedback) {
        feedback.hidden = false;
        const aiBadge = res.ai_structured ? 'Estruturado com IA' : 'Texto extraído localmente';
        feedback.innerHTML = `
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:var(--ok)"><polyline points="20 6 9 17 4 12"></polyline></svg>
          <span>Arquivo <strong>${esc(file.name)}</strong> importado (${res.char_count} caracteres). Modo: <strong>${aiBadge}</strong>.</span>
        `;
      }

      // Sugestão de cidade/localização se ainda estiver padrão/vazia
      if (res.suggested_location && (!($('#p-city').value) || $('#p-city').value === 'Araraquara')) {
        $('#p-city').value = res.suggested_location;
      }

      toast(
        res.ai_structured
          ? 'Currículo importado e estruturado com sucesso pelo Gemini! Revise os campos e clique em Salvar currículo.'
          : 'Texto do PDF extraído com sucesso! Revise o texto no editor e clique em Salvar currículo.'
      );
    }
  } catch (err) {
    console.error('Falha no upload do PDF:', err);
    toast(`Erro ao processar PDF: ${err.message || 'Arquivo ilegível'}`, true);
    if (feedback) {
      feedback.hidden = false;
      feedback.innerHTML = `<span style="color:var(--bad)">Falha ao importar ${esc(file.name)}: ${esc(err.message || 'Erro desconhecido')}</span>`;
    }
  } finally {
    if (loading) loading.hidden = true;
    if (btnSelect) btnSelect.disabled = false;
    if (btnImport) btnImport.disabled = false;
    const input = $('#cv-pdf-input');
    if (input) input.value = '';
  }
}

function initResumePdfUpload() {
  const dropzone = $('#cv-dropzone');
  const input = $('#cv-pdf-input');
  const btnSelect = $('#btn-select-pdf');
  const btnImport = $('#btn-import-pdf');

  if (btnSelect && input) {
    btnSelect.addEventListener('click', () => input.click());
  }
  if (btnImport && input) {
    btnImport.addEventListener('click', () => input.click());
  }

  if (input) {
    input.addEventListener('change', (e) => {
      const file = e.target.files?.[0];
      if (file) handlePdfFile(file);
    });
  }

  if (dropzone) {
    ['dragenter', 'dragover'].forEach((eventName) => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.add('dragover');
      });
    });

    ['dragleave', 'drop'].forEach((eventName) => {
      dropzone.addEventListener(eventName, (e) => {
        e.preventDefault();
        e.stopPropagation();
        dropzone.classList.remove('dragover');
      });
    });

    dropzone.addEventListener('drop', (e) => {
      const dt = e.dataTransfer;
      const file = dt?.files?.[0];
      if (file) handlePdfFile(file);
    });
  }
}

/* ---------- Observabilidade e Execuções (Sprint 08) ---------- */

let execEventSource = null;

function initExecutionsSSE() {
  if (execEventSource) return;
  try {
    execEventSource = new EventSource('/api/executions/stream');
    execEventSource.addEventListener('execution_event', (e) => {
      try {
        const msg = JSON.parse(e.data);
        handleExecutionMessage(msg);
      } catch (err) {
        console.error('Erro ao processar mensagem SSE:', err);
      }
    });

    execEventSource.onopen = () => {
      const chip = $('#sse-status');
      if (chip) {
        chip.textContent = 'SSE Conectado';
        chip.className = 'chip ok';
      }
    };

    execEventSource.onerror = () => {
      const chip = $('#sse-status');
      if (chip) {
        chip.textContent = 'Reconectando SSE…';
        chip.className = 'chip warn';
      }
    };
  } catch (err) {
    console.warn('Falha ao inicializar EventSource:', err);
  }
}

function handleExecutionMessage(msg) {
  if (!msg || !msg.type) return;
  const { type, payload } = msg;

  if (type === 'ExecutionCreated') {
    S.activeExec = payload;
    updateActiveExecCard(payload);
    showFloatingPill(payload);
    $('#exec-badge')?.removeAttribute('hidden');
    if (cur() === 'execucoes') loadAndRenderExecutionsHistory();
  } else if (type === 'ExecutionUpdated') {
    if (S.activeExec && S.activeExec.id === payload.id) {
      S.activeExec = payload;
      updateActiveExecCard(payload);
      showFloatingPill(payload);
    }
    const isTerminal = ['COMPLETED', 'PARTIALLY_COMPLETED', 'FAILED', 'CANCELLED'].includes(payload.status);
    if (isTerminal) {
      $('#exec-badge')?.setAttribute('hidden', '');
      setTimeout(() => {
        hideFloatingPill();
      }, 4000);
      if (cur() === 'execucoes') loadAndRenderExecutionsHistory();
    }
  } else if (type === 'StageStarted' || type === 'StageCompleted') {
    if (S.activeExec && S.activeExec.id === payload.execution_id) {
      const stageLabel = $('#act-stage-label');
      if (stageLabel) stageLabel.textContent = `Etapa: ${payload.stage_name} (${payload.status})`;
      const curStage = $('#act-current-stage');
      if (curStage) curStage.textContent = payload.stage_name;
    }
  } else if (type === 'EventLogged') {
    appendEventToActiveLog(payload);
  }
}

function updateActiveExecCard(exec) {
  const card = $('#active-exec-card');
  if (!card) return;
  const isTerminal = ['COMPLETED', 'PARTIALLY_COMPLETED', 'FAILED', 'CANCELLED'].includes(exec.status);
  card.hidden = isTerminal;
  if (isTerminal) return;

  const titleEl = $('#act-title');
  if (titleEl) titleEl.textContent = `${formatExecType(exec.execution_type)} (via ${exec.request_origin})`;
  const idEl = $('#act-id');
  if (idEl) idEl.textContent = `#${exec.id}`;
  const statusEl = $('#act-status');
  if (statusEl) {
    statusEl.textContent = exec.status;
    statusEl.className = `badge ${exec.status.toLowerCase()} pulse`;
  }
  const pct = Math.round(exec.progress_pct || 0);
  const progLabel = $('#act-progress-label');
  if (progLabel) progLabel.textContent = `${pct}%`;
  const progFill = $('#act-progress-fill');
  if (progFill) progFill.style.width = `${pct}%`;
  const stageLabel = $('#act-stage-label');
  if (stageLabel) stageLabel.textContent = `Etapa: ${exec.current_stage || 'Iniciando…'}`;
  const curStage = $('#act-current-stage');
  if (curStage) curStage.textContent = exec.current_stage || '—';
  const procEl = $('#act-processed');
  if (procEl) procEl.textContent = exec.processed_count;
  const failEl = $('#act-failed');
  if (failEl) failEl.textContent = exec.failed_count;
}

function appendEventToActiveLog(evt) {
  const box = $('#act-events-stream');
  if (!box) return;
  const time = evt.created_at ? evt.created_at.slice(11, 19) : new Date().toLocaleTimeString();
  const lvl = (evt.level || 'INFO').toLowerCase();
  const src = evt.source ? `[${evt.source}] ` : '';
  const rowHtml = `<div class="event-row">
    <span class="event-time">${time}</span>
    <span class="event-tag ${lvl}">[${lvl.toUpperCase()}]</span>
    <span>${esc(src)}${esc(evt.message)}</span>
  </div>`;
  box.insertAdjacentHTML('beforeend', rowHtml);
  box.scrollTop = box.scrollHeight;
}

function showFloatingPill(exec) {
  const pill = $('#floating-exec-pill');
  if (!pill) return;
  const isTerminal = ['COMPLETED', 'PARTIALLY_COMPLETED', 'FAILED', 'CANCELLED'].includes(exec.status);
  if (isTerminal) {
    const pText = $('#pill-text');
    if (pText) pText.textContent = `${formatExecType(exec.execution_type)}: ${exec.status}`;
    const pPct = $('#pill-pct');
    if (pPct) pPct.textContent = '100%';
    return;
  }
  pill.hidden = false;
  const pText = $('#pill-text');
  if (pText) pText.textContent = `${formatExecType(exec.execution_type)} em andamento…`;
  const pPct = $('#pill-pct');
  if (pPct) pPct.textContent = `${Math.round(exec.progress_pct || 0)}%`;
}

function hideFloatingPill() {
  const pill = $('#floating-exec-pill');
  if (pill) pill.hidden = true;
}

function formatExecType(t) {
  const map = {
    JOB_SEARCH: 'Busca de Vagas',
    COMPANY_SEARCH: 'Busca de Empresas',
    COMPANY_MONITORING: 'Monitoramento',
    AI_ENRICHMENT: 'Enriquecimento IA',
    JOB_SCREENING: 'Screening / Ranking',
    MATCH_ANALYSIS: 'Análise de Match',
    AUTOMATION: 'Automação',
  };
  return map[t] || t;
}

async function loadAndRenderExecutionsHistory() {
  const typeFilter = $('#filter-exec-type')?.value || '';
  const statusFilter = $('#filter-exec-status')?.value || '';
  const params = { limit: 50 };
  if (typeFilter) params.type = typeFilter;
  if (statusFilter) params.status = statusFilter;

  try {
    const list = await API.executions(params);
    S.execs = Array.isArray(list) ? list : [];
    renderExecutionsTable(S.execs);
  } catch (err) {
    const tbody = $('#execs-table-body');
    if (tbody) {
      tbody.innerHTML = `<tr><td colspan="10" class="mut" style="color:var(--bad);text-align:center;padding:16px">Falha ao carregar histórico: ${esc(err.message)}</td></tr>`;
    }
  }
}

function renderExecutionsTable(list) {
  const tbody = $('#execs-table-body');
  if (!tbody) return;
  if (!list.length) {
    tbody.innerHTML = '<tr><td colspan="10" class="mut" style="text-align:center;padding:16px">Nenhuma execução registrada até o momento.</td></tr>';
    return;
  }

  tbody.innerHTML = list.map((e) => {
    const durStr = e.duration_ms != null ? `${(e.duration_ms / 1000).toFixed(1)}s` : '—';
    const dateStr = e.started_at ? e.started_at.slice(0, 19).replace('T', ' ') : '—';
    const stCls = e.status.toLowerCase();
    const isRunning = e.status === 'RUNNING' || e.status === 'PENDING';
    const canRetry = e.status === 'FAILED' || e.status === 'CANCELLED' || e.status === 'PARTIALLY_COMPLETED';

    return `<tr>
      <td><code style="font-size:12px">${esc(e.id)}</code></td>
      <td><strong>${esc(formatExecType(e.execution_type))}</strong></td>
      <td><span class="mut">${esc(e.request_origin)}</span></td>
      <td><span class="badge ${stCls}">${esc(e.status)}</span></td>
      <td>${Math.round(e.progress_pct)}%</td>
      <td>${e.processed_count}</td>
      <td>${e.failed_count > 0 ? `<span style="color:var(--bad)">${e.failed_count}</span>` : '0'}</td>
      <td>${durStr}</td>
      <td><span class="mut" style="font-size:12px">${dateStr}</span></td>
      <td>
        <div class="row" style="gap:4px">
          <button type="button" class="btn ghost sm" data-open-exec="${esc(e.id)}">Detalhes</button>
          ${isRunning ? `<button type="button" class="btn danger sm" data-cancel-exec="${esc(e.id)}">Cancelar</button>` : ''}
          ${canRetry ? `<button type="button" class="btn sm" data-retry-exec="${esc(e.id)}">Reexecutar</button>` : ''}
        </div>
      </td>
    </tr>`;
  }).join('');
}

async function loadAndRenderExecutionsSummary() {
  try {
    const s = await API.executionsSummary();
    if (!s) return;
    if ($('#st-exec-active')) $('#st-exec-active').textContent = s.active_executions || 0;
    if ($('#st-exec-completed')) $('#st-exec-completed').textContent = s.completed_executions || 0;
    if ($('#st-exec-failed')) $('#st-exec-failed').textContent = s.failed_executions || 0;
    if ($('#st-exec-jobs')) $('#st-exec-jobs').textContent = s.total_collected_jobs || 0;
    if ($('#st-exec-sources')) $('#st-exec-sources').textContent = s.total_sources_tracked || 0;
    if ($('#st-exec-ai-ops')) $('#st-exec-ai-ops').textContent = s.total_ai_operations || 0;
    if ($('#st-exec-avg-time')) {
      const avgSec = s.avg_duration_ms > 0 ? (s.avg_duration_ms / 1000).toFixed(1) + 's' : '0s';
      $('#st-exec-avg-time').textContent = avgSec;
    }
  } catch (err) {
    console.warn('Falha ao carregar resumo de execuções:', err);
  }
}

async function loadAndRenderCollectorsMonitor() {
  const grid = $('#collectors-monitor-grid');
  if (!grid) return;
  try {
    const cols = await API.collectors();
    if (!Array.isArray(cols)) return;
    grid.innerHTML = cols.map((c) => {
      const st = c.last_status || (c.enabled ? 'IDLE' : 'DISABLED');
      const stClass = st === 'SUCCESS' ? 'ok' : st === 'BLOCKED' || st === 'ERROR' ? 'bad' : st === 'EMPTY' ? 'warn' : '';
      const runTime = c.last_run_at ? c.last_run_at.slice(11, 19) : '—';
      return `<div class="collector-monitor-pill">
        <span><strong>${esc(c.display_name)}</strong> <span class="mut" style="font-size:11px">(${esc(c.group_name)})</span></span>
        <span class="chip ${stClass}">${esc(st)}</span>
        <span class="mut" style="font-size:11px">${c.last_count} vagas · ${runTime}</span>
      </div>`;
    }).join('') || '<p class="mut" style="font-size:12px">Nenhum coletor configurado.</p>';
  } catch (err) {
    grid.innerHTML = `<span class="mut" style="font-size:12px">Monitor de coletores indisponível: ${esc(err.message)}</span>`;
  }
}

async function renderExecucoes() {
  initExecutionsSSE();
  if (S.activeExec) {
    updateActiveExecCard(S.activeExec);
  } else {
    API.executions({ status: 'RUNNING', limit: 1 }).then((res) => {
      if (Array.isArray(res) && res.length > 0) {
        S.activeExec = res[0];
        updateActiveExecCard(res[0]);
      } else {
        const card = $('#active-exec-card');
        if (card) card.hidden = true;
      }
    }).catch(() => {});
  }
  await Promise.all([
    loadAndRenderExecutionsSummary(),
    loadAndRenderCollectorsMonitor(),
    loadAndRenderExecutionsHistory(),
    loadAndRenderAiTelemetry(),
  ]);
}

async function loadAndRenderAiTelemetry() {
  const box = $('#ai-telemetry-summary');
  if (!box) return;
  try {
    const tel = await API.aiTelemetry();
    S.aiTelemetry = tel;
    const ops = Array.isArray(tel.recent_operations) ? tel.recent_operations.slice(0, 6) : [];
    box.innerHTML = `
      <div class="chips" style="width:100%;gap:10px">
        <span class="chip ok"><strong>Operações IA:</strong> ${tel.total_operations || 0}</span>
        <span class="chip"><strong>Tokens Totais:</strong> ${(tel.total_tokens || 0).toLocaleString('pt-BR')} (In: ${(tel.total_prompt_tokens || 0).toLocaleString('pt-BR')} / Out: ${(tel.total_completion_tokens || 0).toLocaleString('pt-BR')})</span>
        <span class="chip ok"><strong>Cache BLAKE3:</strong> ${tel.cache_entries || 0} entradas · ${tel.total_cache_hits || 0} hits economizados</span>
        <span class="chip ${tel.fallback_count > 0 ? 'warn' : ''}"><strong>Fallbacks de Modelo:</strong> ${tel.fallback_count || 0}</span>
        <span class="chip"><strong>Latência Média:</strong> ${Math.round(tel.avg_duration_ms || 0)}ms</span>
      </div>
      ${
        ops.length
          ? `<div class="table-responsive" style="width:100%;margin-top:8px"><table class="exec-table" style="font-size:12px">
              <thead><tr><th>Operação</th><th>Modelo Efetivo</th><th>Status</th><th>Registros</th><th>Tokens</th><th>Duração</th><th>Horário</th></tr></thead>
              <tbody>
                ${ops
                  .map(
                    (o) => `<tr>
                      <td><strong>${esc(o.operation_type)}</strong></td>
                      <td><code>${esc(o.model_used)}</code>${o.fallback_used ? ' <span class="chip warn" style="font-size:10px">Fallback</span>' : ''}</td>
                      <td><span class="chip ${o.status === 'SUCCESS' || o.status === 'CACHE_HIT' ? 'ok' : 'bad'}">${esc(o.status)}</span></td>
                      <td>${o.records_count}</td>
                      <td>${o.total_tokens}</td>
                      <td>${o.duration_ms}ms</td>
                      <td class="mut">${esc((o.started_at || '').slice(0, 19))}</td>
                    </tr>`
                  )
                  .join('')}
              </tbody>
            </table></div>`
          : '<p class="mut" style="font-size:12px">Nenhuma chamada Gemini registrada ainda nesta sessão.</p>'
      }
    `;
  } catch (err) {
    box.innerHTML = `<span class="mut">Telemetria indisponível: ${esc(err.message)}</span>`;
  }
}

/* ---------- Sprint 11: Company Intelligence ---------- */
async function renderEmpresas() {
  const listEl = $('#companies-list');
  if (!listEl) return;
  try {
    const list = await API.companies();
    S.companies = Array.isArray(list) ? list : [];
  } catch {}
  const onlyMon = Boolean($('#chk-only-monitored')?.checked);
  const filteredComps = S.companies.filter((c) => !onlyMon || c.is_monitored === 1);
  if (!S.selectedCompanyId && filteredComps.length > 0) {
    S.selectedCompanyId = filteredComps[0].id;
  }
  listEl.innerHTML =
    filteredComps
      .map((c) => {
        const isSel = c.id === S.selectedCompanyId;
        const badges = [
          c.is_monitored ? chip('Monitorada', 'ok') : '',
          c.is_preferred ? chip('Preferida', 'ok') : '',
          c.is_excluded ? chip('Excluída', 'bad') : '',
          chip(`${c.active_jobs_count || 0} vagas`),
        ]
          .filter(Boolean)
          .join('');
        return `<li><button type="button" class="job${isSel ? ' on' : ''}" data-comp-id="${c.id}">
          <span class="job-t">
            <strong>${esc(c.official_name)}</strong>
            <span class="mut">${esc([c.segment, c.location].filter(Boolean).join(' · ') || 'Sem segmento')}</span>
            <span class="chips">${badges}</span>
          </span>
        </button></li>`;
      })
      .join('') || '<li class="empty">Nenhuma empresa cadastrada.</li>';

  if (S.selectedCompanyId) {
    openCompanyDetail(S.selectedCompanyId);
  }
}

async function openCompanyDetail(compId) {
  const pane = $('#company-detail-pane');
  if (!pane) return;
  try {
    const d = await API.companyDetail(compId);
    const c = d.company;
    const sources = d.sources || [];
    const aliases = d.aliases || [];
    const jobs = (d.jobs || []).map(norm);

    pane.innerHTML = `
      <div class="card-h">
        <div>
          <h2>${esc(c.official_name)}</h2>
          <p class="mut">${esc([c.segment, c.location, c.website].filter(Boolean).join(' · '))}</p>
        </div>
        <button type="button" class="btn primary" data-scan-company="${c.id}">Varredura ATS Agora</button>
      </div>
      ${c.description ? `<p style="margin-top:8px;font-size:13px">${esc(c.description)}</p>` : ''}
      <div style="margin-top:12px">
        <h3 style="font-size:13px">Fontes Corporativas &amp; Portais ATS (${sources.length})</h3>
        ${
          sources.length
            ? `<ul class="plain list" style="margin-top:6px">
                ${sources
                  .map(
                    (s) => `<li style="padding:6px 0;border-bottom:1px solid var(--bd);font-size:12px">
                      <div class="between row">
                        <span><strong>[${esc(s.ats_provider.toUpperCase())}]</strong> Portal: <code>${esc(s.portal_id || '—')}</code> · <a href="${esc(s.career_url)}" target="_blank" rel="noopener">${esc(s.career_url)} ↗</a></span>
                        <span class="chip ${s.integration_status === 'SUCCESS' || s.integration_status === 'ACTIVE' ? 'ok' : s.integration_status === 'EMPTY' ? 'warn' : 'bad'}">${esc(s.integration_status)} (${s.last_found_count} vagas)</span>
                      </div>
                      ${s.last_checked_at ? `<div class="mut" style="font-size:11px">Última verificação: ${esc(s.last_checked_at)}${s.last_error ? ` · Erro: ${esc(s.last_error)}` : ''}</div>` : ''}
                    </li>`
                  )
                  .join('')}
              </ul>`
            : '<p class="mut" style="font-size:12px;margin-top:4px">Nenhuma página de carreira vinculada.</p>'
        }
      </div>
      <div style="margin-top:12px">
        <h3 style="font-size:13px">Aliases / Variações de Nome (${aliases.length})</h3>
        <div class="chips" style="margin-top:4px">
          <span class="chip">${esc(c.normalized_name)} (normalizado)</span>
          ${aliases.map((a) => `<span class="chip">${esc(a.alias_name)}</span>`).join('')}
        </div>
      </div>
      <div style="margin-top:14px">
        <h3 style="font-size:13px">Oportunidades Identificadas nesta Empresa (${jobs.length})</h3>
        <ul class="plain list" style="margin-top:6px">
          ${jobs.slice(0, 15).map((j) => row(j, true)).join('') || '<li class="empty">Nenhuma vaga coletada para esta empresa ainda. Clique em "Varredura ATS Agora".</li>'}
        </ul>
      </div>
    `;
  } catch (err) {
    pane.innerHTML = `<p class="mut">Erro ao carregar detalhes da empresa: ${esc(err.message)}</p>`;
  }
}

async function openExecutionModal(id) {
  const modal = $('#exec-detail-modal');
  const content = $('#modal-exec-content');
  if (!modal || !content) return;
  openModal(modal);
  $('#modal-exec-id').textContent = `#${id}`;
  content.innerHTML = '<p class="mut" style="padding:20px;text-align:center">Carregando detalhes da execução…</p>';

  try {
    const detail = await API.executionDetail(id);
    window._currentModalDetail = detail;
    const { execution: e, stages, events, metrics } = detail;
    $('#modal-exec-title').textContent = `${formatExecType(e.execution_type)} — Status: ${e.status}`;

    const canRetry = e.status === 'FAILED' || e.status === 'CANCELLED' || e.status === 'PARTIALLY_COMPLETED';
    const isRunning = e.status === 'RUNNING' || e.status === 'PENDING';

    const actionButtons = `
      <div class="row" style="gap:8px;margin-bottom:12px;flex-wrap:wrap">
        <button type="button" class="btn ghost sm" data-copy-exec-id="${esc(e.id)}">${icon('copy')} Copiar ID</button>
        <button type="button" class="btn ghost sm" data-export-logs="${esc(e.id)}">${icon('download')} Exportar Logs (JSON)</button>
        ${canRetry ? `<button type="button" class="btn sm" data-retry-exec="${esc(e.id)}">${icon('refresh')} Reexecutar</button>` : ''}
        ${isRunning ? `<button type="button" class="btn danger sm" data-cancel-exec="${esc(e.id)}">${icon('stop')} Cancelar</button>` : ''}
      </div>
    `;

    // Timeline Visual de Etapas (Sprint 14.4)
    let timelineHtml = '<p class="mut">Nenhuma etapa registrada.</p>';
    if (stages && stages.length > 0) {
      timelineHtml = `<div class="timeline">
        ${stages.map((s) => {
          const stCls = s.status.toLowerCase();
          const durStr = s.duration_ms != null ? `${s.duration_ms}ms` : '—';
          return `<div class="timeline-step">
            <div class="timeline-dot ${stCls}"></div>
            <div class="timeline-content">
              <span><strong>${esc(s.stage_name)}</strong> <span class="badge ${stCls}">${esc(s.status)}</span></span>
              <span class="mut" style="font-size:12px">In: ${s.items_in} | Out: ${s.items_out} | ${durStr}</span>
            </div>
            ${s.error_message ? `<div style="color:var(--bad);font-size:12px;margin-top:2px">${esc(s.error_message)}</div>` : ''}
          </div>`;
        }).join('')}
      </div>`;
    }

    let metricsHtml = '<p class="mut">Nenhuma métrica registrada.</p>';
    if (metrics && metrics.length > 0) {
      metricsHtml = `<div class="chips" style="gap:8px">
        ${metrics.map((m) => `<div class="chip" style="padding:6px 10px;font-size:12px">
          <span class="mut">${esc(m.metric_name)}:</span> <strong>${m.metric_value} ${esc(m.metric_unit)}</strong>
        </div>`).join('')}
      </div>`;
    }

    // Barra de Filtros e Busca no Log (Sprint 14.5)
    const logFilterBar = `
      <div class="log-filter-bar">
        <input type="text" id="modal-log-search" placeholder="Filtrar eventos por mensagem ou origem…" style="flex:1;min-width:180px">
        <select id="modal-log-level" class="sel-fixed" style="font-size:12px">
          <option value="">Todos os níveis</option>
          <option value="INFO">INFO</option>
          <option value="WARNING">WARNING</option>
          <option value="ERROR">ERROR</option>
          <option value="DEBUG">DEBUG</option>
        </select>
      </div>
    `;

    content.innerHTML = `
      <div class="stack" style="gap:16px">
        ${actionButtons}
        <div class="two">
          <div><span class="mut">Início:</span> <strong>${esc(e.started_at)}</strong></div>
          <div><span class="mut">Término:</span> <strong>${esc(e.completed_at || 'Em andamento')}</strong></div>
          <div><span class="mut">Duração:</span> <strong>${e.duration_ms != null ? (e.duration_ms / 1000).toFixed(2) + 's' : '—'}</strong></div>
          <div><span class="mut">Origem:</span> <strong>${esc(e.request_origin)}</strong></div>
        </div>
        ${e.diagnostic_message ? `<div class="alert-item info"><strong>Diagnóstico:</strong> ${esc(e.diagnostic_message)}</div>` : ''}
        <div>
          <h3 style="font-size:14px;margin-bottom:8px">Timeline de Execução (Etapas)</h3>
          ${timelineHtml}
        </div>
        <div>
          <h3 style="font-size:14px;margin-bottom:8px">Métricas de Desempenho</h3>
          ${metricsHtml}
        </div>
        <div>
          <h3 style="font-size:14px;margin-bottom:8px">Log Cronológico Técnico</h3>
          ${logFilterBar}
          <div id="modal-events-box" class="event-stream-box" style="max-height:280px"></div>
        </div>
      </div>
    `;
    filterAndRenderModalEvents();
  } catch (err) {
    content.innerHTML = `<p style="color:var(--bad);padding:20px">Erro ao carregar detalhes: ${esc(err.message)}</p>`;
  }
}

function filterAndRenderModalEvents() {
  const box = $('#modal-events-box');
  if (!box || !window._currentModalDetail) return;
  const q = ($('#modal-log-search')?.value || '').toLowerCase();
  const lvl = ($('#modal-log-level')?.value || '').toUpperCase();
  const events = window._currentModalDetail.events || [];

  const filtered = events.filter((evt) => {
    if (lvl && (evt.level || 'INFO').toUpperCase() !== lvl) return false;
    if (q) {
      const msg = (evt.message || '').toLowerCase();
      const src = (evt.source || '').toLowerCase();
      const stg = (evt.stage || '').toLowerCase();
      if (!msg.includes(q) && !src.includes(q) && !stg.includes(q)) return false;
    }
    return true;
  });

  if (!filtered.length) {
    box.innerHTML = '<p class="mut" style="padding:10px;text-align:center">Nenhum evento corresponde ao filtro.</p>';
    return;
  }

  box.innerHTML = filtered.map((evt) => {
    const time = evt.created_at ? evt.created_at.slice(11, 19) : '';
    const l = (evt.level || 'INFO').toLowerCase();
    const src = evt.source ? `[${evt.source}] ` : '';
    return `<div class="event-row">
      <span class="event-time">${time}</span>
      <span class="event-tag ${l}">[${l.toUpperCase()}]</span>
      <span><strong>${esc(src)}</strong>${esc(evt.message)}</span>
    </div>`;
  }).join('');
}

/* ---------- Roteamento (Suporte a #vagas/12) ---------- */
const VIEWS = ['painel', 'vagas', 'empresas', 'funil', 'curriculo', 'execucoes'];

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
    const el = $('#v-' + x);
    if (el) el.hidden = x !== view;
  });
  document
    .querySelectorAll('[data-nav]')
    .forEach((a) =>
      a.dataset.nav === view
        ? a.setAttribute('aria-current', 'page')
        : a.removeAttribute('aria-current')
    );
  ({
    painel: renderPainel,
    vagas: renderVagas,
    empresas: renderEmpresas,
    funil: renderFunil,
    curriculo: renderCurriculo,
    execucoes: renderExecucoes,
  })[view]();
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

let activeModal = null;
let modalReturnFocus = null;

function openModal(modal) {
  if (!modal) return;
  modalReturnFocus = document.activeElement;
  activeModal = modal;
  modal.hidden = false;
  document.body.classList.add('modal-open');
  requestAnimationFrame(() => {
    const first = modal.querySelector('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])');
    (first || modal).focus({ preventScroll: true });
  });
}

function closeModal(modal = activeModal) {
  if (!modal) return;
  modal.hidden = true;
  if (activeModal === modal) activeModal = null;
  document.body.classList.remove('modal-open');
  const target = modalReturnFocus;
  modalReturnFocus = null;
  if (target && document.contains(target)) target.focus({ preventScroll: true });
}

document.addEventListener('keydown', (e) => {
  if (!activeModal) return;
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopImmediatePropagation();
    closeModal();
    return;
  }
  if (e.key !== 'Tab') return;
  const focusable = [...activeModal.querySelectorAll('button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])')]
    .filter((el) => !el.disabled && el.getClientRects().length > 0);
  if (!focusable.length) {
    e.preventDefault();
    activeModal.focus();
    return;
  }
  const first = focusable[0];
  const last = focusable[focusable.length - 1];
  if (e.shiftKey && document.activeElement === first) {
    e.preventDefault();
    last.focus();
  } else if (!e.shiftKey && document.activeElement === last) {
    e.preventDefault();
    first.focus();
  }
});

document.addEventListener('click', (e) => {
  if (e.target.classList?.contains('modal-backdrop')) {
    closeModal(e.target);
    return;
  }
  if (e.target.id === 'nav-toggle' || e.target.closest('#nav-toggle')) {
    const toggle = $('#nav-toggle');
    const nav = $('#primary-nav');
    const open = !nav.classList.contains('is-open');
    nav.classList.toggle('is-open', open);
    toggle.setAttribute('aria-expanded', String(open));
    toggle.textContent = open ? 'Fechar' : 'Menu';
    return;
  }
  if (e.target.closest('[data-nav]')) {
    $('#primary-nav').classList.remove('is-open');
    $('#nav-toggle').setAttribute('aria-expanded', 'false');
    $('#nav-toggle').textContent = 'Menu';
    return;
  }
  if (e.target.id === 'btn-toggle-filters' || e.target.closest('#btn-toggle-filters')) {
    const toggle = $('#btn-toggle-filters');
    const filters = $('#vagas-filter-secondary');
    const open = !filters.classList.contains('is-open');
    filters.classList.toggle('is-open', open);
    toggle.setAttribute('aria-expanded', String(open));
    return;
  }
  if (e.target.closest('[data-back-vagas]')) {
    go('vagas');
    return;
  }
  if (e.target.id === 'btn-dismiss-onboarding' || e.target.closest('#btn-dismiss-onboarding')) {
    localStorage.setItem('jh-onboarding-dismissed', '1');
    const banner = $('#onboarding-banner');
    if (banner) banner.hidden = true;
    return;
  }
  if (e.target.id === 'btn-open-guide' || e.target.closest('#btn-open-guide')) {
    openModal($('#guide-modal'));
    return;
  }
  if (e.target.id === 'guide-modal-close' || e.target.closest('#guide-modal-close')) {
    closeModal($('#guide-modal'));
    return;
  }
  if (e.target.id === 'btn-toggle-options' || e.target.closest('#btn-toggle-options')) {
    const d = $('#search-options-drawer');
    if (d) d.hidden = !d.hidden;
    return;
  }
  const t = e.target.closest(
    '[data-id],[data-open],[data-tab],[data-act],[data-move],[data-stage-to],[data-toggle-col],[data-toggle-search],[data-del-search]'
  );
  if (!t) return;
  const d = t.dataset;
  if (d.id) {
    go('vagas', +d.id);
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
  } else if (d.toggleSearch) {
    const id = +d.toggleSearch;
    const nextEnabled = d.enabled !== '1';
    run(t, async () => {
      await API.toggleSearch(id, nextEnabled);
      await load();
    }, nextEnabled ? 'Ronda ativada.' : 'Ronda pausada.');
  } else if (d.delSearch) {
    const id = +d.delSearch;
    run(t, async () => {
      await API.deleteSearch(id);
      await load();
    }, 'Ronda removida.');
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

$('#btn-consolidate-dup')?.addEventListener('click', (e) => {
  run(e.currentTarget, async () => {
    const res = await API.consolidateDuplicates();
    await load();
    toast(`Consolidação concluída: ${res.consolidated_count || 0} vagas duplicadas unificadas entre fontes distintas.`);
  });
});

$('#clear-f')?.addEventListener('click', () => {
  S.f = { q: '', local: '', modo: '', turno: '', etapa: '', fonte: '', triagem: '' };
  saveFilters();
  syncFilterInputs();
  renderVagas();
});

document.addEventListener('change', (e) => {
  if (e.target.id === 'f-sort') {
    renderVagas();
  } else if (e.target.dataset.act === 'etapa') {
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
  } else if (e.target.id === 'modal-log-search') {
    filterAndRenderModalEvents();
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
    if (rep.source_diagnostics) {
      renderSourceDiagnosticsBanner(rep.source_diagnostics);
    }
    const ins = rep.newly_inserted || 0;
    const dup = rep.duplicates_skipped || 0;
    const breakdown = `Gupy: ${rep.gupy_count || 0}, LinkedIn: ${rep.linkedin_count || 0}, Indeed: ${rep.indeed_count || 0}, Glassdoor: ${rep.glassdoor_count || 0}, Vagas.com: ${rep.vagas_com_count || 0}`;
    if (ins === 0) {
      toast(
        dup > 0
          ? `Nenhuma vaga inédita (${dup} já estavam no banco). [${breakdown}]`
          : `Nenhuma vaga encontrada para “${query}”. [${breakdown}]`
      );
      return;
    }
    toast(
      `${ins} ${ins === 1 ? 'nova vaga coletada' : 'novas vagas coletadas'} (${breakdown}).`,
      false,
      null
    );
    if (rep.warnings && rep.warnings.length > 0) {
      setTimeout(() => toast(`Aviso na coleta: ${rep.warnings[0]}`, true), 3500);
    }
    if (wantEnrich) {
      await enrichAllPending(null);
    }
  });
});

$('#save-cv').addEventListener('click', (e) => saveCurriculum(e.currentTarget));

$('#btn-run-auto')?.addEventListener('click', (e) => {
  run(e.currentTarget, async () => {
    toast('Executando ronda completa (coleta + enriquecimento + auto-match)…');
    const res = await API.runAutoNow();
    await load();
    toast(res.summary || 'Ronda concluída.');
  });
});

$('#f-new-search')?.addEventListener('submit', (e) => {
  e.preventDefault();
  const f = new FormData(e.target);
  run(e.submitter, async () => {
    await API.addSearch({
      query: String(f.get('query') || '').trim(),
      location: String(f.get('location') || '').trim(),
      remote: Boolean(f.get('remote')),
      interval_minutes: Number(f.get('interval_minutes') || 180),
    });
    e.target.reset();
    await load();
  }, 'Nova ronda de busca adicionada.');
});

$('#f-profile')?.addEventListener('submit', (e) => {
  e.preventDefault();
  const payload = {
    id: 1,
    base_city: $('#p-city').value.trim(),
    accepted_region: $('#p-region').value.trim(),
    accepted_shifts: $('#p-shifts').value.trim(),
    min_salary_brl: $('#p-salary').value.trim(),
    exclude_terms: $('#p-exclude').value.trim(),
    auto_match_threshold: Number($('#p-threshold').value || 0.62),
    accept_remote: $('#p-remote').checked ? 1 : 0,
    auto_enrich: $('#p-auto-enrich').checked ? 1 : 0,
    notify_on_apply: $('#p-notify').checked ? 1 : 0,
  };
  run($('#save-profile'), async () => {
    await API.saveProfile(payload);
    S.profile = payload;
    S.cvStale = true;
    $('#cv-stale').hidden = false;
  }, 'Critérios operacionais salvos com sucesso.');
});

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
      $('.job-item-card.on, .job.on')?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'k' || e.key === 'ArrowUp') {
      e.preventDefault();
      if (!ids.length) return;
      S.sel = ids[Math.max(i - 1, 0)];
      history.replaceState(null, '', `#vagas/${S.sel}`);
      renderVagas();
      $('.job-item-card.on, .job.on')?.scrollIntoView({ block: 'nearest' });
    } else if (e.key === 'm') calcMatch(null);
    else if (e.key === 's') nextStage();
  }
});

document.addEventListener('change', (e) => {
  if (e.target.dataset.f === 'fonte') {
    S.f.fonte = e.target.value;
    saveFilters();
    renderVagas();
  } else if (e.target.id === 'filter-exec-type' || e.target.id === 'filter-exec-status') {
    loadAndRenderExecutionsHistory();
  } else if (e.target.id === 'modal-log-level') {
    filterAndRenderModalEvents();
  }
});

document.addEventListener('click', (e) => {
  const copyBtn = e.target.closest('[data-copy-exec-id]');
  if (copyBtn) {
    const id = copyBtn.dataset.copyExecId;
    navigator.clipboard?.writeText(id).then(() => {
      toast(`ID #${id} copiado para a área de transferência.`);
    }).catch(() => {
      toast(`ID: ${id}`);
    });
    return;
  }
  const exportBtn = e.target.closest('[data-export-logs]');
  if (exportBtn) {
    if (window._currentModalDetail) {
      const jsonStr = JSON.stringify(window._currentModalDetail, null, 2);
      const blob = new Blob([jsonStr], { type: 'application/json' });
      const url = URL.createObjectURL(blob);
      const a = document.createElement('a');
      a.href = url;
      a.download = `exec_${window._currentModalDetail.execution.id}_logs.json`;
      a.click();
      URL.revokeObjectURL(url);
      toast('Logs exportados com sucesso.');
    }
    return;
  }
  const retryBtn = e.target.closest('[data-retry-exec]');
  if (retryBtn) {
    const id = retryBtn.dataset.retryExec;
    run(retryBtn, async () => {
      const res = await API.retryExecution(id);
      closeModal($('#exec-detail-modal'));
      await Promise.all([loadAndRenderExecutionsHistory(), loadAndRenderExecutionsSummary()]);
      toast(res.new_execution_id ? `Reexecução disparada (#${res.new_execution_id})` : (res.message || 'Reexecução disparada.'));
    });
    return;
  }
  const cancelBtn = e.target.closest('[data-cancel-exec]');
  if (cancelBtn) {
    const id = cancelBtn.dataset.cancelExec;
    run(cancelBtn, async () => {
      await API.cancelExecution(id);
      await Promise.all([loadAndRenderExecutionsHistory(), loadAndRenderExecutionsSummary()]);
      toast(`Cancelamento solicitado para execução #${id}.`);
    });
    return;
  }
  const openExecBtn = e.target.closest('[data-open-exec]');
  if (openExecBtn) {
    openExecutionModal(openExecBtn.dataset.openExec);
    return;
  }
  if (e.target.id === 'modal-exec-close' || e.target.closest('#modal-exec-close')) {
    closeModal($('#exec-detail-modal'));
    return;
  }
  if (e.target.id === 'btn-cancel-act') {
    if (S.activeExec && S.activeExec.id) {
      run($('#btn-cancel-act'), async () => {
        await API.cancelExecution(S.activeExec.id);
        toast('Solicitação de cancelamento enviada.');
      });
    }
    return;
  }
  if (e.target.id === 'btn-refresh-execs') {
    run($('#btn-refresh-execs'), async () => {
      await Promise.all([loadAndRenderExecutionsHistory(), loadAndRenderAiTelemetry()]);
      toast('Histórico de execuções e telemetria IA atualizados.');
    });
    return;
  }
  const toggleColBtn = e.target.closest('[data-toggle-collector]');
  if (toggleColBtn) {
    const srcId = toggleColBtn.dataset.toggleCollector;
    const nextEnabled = toggleColBtn.dataset.enabled !== '1';
    run(toggleColBtn, async () => {
      await API.toggleCollector(srcId, nextEnabled);
      await load();
    }, `Coletor ${srcId} ${nextEnabled ? 'ativado' : 'desativado'}.`);
    return;
  }
  const compBtn = e.target.closest('[data-comp-id]');
  if (compBtn) {
    S.selectedCompanyId = Number(compBtn.dataset.compId);
    renderEmpresas();
    return;
  }
  const scanCompBtn = e.target.closest('[data-scan-company]');
  if (scanCompBtn) {
    const compId = Number(scanCompBtn.dataset.scanCompany);
    run(scanCompBtn, async () => {
      toast('Consultando portal ATS da empresa…');
      const rep = await API.scanCompany(compId, { limit: 30 });
      await load();
      openCompanyDetail(compId);
      toast(`Varredura de ${rep.company_name}: ${rep.newly_inserted} novas vagas, ${rep.duplicates_updated} atualizadas.`);
    });
    return;
  }
  if (e.target.id === 'btn-rescreen-all') {
    run($('#btn-rescreen-all'), async () => {
      const res = await API.screenAllJobs();
      await load();
      toast(`Triagem determinística recalculada para ${res.screened_count || 0} vagas.`);
    });
    return;
  }
});

$('#f-new-company')?.addEventListener('submit', (e) => {
  e.preventDefault();
  const f = new FormData(e.target);
  run(e.submitter, async () => {
    const res = await API.saveCompany({
      official_name: String(f.get('official_name') || '').trim(),
      segment: String(f.get('segment') || '').trim(),
      location: String(f.get('location') || '').trim(),
      website: String(f.get('website') || '').trim(),
      ats_provider: String(f.get('ats_provider') || '').trim(),
      portal_id: String(f.get('portal_id') || '').trim(),
      career_url: String(f.get('career_url') || '').trim(),
      is_monitored: Boolean(f.get('is_monitored')),
      is_preferred: Boolean(f.get('is_preferred')),
      is_excluded: Boolean(f.get('is_excluded')),
    });
    S.selectedCompanyId = res.id;
    e.target.reset();
    await load();
    renderEmpresas();
  }, 'Empresa salva e vinculada com sucesso.');
});

$('#chk-only-monitored')?.addEventListener('change', () => {
  renderEmpresas();
});

addEventListener('hashchange', render);
initResumePdfUpload();
initExecutionsSSE();
load().catch((e) => {
  toast('Não foi possível carregar as vagas: ' + e.message, true);
  render();
});
