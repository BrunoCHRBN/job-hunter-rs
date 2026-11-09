'use strict';
/* Job Hunter RS — UI v2.
   >>> Se algo não carregar, ajuste APENAS o bloco API e a função norm() abaixo. <<< */

const STAGES = [['descoberta','Descoberta'],['salva','Salva'],['aplicada','Aplicada'],['entrevista','Entrevista'],['descartada','Descartada']];
const FUNIL = ['salva','aplicada','entrevista'];

async function req(method, url, body) {
  const r = await fetch(url, { method, headers: body ? { 'Content-Type': 'application/json' } : {}, body: body ? JSON.stringify(body) : undefined });
  const t = await r.text();
  if (!r.ok) throw new Error(t || 'Erro ' + r.status);
  try { return JSON.parse(t); } catch { return t; }
}
const API = { // endpoints assumidos: confira com src/web.rs
  jobs:       ()         => req('GET',  '/api/jobs'),
  setStatus:  (id, st)   => req('POST', `/api/jobs/${id}/status`, { status: st }),
  match:      (id)       => req('POST', `/api/jobs/${id}/match`),
  collect:    (b)        => req('POST', '/api/collect', b),
  enrich:     ()         => req('POST', '/api/enrich'),
  resume:     ()         => req('GET',  '/api/resume'),
  saveResume: (content)  => req('PUT',  '/api/resume', { content }),
};

