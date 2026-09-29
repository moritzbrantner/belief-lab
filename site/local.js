import { translations } from './local-translations.js';
const $ = (id) => document.getElementById(id);
// The token is delivered by the CLI in a fragment, never in an HTTP URL or shared storage.
const token = new URLSearchParams(location.hash.slice(1)).get('token');
document.querySelector('.brand').href = location.href;
let language = localStorage.getItem('belief-language') || 'en';
if (!translations[language]) language = 'en';
let result = null;
let statusKey = 'loading';
let statusDetail = '';
const t = (key) => translations[language][key] || translations.en[key];
function status(key, detail = '') {
  statusKey = key; statusDetail = detail;
  $('status').textContent = `${t(key)}${detail ? `: ${detail}` : ''}`;
  $('status').dataset.error = String(key === 'failed' || key === 'disconnected');
}
function localize() {
  document.documentElement.lang = language;
  document.querySelectorAll('[data-i18n]').forEach((el) => { el.textContent = t(el.dataset.i18n); });
  $('request').setAttribute('aria-label', t('requestJson'));
  $('language').value = language;
  status(statusKey, statusDetail);
  if (result) render(result);
}
function node(tag, text) {
  const el = document.createElement(tag);
  if (text !== undefined) el.textContent = text;
  return el;
}
function table(headings, rows) {
  const wrap = node('div'); wrap.className = 'table-scroll';
  const table = node('table'); const header = node('tr'); const head = node('thead'); const body = node('tbody');
  for (const label of headings) header.append(node('th', label));
  head.append(header); table.append(head);
  for (const values of rows) {
    const row = node('tr');
    for (const value of values) row.append(node('td', String(value)));
    body.append(row);
  }
  table.append(body); wrap.append(table); return wrap;
}
function clear() {
  result = null; $('result').replaceChildren(); $('json').textContent = ''; $('download').disabled = true;
}
function render(value) {
  $('result').replaceChildren(); $('json').textContent = JSON.stringify(value, null, 2);
  if (value.status === 'error') { $('result').append(node('p', `${value.error.code}: ${value.error.message}`)); return; }
  const decision = value.decision;
  if (decision.provider === 'fixture') $('result').append(node('p', t('simulated')));
  $('result').append(node('h3', `${t('judgment')}: ${value.judgment.outcome}`));
  $('result').append(node('p', `${decision.model} @ ${decision.modelRevision}`));
  $('result').append(table([t('option'), t('probability')], Object.entries(decision.scores).map(([id, score]) => [id, score.toLocaleString(language, { maximumFractionDigits: 6 })])));
  $('result').append(node('h3', value.belief ? `${t('belief')}: ${value.belief.value.toLocaleString(language, { maximumFractionDigits: 6 })} ${value.belief.semantics}` : t('unknown')));
  $('result').append(table([t('source'), t('revision'), t('producer')], value.request.evidence.evidence.map((e) => [`${e.source.repository}/${e.source.recordId}`, e.source.revision, `${e.producer.name} @ ${e.producer.revision}`])));
}
function parseRequest() {
  const text = $('request').value;
  if (new TextEncoder().encode(text).length > 1048576) throw new Error(t('tooLarge'));
  const value = JSON.parse(text);
  if (value.schema !== 'belief_semantic_request' || value.schemaVersion !== 1) throw new Error(t('invalid'));
  return value;
}
function syncModel(value) {
  const choice = value.provider?.kind === 'fixture' ? 'fixture' : value.provider?.kind === 'semif' ? value.provider.tier : '';
  $('model').value = choice;
}
async function loadExample() {
  clear(); status('loading'); $('inputs').disabled = true; $('run').disabled = true;
  try {
    const response = await fetch('example.json', { signal: AbortSignal.timeout(15000) });
    if (!response.ok) throw new Error(String(response.status));
    $('request').value = JSON.stringify(await response.json(), null, 2);
    syncModel(parseRequest()); status(token ? 'ready' : 'disconnected');
  } catch (error) { status('failed', error.message); }
  finally { $('inputs').disabled = false; $('run').disabled = !token; }
}
$('language').addEventListener('change', () => { language = $('language').value; localStorage.setItem('belief-language', language); localize(); });
$('theme').value = localStorage.getItem('belief-theme') || 'system';
function theme() { document.documentElement.dataset.theme = $('theme').value; localStorage.setItem('belief-theme', $('theme').value); }
$('theme').addEventListener('change', theme); theme(); localize();
$('load').addEventListener('click', loadExample);
$('request').addEventListener('input', () => {
  clear();
  try { syncModel(parseRequest()); status(token ? 'ready' : 'disconnected'); }
  catch (error) { $('model').value = ''; status('failed', error.message); }
});
$('model').addEventListener('change', () => {
  clear();
  try {
    const input = parseRequest();
    input.provider = $('model').value === 'fixture' ? { kind: 'fixture', scores: { supports: 0.9, contradicts: 0.05, unknown: 0.05 } } : { kind: 'semif', tier: $('model').value };
    $('request').value = JSON.stringify(input, null, 2); status(token ? 'ready' : 'disconnected');
  } catch (error) { status('failed', error.message); }
});
$('file').addEventListener('change', async (event) => {
  clear(); const file = event.target.files[0]; if (!file) return;
  try {
    if (file.size > 1048576) throw new Error(t('tooLarge'));
    $('request').value = await file.text(); syncModel(parseRequest()); status(token ? 'ready' : 'disconnected');
  } catch (error) { status('failed', error.message); }
});
$('run').addEventListener('click', async () => {
  clear(); $('inputs').disabled = true; $('run').disabled = true; status('running');
  try {
    parseRequest(); // Validation aid only; Rust independently validates the original text.
    const response = await fetch('api/decide', { method: 'POST', headers: { 'Content-Type': 'application/json', 'X-Belief-Token': token }, body: $('request').value });
    result = await response.json(); render(result); $('download').disabled = false;
    status(result.status === 'ok' ? 'complete' : 'failed', result.error?.message || '');
  } catch (error) { status('failed', error.message); }
  finally { $('inputs').disabled = false; $('run').disabled = !token; }
});
$('download').addEventListener('click', () => {
  if (!result) return;
  const url = URL.createObjectURL(new Blob([JSON.stringify(result, null, 2)], { type: 'application/json' }));
  const link = node('a'); link.href = url; link.download = 'belief-result.json'; link.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});
await loadExample();
