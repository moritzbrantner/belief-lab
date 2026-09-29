import init, { explain_json } from './pkg/belief_cli.js';
import { translations } from './translations.js';
const $ = (id) => document.getElementById(id);
let language = localStorage.getItem('belief-language') || 'en';
if (!translations[language]) language = 'en';
let report = null;
let loading = false;
const t = (key) => translations[language][key] || translations.en[key] || key;
function localize() {
  document.documentElement.lang = language;
  document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
  $('language').value = language;
  $('evidence').setAttribute('aria-label',t('evidence'));
  $('judgments').setAttribute('aria-label',t('judgments'));
  if (report) render(report);
}
function status(message, error = false) { $('status').textContent = message; $('status').dataset.error = String(error); }
function clearReport() {
  report = null; $('summary').replaceChildren(); $('results').replaceChildren(); $('report').textContent = '';
  $('download-report').disabled = true;
}
async function textFile(file) {
  if (file.size > 1048576) throw new Error(t('tooLarge'));
  return file.text();
}
async function fetchText(url) {
  const response = await fetch(url, { signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw new Error(`${t('loadFailed')}: ${response.status}`);
  return textFile(await response.blob());
}
async function loadExample() {
  if (loading) return;
  loading = true; $('load').disabled = true; $('analyze').disabled = true; clearReport();
  try {
    const name = $('example').value;
    const base = `examples/explain/${name}/`;
    const [evidence, judgments] = await Promise.all([fetchText(`${base}evidence.json`), fetchText(`${base}judgments.json`)]);
    $('evidence').value = evidence; $('judgments').value = judgments;
    $('download-evidence').href = `${base}evidence.json`; $('download-judgments').href = `${base}judgments.json`;
    $('evidence-file').value = ''; $('judgments-file').value = '';
    const url = new URL(location.href); url.searchParams.set('example',name); history.replaceState(null,'',url);
    status(t('ready'));
  } catch (error) { status(error.message,true); }
  finally { loading = false; $('load').disabled = false; $('analyze').disabled = false; }
}
function node(tag, text) { const el = document.createElement(tag); if (text !== undefined) el.textContent = text; return el; }
function table(headings, rows) {
  const wrapper = node('div'); wrapper.className = 'table-scroll'; const table = node('table');
  const header = node('tr'); headings.forEach((h) => header.append(node('th',h))); const head = node('thead'); head.append(header); table.append(head);
  const body = node('tbody'); rows.forEach((row) => { const tr = node('tr'); row.forEach((v) => tr.append(node('td',String(v)))); body.append(tr); });
  table.append(body); wrapper.append(table); return wrapper;
}
function render(value) {
  $('summary').replaceChildren(); $('results').replaceChildren(); $('report').textContent = JSON.stringify(value,null,2);
  const evidence = value.batch?.evidence || [];
  [[evidence.length,t('records')],[evidence.filter((e) => e.admission.admitted).length,t('admitted')],[value.inferences.filter((i) => i.belief).length,t('beliefs')]].forEach(([n,label]) => {
    const metric = node('div'); metric.className = 'metric'; metric.append(node('strong',String(n)),node('span',label)); $('summary').append(metric);
  });
  if (value.rejection) $('results').append(node('p',`${t('rejected')}: ${value.rejection.reason}`));
  if (value.batch?.import.reason) $('results').append(node('p',value.batch.import.reason));
  if (evidence.length) $('results').append(table([t('evidence'),t('class'),t('admission')],evidence.map((e) => [e.id,e.class,e.admission.admitted ? t('admitted') : e.admission.rejection_reasons.join('; ')])));
  value.inferences.forEach((inference) => {
    const article = node('article'); const p = inference.proposition;
    article.append(node('h3',`${p.subject} · ${p.predicate} · ${p.object}`));
    article.append(node('p',inference.belief ? `${inference.belief.value.toLocaleString(language,{maximumFractionDigits:4})} ${inference.belief.semantics}` : `${inference.status}: ${inference.reason || ''}`));
    if (inference.judgments.length) article.append(table([t('judgment'),t('outcome'),t('score'),t('selection')],inference.judgments.map((j) => [j.id,j.outcome,`${j.confidence.value} ${j.confidence.semantics}`,j.selection || '—'])));
    if (inference.provenance) { const detail = node('details'); detail.append(node('summary',t('provenance')),node('pre',JSON.stringify(inference.provenance,null,2))); article.append(detail); }
    $('results').append(article);
  });
}
$('language').addEventListener('change',() => { language = $('language').value; localStorage.setItem('belief-language',language); localize(); status(t('ready')); });
$('theme').value = localStorage.getItem('belief-theme') || 'system';
function theme() { document.documentElement.dataset.theme = $('theme').value; localStorage.setItem('belief-theme',$('theme').value); }
$('theme').addEventListener('change',theme); theme(); localize();
for (const id of ['evidence','judgments']) {
  $(id).addEventListener('input',clearReport);
  $(`${id}-file`).addEventListener('change',async (event) => {
    clearReport(); try { const file = event.target.files[0]; if (file) { $(id).value = await textFile(file); status(t('ready')); } } catch(error) { status(error.message,true); }
  });
}
$('profile').addEventListener('change',clearReport); $('settings').addEventListener('input',clearReport);
$('load').addEventListener('click',loadExample);
$('analyze').addEventListener('click',() => {
  clearReport();
  try {
    const policy = JSON.parse($('settings').value);
    if (!policy || Array.isArray(policy) || typeof policy !== 'object') throw new Error(t('invalidSettings'));
    if ('BELIEF_POLICY_PROFILE' in policy) throw new Error(t('profileSetting'));
    policy.BELIEF_POLICY_PROFILE = $('profile').value;
    report = JSON.parse(explain_json($('evidence').value,$('judgments').value,JSON.stringify(policy)));
    render(report); $('download-report').disabled = false;
    status(report.outcome === 'input_rejected' ? t('rejected') : t('complete'),report.outcome === 'input_rejected');
  } catch(error) { status(String(error.message || error),true); }
});
$('download-report').addEventListener('click',() => {
  if (!report) return;
  const url = URL.createObjectURL(new Blob([JSON.stringify(report,null,2)],{type:'application/json'}));
  const link = node('a'); link.href = url; link.download = 'belief-report.json'; link.click(); setTimeout(() => URL.revokeObjectURL(url),1000);
});
$('result-file').addEventListener('change',async (event) => {
  $('model-result').textContent = '';
  try {
    const file = event.target.files[0]; if (!file) return;
    const result = JSON.parse(await textFile(file));
    if (result.schema !== 'belief_semantic_result' || result.schemaVersion !== 1) throw new Error(t('invalidResult'));
    $('model-result').textContent = JSON.stringify(result,null,2);
  } catch(error) { $('model-result').textContent = error.message; }
});
try {
  await init();
  const example = new URL(location.href).searchParams.get('example');
  if ([...$('example').options].some((o) => o.value === example)) $('example').value = example;
  await loadExample();
} catch(error) { status(`${t('startupFailed')}: ${error.message}`,true); }