const nk = s => String(s ?? '').normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLowerCase();
const esc = s => String(s ?? '').replace(/[&<>"']/g, c => ({ '&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;',"'":'&#39;' }[c]));
const parse = v => { try { return JSON.parse(v); } catch { return null; } };
const arr = v => Array.isArray(v) ? v : (typeof v === 'string' && v) ? (Array.isArray(parse(v)) ? parse(v) : v.split(',').map(s => s.trim()).filter(Boolean)) : [];
const stage = s => { const k = nk(s); return (STAGES.find(x => k.startsWith(x[0].slice(0, 5))) || STAGES[0])[0]; };

function parseMatchMd(md) {
  if (!md || typeof md !== 'string') return {};
  const j = parse(md);
  if (j && typeof j === 'object') return j;
  const sec = (title) => {
    const m = md.match(new RegExp(`##\\s+${title}\\n([\\s\\S]*?)(?=\\n##\\s+|$)`));
    return m ? m[1].trim() : '';
  };
  const items = (txt) => txt.split('\n').map(l => l.replace(/^[-•*]\s*/, '').trim()).filter(l => l && !/^Nenhuma|^Nenhum|^Sem /i.test(l));
  return {
    shift_and_location_verdict: sec('Veredito de Escala e Localização'),
    matching_skills: items(sec('Skills Compatíveis')),
    missing_skills: items(sec('Gaps / Skills Ausentes')),
    ats_suggestions: items(sec('Sugestões de Otimização ATS')),
    detailed_analysis_markdown: sec('Análise Detalhada') || md,
  };
}

function norm(j) { // adapta o JSON do backend ao formato usado pela UI
  let a = j.analysis || j.match_analysis || parseMatchMd(j.match_analysis_md) || {};
  if (typeof a === 'string') a = parse(a) || parseMatchMd(a) || {};
  const pick = k => j[k] ?? a[k];
  return {
    id: j.id, title: j.title || 'Sem título', company: j.company || '—', location: j.location || '', source: j.source || '',
    url: /^https?:/i.test(j.url || j.job_url || '') ? (j.url || j.job_url) : '',
    modo: j.work_model || '', turno: j.shift_schedule || '', contrato: j.contract_type || '', nivel: j.seniority || '',
    salario: j.salary_estimate_brl || '', resumo: j.summary || j.ai_summary || '',
    skills: arr(j.required_skills ?? j.required_skills_json),
    flags: arr(j.red_flags ?? j.red_flags_json),
    desc: j.description || j.raw_description || '',
    etapa: stage(j.status || j.application_status),
    score: j.match_score != null ? Math.round(j.match_score) : null,
    enriched: Boolean(j.is_enriched ?? j.enriched ?? j.summary ?? j.ai_summary),
    an: { ok: arr(pick('matching_skills')), falta: arr(pick('missing_skills')), ats: arr(pick('ats_suggestions')),
          veredito: pick('shift_and_location_verdict') || '', md: pick('detailed_analysis_markdown') || '' },
  };
}

/* ---------- estado e utilitários ---------- */
const $ = (s, r = document) => r.querySelector(s);
const S = { jobs: [], sel: null, tab: 'geral', f: { q: '', modo: '', turno: '', etapa: '' }, cv: null };
const cls = s => s >= 70 ? 'hi' : s >= 40 ? 'mid' : 'lo';
const chip = (t, c = '') => t ? `<span class="chip ${c}">${esc(t)}</span>` : '';
const ul = a => a.length ? `<ul>${a.map(x => `<li>${esc(x)}</li>`).join('')}</ul>` : '';
const byScore = (a, b) => (b.score ?? -1) - (a.score ?? -1) || b.id - a.id;
const stageLabel = k => STAGES.find(s => s[0] === k)[1];

let tt;
function toast(msg, err) {
  const t = $('#toast'); t.textContent = msg; t.className = err ? 'err' : ''; t.hidden = false;
  clearTimeout(tt); tt = setTimeout(() => t.hidden = true, 4000);
}
async function run(btn, fn, ok) {
  if (btn) btn.disabled = true;
  try { await fn(); if (ok) toast(ok); } catch (e) { toast(String(e.message || e).slice(0, 160), true); }
  finally { if (btn) btn.disabled = false; }
}
async function load() {
  const d = await API.jobs();
  S.jobs = (Array.isArray(d) ? d : d.jobs || []).map(norm);
  render();
}

/* ---------- tela: vagas ---------- */
function filtered() {
  const { q, modo, turno, etapa } = S.f, k = nk(q);
  return S.jobs.filter(j =>
    (!k || nk([j.title, j.company, j.location, j.skills.join(' ')].join(' ')).includes(k)) &&
    (!modo || nk(j.modo).startsWith(modo)) && (!turno || j.turno === turno) && (!etapa || j.etapa === etapa)
  ).sort(byScore);
}
function row(j) {
  return `<li><button class="job${j.id === S.sel ? ' on' : ''}" data-id="${j.id}">
    <span class="logo" aria-hidden="true">${esc(j.company[0] || '?')}</span>
    <span class="job-t"><strong>${esc(j.title)}</strong><span>${esc(j.company)}</span><span class="mut">${esc(j.location)}</span>
    <span class="chips">${chip(j.modo)}${chip(j.turno)}</span></span>
    ${j.score != null ? `<span class="score ${cls(j.score)}">${j.score}%</span>` : ''}</button></li>`;
}
function tabBody(j) {
  if (S.tab === 'desc') return `<p class="raw">${esc(j.desc) || '<span class="mut">Sem descrição disponível.</span>'}</p>`;
  if (S.tab === 'match') {
    const a = j.an;
    if (j.score == null && !a.md && !a.ok.length) return '<p class="empty">Calcule o match para comparar esta vaga com o seu currículo.</p>';
    return `${a.veredito ? `<p>${esc(a.veredito)}</p>` : ''}
      <div class="two"><div><h3>Você já tem</h3><div class="chips">${a.ok.map(x => chip(x, 'ok')).join('') || '<span class="mut">Nada identificado</span>'}</div></div>
      <div><h3>Falta no currículo</h3><div class="chips">${a.falta.map(x => chip(x, 'bad')).join('') || '<span class="mut">Nada identificado</span>'}</div></div></div>
      ${a.ats.length ? `<div><h3>Ajustes sugeridos para o ATS</h3>${ul(a.ats)}</div>` : ''}
      ${a.md ? `<div><h3>Análise completa</h3>${a.md.split(/\n{2,}/).map(p => `<p class="raw" style="margin-top:8px">${esc(p)}</p>`).join('')}</div>` : ''}`;
  }
  return `${j.enriched ? '' : '<p class="mut">Esta vaga ainda não foi enriquecida. Use “Enriquecer pendentes” no Painel.</p>'}
    ${j.resumo ? `<div><h3>Resumo</h3><p>${esc(j.resumo)}</p></div>` : ''}
    ${j.flags.length ? `<div class="alert"><h3>Pontos de atenção</h3>${ul(j.flags)}</div>` : ''}
    ${j.skills.length ? `<div><h3>Competências exigidas</h3><div class="chips">${j.skills.map(x => chip(x)).join('')}</div></div>` : ''}`;
}
function detail(j) {
  if (!j) return '<p class="empty">Selecione uma vaga para ver os detalhes.</p>';
  const tabs = [['geral', 'Visão geral'], ['match', 'Match ATS'], ['desc', 'Descrição']];
  return `<div class="d-head"><span class="logo lg" aria-hidden="true">${esc(j.company[0] || '?')}</span>
    <div class="grow"><h2>${esc(j.title)}</h2><p>${esc(j.company)}</p><p class="mut">${esc([j.location, j.source].filter(Boolean).join(', '))}</p></div>
    ${j.score != null ? `<div class="big ${cls(j.score)}"><b>${j.score}%</b><span>match</span></div>` : ''}</div>
    <div class="chips">${[j.modo, j.turno, j.contrato, j.nivel, j.salario].map(x => chip(x)).join('')}</div>
    <div class="d-act"><button class="btn primary" data-act="match">${j.score != null ? 'Recalcular match' : 'Calcular match'}</button>
    <select data-act="etapa" aria-label="Etapa no funil">${STAGES.map(([k, l]) => `<option value="${k}"${k === j.etapa ? ' selected' : ''}>${l}</option>`).join('')}</select>
    ${j.url ? `<a class="btn" href="${esc(j.url)}" target="_blank" rel="noopener">Abrir vaga</a>` : ''}</div>
    <div class="tabs" role="tablist">${tabs.map(([k, l]) => `<button role="tab" aria-selected="${S.tab === k}" data-tab="${k}">${l}</button>`).join('')}</div>
    <div class="d-body">${tabBody(j)}</div>`;
}
function renderVagas() {
  const list = filtered();
  if (!list.some(j => j.id === S.sel)) S.sel = list[0]?.id ?? null;
  $('#count').textContent = `${list.length} ${list.length === 1 ? 'vaga' : 'vagas'}`;
  $('#list').innerHTML = list.map(row).join('') || '<li class="empty">Nenhuma vaga com esses filtros. Ajuste a busca ou colete novas vagas no Painel.</li>';
  $('#detail').innerHTML = detail(S.jobs.find(j => j.id === S.sel));
  const t = $('#f-turno'), turnos = [...new Set(S.jobs.map(j => j.turno).filter(Boolean))].sort();
  if (t.options.length !== turnos.length + 1) t.innerHTML = '<option value="">Escala</option>' + turnos.map(x => `<option${x === S.f.turno ? ' selected' : ''}>${esc(x)}</option>`).join('');
}

/* ---------- tela: painel ---------- */
function renderPainel() {
  const n = S.jobs.length, enr = S.jobs.filter(j => j.enriched).length;
  $('#stats').innerHTML = [['Vagas coletadas', n], ['Enriquecidas', enr], ['No funil', S.jobs.filter(j => FUNIL.includes(j.etapa)).length], ['Pendentes', n - enr]]
    .map(([l, v]) => `<div><dd>${v}</dd><dt>${l}</dt></div>`).join('');
  const top = S.jobs.filter(j => j.score != null).sort(byScore).slice(0, 5);
  $('#top').innerHTML = top.map(row).join('').replaceAll('data-id=', 'data-open=') || '<li class="empty">Nenhum match calculado ainda. Abra uma vaga e use “Calcular match”.</li>';
  const p = n - enr;
  $('#pend').innerHTML = `<h2>Enriquecimento com IA</h2><p class="mut" style="margin:8px 0 12px">${p ? `${p} ${p === 1 ? 'vaga aguarda' : 'vagas aguardam'} resumo, competências e alertas.` : 'Todas as vagas já foram enriquecidas.'}</p>
    <button class="btn" data-act="enrich"${p ? '' : ' disabled'}>Enriquecer pendentes</button>`;
}

/* ---------- tela: funil ---------- */
function renderFunil() {
  $('#board').innerHTML = STAGES.map(([k, l], i) => {
    const js = S.jobs.filter(j => j.etapa === k).sort(byScore);
    return `<div class="col"><div class="col-h">${l}<span>${js.length}</span></div>${js.map(j => `<div class="k">
      <button class="k-b" data-open="${j.id}">${esc(j.title)}</button><span class="mut">${esc(j.company)}</span>
      <div class="k-f">${j.score != null ? `<span class="score ${cls(j.score)}">${j.score}%</span>` : '<span></span>'}<span>
      ${i > 0 ? `<button class="btn ghost" data-move="-1" data-for="${j.id}" aria-label="Mover para ${STAGES[i - 1][1]}">Voltar</button>` : ''}
      ${i < 4 ? `<button class="btn" data-move="1" data-for="${j.id}" aria-label="Mover para ${STAGES[i + 1][1]}">Avançar</button>` : ''}</span></div></div>`).join('') || '<p class="empty">Vazio</p>'}</div>`;
  }).join('');
}

/* ---------- tela: currículo ---------- */
async function renderCurriculo() {
  if (S.cv !== null) return;
  try { const d = await API.resume(); S.cv = typeof d === 'string' ? d : d.content ?? d.resume ?? d.markdown ?? ''; $('#cv').value = S.cv; }
  catch (e) { toast('Não foi possível carregar o currículo.', true); }
}

/* ---------- roteamento ---------- */
const VIEWS = ['painel', 'vagas', 'funil', 'curriculo'];
const cur = () => VIEWS.includes(location.hash.slice(1)) ? location.hash.slice(1) : 'painel';
function render() {
  const v = cur();
  VIEWS.forEach(x => { $('#v-' + x).hidden = x !== v; });
  document.querySelectorAll('[data-nav]').forEach(a => a.dataset.nav === v ? a.setAttribute('aria-current', 'page') : a.removeAttribute('aria-current'));
  ({ painel: renderPainel, vagas: renderVagas, funil: renderFunil, curriculo: renderCurriculo })[v]();
}
const go = (v, id) => { if (id != null) S.sel = id; location.hash === '#' + v ? render() : location.hash = v; };

/* ---------- ações ---------- */
async function moveStage(id, to) {
  const j = S.jobs.find(x => x.id === id); if (!j) return;
  const old = j.etapa; j.etapa = to; render();
  try { await API.setStatus(id, stageLabel(to)); } catch (e) { j.etapa = old; render(); throw e; }
}
const calcMatch = btn => run(btn, async () => { await API.match(S.sel); await load(); }, 'Match calculado.');
const nextStage = () => { const j = S.jobs.find(x => x.id === S.sel); if (j) run(null, () => moveStage(j.id, STAGES[(STAGES.findIndex(s => s[0] === j.etapa) + 1) % 5][0])); };

document.addEventListener('click', e => {
  const t = e.target.closest('[data-id],[data-open],[data-tab],[data-act],[data-move]'); if (!t) return;
  const d = t.dataset;
  if (d.id) { S.sel = +d.id; S.tab = 'geral'; renderVagas(); }
  else if (d.open) go('vagas', +d.open);
  else if (d.tab) { S.tab = d.tab; renderVagas(); }
  else if (d.move) { const j = S.jobs.find(x => x.id === +d.for), i = STAGES.findIndex(s => s[0] === j.etapa) + +d.move; run(null, () => moveStage(j.id, STAGES[i][0])); }
  else if (d.act === 'match') calcMatch(t);
  else if (d.act === 'enrich') run(t, async () => { await API.enrich(); await load(); }, 'Enriquecimento concluído.');
});
document.addEventListener('change', e => {
  if (e.target.dataset.act === 'etapa') run(null, () => moveStage(S.sel, e.target.value));
  else if (e.target.dataset.f) { S.f[e.target.dataset.f] = e.target.value; renderVagas(); }
});
document.addEventListener('input', e => { if (e.target.dataset.f === 'q') { S.f.q = e.target.value; renderVagas(); } });

$('#f-etapa').insertAdjacentHTML('beforeend', STAGES.map(([k, l]) => `<option value="${k}">${l}</option>`).join(''));
$('#f-coleta').addEventListener('submit', e => {
  e.preventDefault(); const f = new FormData(e.target);
  run(e.submitter, async () => {
    await API.collect({ query: f.get('query'), location: f.get('location') || '', remote: !!f.get('remote'), enrich: !!f.get('enrich'), limit: 20 });
    await load();
  }, 'Coleta concluída.');
});
$('#save-cv').addEventListener('click', e => run(e.currentTarget, async () => { await API.saveResume($('#cv').value); S.cv = $('#cv').value; }, 'Currículo salvo.'));
$('#theme').addEventListener('click', () => {
  const d = document.documentElement, n = d.dataset.theme === 'dark' ? 'light' : 'dark';
  d.dataset.theme = n; $('#theme').textContent = n === 'dark' ? 'Tema claro' : 'Tema escuro';
  try { localStorage.setItem('jh-theme', n); } catch {}
});
$('#theme').textContent = document.documentElement.dataset.theme === 'dark' ? 'Tema claro' : 'Tema escuro';

document.addEventListener('keydown', e => {
  if (e.key === 'Escape') return document.activeElement.blur();
  const typing = /INPUT|TEXTAREA|SELECT/.test(document.activeElement.tagName);
  if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); go('vagas'); return $('[data-f=q]').focus(); }
  if (typing || e.ctrlKey || e.metaKey || e.altKey) return;
  const v = cur(), ids = filtered().map(j => j.id), i = ids.indexOf(S.sel);
  if (e.key === '/') { e.preventDefault(); go('vagas'); $('[data-f=q]').focus(); }
  else if (e.key === 'r') go('curriculo');
  else if (e.key === 'v') go(v === 'funil' ? 'vagas' : 'funil');
  else if (e.key === 'e') run(null, async () => { await API.enrich(); await load(); }, 'Enriquecimento concluído.');
  else if (v === 'vagas') {
    if (e.key === 'j' || e.key === 'ArrowDown') { S.sel = ids[Math.min(i + 1, ids.length - 1)]; renderVagas(); $('.job.on')?.scrollIntoView({ block: 'nearest' }); }
    else if (e.key === 'k' || e.key === 'ArrowUp') { S.sel = ids[Math.max(i - 1, 0)]; renderVagas(); $('.job.on')?.scrollIntoView({ block: 'nearest' }); }
    else if (e.key === 'm') calcMatch(null);
    else if (e.key === 's') nextStage();
  }
});

addEventListener('hashchange', render);
load().catch(e => { toast('Não foi possível carregar as vagas: ' + e.message, true); render(); });
