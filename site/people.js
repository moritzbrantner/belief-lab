import init, { compare_attributions_json } from './pkg/belief_cli.js';

const $ = (id) => document.getElementById(id);
const examplePath = 'examples/person-attributions/';
let comparison = null;
let initialized = false;
function element(tag, value) {
  const item = document.createElement(tag);
  if (value !== undefined) item.textContent = String(value);
  return item;
}
function status(value, failed = false) {
  $('people-status').textContent = value;
  $('people-status').dataset.error = String(failed);
}
function clearComparison() {
  comparison = null;
  $('comparison').replaceChildren();
  $('proposition').replaceChildren();
  $('comparison-json').textContent = '';
  $('download-comparison').disabled = true;
}
async function readFile(file) {
  if (file.size > 1048576) throw Error('Input must be at most 1 MiB.');
  return file.text();
}
async function getExample(filename) {
  const response = await fetch(examplePath + filename, { signal: AbortSignal.timeout(15000) });
  if (!response.ok) throw Error('Could not load example: ' + response.status);
  return readFile(await response.blob());
}
function updateEvidenceOptions() {
  const selected = $('source-evidence').value;
  $('source-evidence').replaceChildren();
  try {
    const batch = JSON.parse($('evidence-input').value);
    for (const evidence of batch.evidence || []) {
      const option = element('option', evidence.id + ' · ' + (evidence.subject || 'no person') + ' · ' + evidence.source.recordId);
      option.value = evidence.id;
      $('source-evidence').append(option);
    }
    if ([...$('source-evidence').options].some((entry) => entry.value === selected)) {
      $('source-evidence').value = selected;
    }
    updatePersonName();
  } catch { $('person-name').value = ''; }
}
function updatePersonName() {
  try {
    const batch = JSON.parse($('evidence-input').value);
    const document = JSON.parse($('attribution-input').value);
    const selected = batch.evidence.find((entry) => entry.id === $('source-evidence').value);
    const person = document.people.find((entry) =>
      entry.subjectId === selected.subject &&
      entry.scopeId === selected.source.scopeId &&
      entry.sourceRepository === selected.source.repository);
    $('person-name').value = person?.displayName || '';
  } catch { $('person-name').value = ''; }
}
async function loadExample() {
  clearComparison();
  try {
    const [batch, attributions] = await Promise.all([getExample('evidence.json'), getExample('attributions.json')]);
    $('evidence-input').value = batch;
    $('attribution-input').value = attributions;
    updateEvidenceOptions();
    runComparison();
  } catch (error) { status(error.message, true); }
}
function numeric(id) {
  const value = $(id).value.trim();
  if (value === '') return null;
  const result = Number(value);
  if (!Number.isFinite(result) || result < 0 || result > 1) throw Error(id + ' must be between 0 and 1.');
  return result;
}
function recordAttribution() {
  try {
    const batch = JSON.parse($('evidence-input').value);
    const annotation = JSON.parse($('attribution-input').value);
    if (!Array.isArray(annotation.people) || !Array.isArray(annotation.observations) || !Array.isArray(annotation.assessments)) throw Error('Open a valid person-attribution document.');
    const evidence = batch.evidence.find((record) => record.id === $('source-evidence').value);
    if (!evidence?.subject || !evidence.source?.scopeId) throw Error('Selected evidence must identify a source-local person.');
    const name = $('person-name').value.trim();
    const passage = $('passage').value.trim();
    const start = Number($('offset-start').value);
    const end = Number($('offset-end').value);
    if (!name || !passage || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end <= start) {
      throw Error('Provide a name, an exact passage, and valid source offsets.');
    }
    if (passage.length > 4096) throw Error('Passage exceeds 4096 bytes.');
    const kind = $('kind').value;
    const stance = $('stance').value;
    const obs = numeric('observation-score');
    const personConfidence = numeric('person-score');
    const certainty = numeric('conviction-score');
    const basis = $('conviction-basis').value;
    if ((certainty === null) !== (basis === '')) throw Error('Select a conviction basis and value together, or leave both unknown.');
    if (kind === 'quotation_or_criticism' && (stance !== 'undetermined' || obs !== null || certainty !== null)) {
      throw Error('A quotation/criticism is not an endorsement. Set stance to undetermined and leave scores unknown.');
    }
    if (obs === null && (certainty !== null || personConfidence !== null)) {
      throw Error('A person-confidence or conviction score needs a scored observation.');
    }
    let existing = annotation.people.find((person) =>
      person.subjectId === evidence.subject &&
      person.scopeId === evidence.source.scopeId &&
      person.sourceRepository === evidence.source.repository);
    if (!existing) {
      let personId = evidence.subject;
      let suffix = 1;
      while (annotation.people.some((person) => person.id === personId)) {
        personId = evidence.subject + '-source-' + suffix++;
      }
      existing = {
        id: personId, subjectId: evidence.subject, scopeId: evidence.source.scopeId,
        sourceRepository: evidence.source.repository, displayName: name
      };
      annotation.people.push(existing);
    } else existing.displayName = name;
    let next = 1;
    while (annotation.observations.some((item) => item.id === 'manual-' + next)) next++;
    const id = 'manual-' + next;
    const observation = {
      id, personId: existing.id, evidenceId: evidence.id, kind, stance, passage,
      span: { start, end }
    };
    if (obs !== null) observation.confidenceObservation = { value: obs, assessedBy: 'analyst' };
    if (certainty !== null) observation.conviction = { value: certainty, basis, assessedBy: 'analyst' };
    annotation.observations.push(observation);
    // Any previous person-confidence assessment would omit this new evidence; make it unknown.
    annotation.assessments = annotation.assessments.filter((item) => item.personId !== existing.id);
    if (personConfidence !== null) {
      annotation.assessments.push({
        personId: existing.id, stance,
        confidencePerson: { value: personConfidence, assessedBy: 'analyst' },
        observationIds: [id],
        reviewedConflicts: $('review-conflicts').checked
      });
    }
    $('attribution-input').value = JSON.stringify(annotation, null, 2);
    runComparison();
  } catch (error) { status(error.message, true); }
}
function score(value) {
  return value ? value.value.toLocaleString(undefined, { maximumFractionDigits: 3 }) + ' · ' + (value.assessed_by === 'scripted_example' ? 'Scripted example' : 'Analyst assessment') : 'Unknown — insufficient evidence';
}
function metric(label, value, description) {
  const div = element('div');
  div.className = 'score-metric';
  div.append(element('strong', label), element('p', value), element('small', description));
  return div;
}
function appendDetail(list, name, value) {
  list.append(element('dt', name), element('dd', value ?? 'Unknown'));
}
function render(report) {
  clearComparison();
  comparison = report;
  $('download-comparison').disabled = false;
  $('comparison-json').textContent = JSON.stringify(report, null, 2);
  const proposition = element('article');
  proposition.append(element('h3', report.proposition.text), element('p', 'Scope: ' + report.proposition.scope));
  proposition.append(element('small', 'One scoped proposition. Comparing people does not decide if it is true.'));
  $('proposition').append(proposition);
  for (const person of report.people) {
    const card = element('article'); card.className = 'person-card';
    card.append(element('h3', person.display_name), element('p', 'Attributed position: ' + person.stance));
    card.append(element('small', 'Source-local person ' + person.subject_id + ' · ' + person.source_repository + ' / ' + person.scope_id));
    card.append(metric('Confidence (person)', score(person.confidence_person), 'How sure we are this person holds this position across cited observations. Not the truth of the proposition.'));
    if (person.has_conflicting_positions) card.append(element('p', 'Conflicting positions in the evidence — inspect each observation.'));
    if (!person.observations.length) card.append(element('p', 'No admitted observations. Attribution unknown.'));
    for (const observation of person.observations) {
      const detail = element('details'); detail.open = true;
      detail.append(element('summary', observation.kind.replaceAll('_', ' ') + ' · ' + observation.stance));
      const quote = element('blockquote', observation.passage); detail.append(quote);
      detail.append(metric('Confidence (observation)', score(observation.confidence_observation), 'How strongly this passage implies this person holds the position.'));
      const conviction = observation.conviction
        ? score({ value: observation.conviction.value, assessed_by: observation.conviction.assessed_by }) + ' · ' + observation.conviction.basis.replaceAll('_', ' ')
        : 'Unknown — insufficient evidence';
      detail.append(metric('Conviction', conviction, 'The person’s own certainty about the proposition; self-reported and inferred estimates are distinct.'));
      const evidence = observation.evidence;
      const provenance = element('dl'); provenance.className = 'provenance-list';
      appendDetail(provenance, 'Evidence ID / family', evidence.id + ' / ' + evidence.family_id);
      appendDetail(provenance, 'Source', evidence.source_repository + ' · ' + evidence.source_scope_id + ' · ' + evidence.source_record_id);
      appendDetail(provenance, 'Source revision / offsets', evidence.source_revision + ' · ' + observation.span.start + '–' + observation.span.end);
      appendDetail(provenance, 'Producer revision', evidence.producer_name + ' · ' + evidence.producer_revision);
      appendDetail(provenance, 'Producer model', evidence.producer_model);
      appendDetail(provenance, 'Producer configuration', evidence.producer_config_hash);
      appendDetail(provenance, 'Authorization', evidence.policy_profile + ' · ' + evidence.authorized_purpose);
      detail.append(provenance);
      card.append(detail);
    }
    $('comparison').append(card);
  }
}
function runComparison() {
  if (!initialized) { status('Rust pipeline is not ready.', true); return; }
  try {
    const report = JSON.parse(compare_attributions_json(
      $('evidence-input').value,
      $('attribution-input').value,
      JSON.stringify({ BELIEF_POLICY_PROFILE: 'semantic_research' })
    ));
    render(report);
    status('Comparison ready.');
  } catch (error) { clearComparison(); status(String(error.message || error), true); }
}
$('theme').value = localStorage.getItem('belief-theme') || 'system';
function theme() { document.documentElement.dataset.theme = $('theme').value; localStorage.setItem('belief-theme', $('theme').value); }
$('theme').addEventListener('change', theme);
theme();
$('source-evidence').addEventListener('change', updatePersonName);
$('evidence-input').addEventListener('input', () => { clearComparison(); updateEvidenceOptions(); });
$('attribution-input').addEventListener('input', () => { clearComparison(); updatePersonName(); });
for (const [id, field] of [['evidence-file', 'evidence-input'], ['attribution-file', 'attribution-input']]) {
  $(id).addEventListener('change', async (event) => {
    try {
      const file = event.target.files[0];
      if (file) {
        $(field).value = await readFile(file);
        clearComparison(); updateEvidenceOptions();
        status('Inputs loaded. Review and compare.');
      }
    } catch (error) { status(error.message, true); }
  });
}
$('load-example').addEventListener('click', loadExample);
$('compare').addEventListener('click', runComparison);
$('record').addEventListener('click', recordAttribution);
$('download-annotations').addEventListener('click', () => {
  try {
    const annotations = JSON.parse($('attribution-input').value);
    if (annotations.schema !== 'belief_person_attributions' || annotations.schemaVersion !== 1) {
      throw Error('Open a valid person-attribution document before downloading.');
    }
    const url = URL.createObjectURL(new Blob([JSON.stringify(annotations, null, 2) + '\n'], { type: 'application/json' }));
    const anchor = element('a'); anchor.href = url; anchor.download = 'person-attributions.json'; anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (error) { status(error.message, true); }
});
$('download-comparison').addEventListener('click', () => {
  if (!comparison) return;
  const url = URL.createObjectURL(new Blob([JSON.stringify(comparison, null, 2)], { type: 'application/json' }));
  const anchor = element('a'); anchor.href = url; anchor.download = 'person-belief-comparison.json'; anchor.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
});
try {
  await init();
  initialized = true;
  $('compare').disabled = false;
  $('record').disabled = false;
  $('download-annotations').disabled = false;
  await loadExample();
} catch (error) { status('Could not start the Rust attribution pipeline: ' + error.message, true); }
