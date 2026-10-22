import init, { compare_attributions_json } from './pkg/belief_cli.js';

const $ = (id) => document.getElementById(id);
const examplePath = 'examples/person-attributions/';
let comparison = null;
let initialized = false;
let correctionTarget = null;
let linkCorrectionTarget = null;
let activeProposition = null;
const stanceLabels = {
  endorses: 'Endorses P', endorses_negation: 'Endorses not-P',
  suspends_judgment: 'Explicitly suspends judgment', no_stance: 'No stance in this passage'
};
const statusLabels = {
  assessed: 'Reviewed documented stance',
  unassessed: 'Stance evidence recorded, not yet assessed',
  conflicted: 'Conflicted — opposing stances that may overlap in time; unresolved',
  differs_over_time: 'Opposing stances at different times — not by itself proof of a change of mind',
  no_stance_evidence: 'No evidence of a stance — unknown, not indecision'
};
function stanceLabel(value) { return stanceLabels[value] || value; }
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
  $('relationships').replaceChildren();
  $('comparison').replaceChildren();
  $('history').replaceChildren();
  $('current-revision').textContent = '';
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
    const document = currentAnnotation();
    const selected = batch.evidence.find((entry) => entry.id === $('source-evidence').value);
    const person = document.people.find((entry) =>
      entry.subjectId === selected.subject &&
      entry.scopeId === selected.source.scopeId &&
      entry.sourceRepository === selected.source.repository);
    $('person-name').value = person?.displayName || '';
  } catch { $('person-name').value = ''; }
}
async function loadExample() {
  cancelCorrection();
  clearComparison();
  try {
    const [batch, attributions] = await Promise.all([getExample('evidence.json'), getExample('attributions.json')]);
    $('evidence-input').value = batch;
    $('attribution-input').value = attributions;
    updateEvidenceOptions();
    runComparison();
  } catch (error) { status(String(error.message || error), true); }
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
    const annotation = currentAnnotation();
    if (!Array.isArray(annotation.people) || !Array.isArray(annotation.observations) || !Array.isArray(annotation.assessments)) throw Error('Open a valid person-attribution document.');
    const evidence = batch.evidence.find((record) => record.id === $('source-evidence').value);
    if (!evidence?.subject || !evidence.source?.scopeId) throw Error('Selected evidence must identify a source-local person.');
    const name = $('person-name').value.trim();
    const passage = $('passage').value;
    const start = Number($('offset-start').value);
    const end = Number($('offset-end').value);
    if (!name || !passage.trim() || !Number.isSafeInteger(start) || !Number.isSafeInteger(end) || start < 0 || end <= start) {
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
    if (kind === 'quotation_or_criticism' && (stance !== 'no_stance' || obs !== null || certainty !== null)) {
      throw Error('A quotation/criticism is not an endorsement. Set the stance to “No stance” and leave scores unknown.');
    }
    if (stance === 'suspends_judgment' && (kind === 'inferred_belief' || certainty !== null)) {
      throw Error('Explicit suspension must be expressed or reported, not inferred, and has no conviction about P.');
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
    while (allObservationIds().has('manual-' + next)) next++;
    const id = 'manual-' + next;
    const observation = {
      id, personId: existing.id, evidenceId: evidence.id, kind, stance, passage,
      span: { start, end }
    };
    if (annotation.propositions) observation.propositionId = activeProposition;
    for (const [field, input] of [['expressedAt', 'expressed-at'], ['publishedAt', 'published-at'], ['discoveredAt', 'discovered-at']]) {
      const value = $(input).value.trim();
      if (value) observation[field] = value;
    }
    if (obs !== null) observation.confidenceObservation = { value: obs, assessedBy: 'analyst' };
    if (certainty !== null) observation.conviction = { value: certainty, basis, assessedBy: 'analyst' };
    const supersedes = {};
    if (correctionTarget !== null) {
      const original = annotation.observations.find((item) => item.id === correctionTarget);
      if (!original || original.personId !== existing.id) throw Error('A correction must retain the same source-local person.');
      annotation.observations = annotation.observations.filter((item) => item.id !== correctionTarget);
      supersedes[id] = correctionTarget;
    }
    annotation.observations.push(observation);
    // Any previous person-confidence assessment would omit this new evidence; make it unknown.
    annotation.assessments = annotation.assessments.filter((item) => item.personId !== existing.id || !inActiveProposition(annotation, item));
    if (personConfidence !== null) {
      const assessment = {
        personId: existing.id, stance,
        confidencePerson: { value: personConfidence, assessedBy: 'analyst' },
        observationIds: [id],
        reviewedConflicts: $('review-conflicts').checked
      };
      if (annotation.propositions) assessment.propositionId = activeProposition;
      annotation.assessments.push(assessment);
    }
    commitRevision(annotation, personConfidence === null ? [] : [existing.id], supersedes);
  } catch (error) { status(String(error.message || error), true); }
}
function currentAnnotation(envelope = JSON.parse($('attribution-input').value)) {
  if (envelope.schemaVersion === 1) return envelope;
  if (![2, 3].includes(envelope.schemaVersion) || !envelope.revisions?.length) throw Error('Open valid version-1 annotations or a version-2/3 history.');
  return envelope.revisions[envelope.revisions.length - 1].document;
}
// Items of a version-3 investigation belong to one proposition; single-proposition files have one.
function inActiveProposition(document, item) {
  return !document.propositions || item.propositionId === activeProposition;
}
function toSnapshot(document) {
  const id = document.proposition.id;
  return {
    propositions: [document.proposition], people: document.people,
    observations: document.observations.map((item) => ({ ...item, propositionId: id })),
    assessments: document.assessments.map((item) => ({ ...item, propositionId: id })),
    relationships: []
  };
}
// Adding a proposition or relationship upgrades a single-proposition history to version 3.
function toInvestigation(previous) {
  const revisions = previous.schemaVersion === 2 ? previous.revisions
    : [{ reason: 'Imported version-1 baseline (original reason unknown)', document: previous }];
  return {
    schema: 'belief_person_attributions', schemaVersion: 3,
    revisions: revisions.map((revision) => ({
      reason: revision.reason, document: toSnapshot(revision.document),
      reassessed: (revision.reassessedPersonIds || []).map((personId) => ({ propositionId: revision.document.proposition.id, personId })),
      supersedes: revision.supersedes || {}
    }))
  };
}
function editableSnapshot() {
  const envelope = JSON.parse($('attribution-input').value);
  const document = currentAnnotation(envelope);
  return structuredClone(document.propositions ? document : toSnapshot(document));
}
function allObservationIds() {
  const envelope = JSON.parse($('attribution-input').value);
  const documents = envelope.revisions ? envelope.revisions.map((revision) => revision.document) : [envelope];
  return new Set(documents.flatMap((document) => document.observations.map((observation) => observation.id)));
}
function allRelationshipIds() {
  const envelope = JSON.parse($('attribution-input').value);
  return new Set((envelope.schemaVersion === 3 ? envelope.revisions : [])
    .flatMap((revision) => (revision.document.relationships || []).map((link) => link.id)));
}
function validateAnnotations(text) {
  return JSON.parse(compare_attributions_json($('evidence-input').value, text, JSON.stringify({ BELIEF_POLICY_PROFILE: 'semantic_research' })));
}
function commitRevision(document, reassessedPersonIds = [], supersedes = {}) {
  const reason = $('revision-reason').value;
  if (!reason.trim()) throw Error('Enter a revision reason before changing annotations.');
  const previous = JSON.parse($('attribution-input').value);
  let envelope;
  if (document.propositions) {
    envelope = previous.schemaVersion === 3 ? previous : toInvestigation(previous);
    envelope.revisions.push({
      reason, document, supersedes,
      reassessed: reassessedPersonIds.map((personId) => ({ propositionId: activeProposition, personId }))
    });
  } else {
    envelope = previous.schemaVersion === 2 ? previous : {
      schema: 'belief_person_attributions', schemaVersion: 2,
      revisions: [{ reason: 'Imported version-1 baseline (original reason unknown)', document: previous }]
    };
    envelope.revisions.push({ reason, document, reassessedPersonIds, supersedes });
  }
  const text = JSON.stringify(envelope, null, 2);
  const report = validateAnnotations(text);
  $('attribution-input').value = text;
  cancelCorrection();
  $('revision-reason').value = '';
  updateEvidenceOptions(); render(report); status('Comparison ready.');
}
function cancelCorrection() {
  correctionTarget = null;
  $('correction-status').textContent = '';
  $('cancel-correction').hidden = true;
}
function correctObservation(id) {
  try {
    const document = currentAnnotation();
    const observation = document.observations.find((item) => item.id === id);
    if (!observation) throw Error('Observation is no longer current.');
    $('source-evidence').value = observation.evidenceId; updatePersonName();
    $('kind').value = observation.kind; $('stance').value = observation.stance;
    $('passage').value = observation.passage;
    $('offset-start').value = observation.span.start; $('offset-end').value = observation.span.end;
    $('observation-score').value = observation.confidenceObservation?.value ?? '';
    $('conviction-score').value = observation.conviction?.value ?? '';
    $('conviction-basis').value = observation.conviction?.basis ?? '';
    $('expressed-at').value = observation.expressedAt ?? '';
    $('published-at').value = observation.publishedAt ?? '';
    $('discovered-at').value = observation.discoveredAt ?? '';
    $('person-score').value = ''; $('review-conflicts').checked = false;
    correctionTarget = id;
    $('correction-status').textContent = 'Correcting ' + id + '. Recording creates a new observation; the original stays in history.';
    $('cancel-correction').hidden = false;
    $('revision-reason').focus();
  } catch (error) { status(String(error.message || error), true); }
}
function withdrawObservation(id) {
  try {
    const document = currentAnnotation();
    const observation = document.observations.find((item) => item.id === id);
    if (!observation) throw Error('Observation is no longer current.');
    document.observations = document.observations.filter((item) => item.id !== id);
    document.assessments = document.assessments.filter((item) =>
      item.personId !== observation.personId || item.propositionId !== observation.propositionId);
    commitRevision(document);
  } catch (error) { status(String(error.message || error), true); }
}
function updateAssessmentOptions() {
  const selected = $('assessment-person').value;
  $('assessment-person').replaceChildren();
  const document = currentAnnotation();
  for (const person of document.people) {
    const option = element('option', person.displayName + ' · ' + person.id); option.value = person.id;
    $('assessment-person').append(option);
  }
  if (document.people.some((person) => person.id === selected)) $('assessment-person').value = selected;
  updateAssessmentSupport();
}
function updateAssessmentSupport() {
  const selected = new Set([...$('assessment-support').selectedOptions].map((option) => option.value));
  $('assessment-support').replaceChildren();
  try {
    const document = currentAnnotation();
    for (const observation of document.observations.filter((item) => item.personId === $('assessment-person').value && inActiveProposition(document, item))) {
      const option = element('option', observation.id + ' · ' + stanceLabel(observation.stance) + ' · ' + observation.passage);
      option.value = observation.id; option.selected = selected.has(observation.id); $('assessment-support').append(option);
    }
  } catch { /* Incomplete editor input has no assessable observations. */ }
}
function reassessPerson() {
  try {
    const document = currentAnnotation();
    const personId = $('assessment-person').value;
    const scope = $('assessment-scope').value;
    const assessment = {
      personId, stance: $('assessment-stance').value,
      observationIds: [...$('assessment-support').selectedOptions].map((option) => option.value),
      reviewedConflicts: $('assessment-reviewed').checked
    };
    const value = numeric('assessment-score');
    if (value !== null) assessment.confidencePerson = { value, assessedBy: 'analyst' };
    if (document.propositions) assessment.propositionId = activeProposition;
    if (scope === 'current') {
      assessment.timeScope = 'current';
      assessment.rationale = $('assessment-rationale').value;
      const asOf = $('assessment-as-of').value.trim();
      if (asOf) assessment.asOf = asOf;
    }
    const sameScope = (item) => item.personId === personId && inActiveProposition(document, item) &&
      (item.timeScope === 'current') === (scope === 'current');
    document.assessments = document.assessments.filter((item) => !sameScope(item));
    // "No reviewed stance" removes the assessment; an unknown current stance is simply absent.
    if (assessment.stance !== 'no_stance') document.assessments.push(assessment);
    commitRevision(document, [personId]);
  } catch (error) { status(String(error.message || error), true); }
}
function score(value) {
  if (!value) return 'Unknown — insufficient evidence';
  const assessor = value.assessedBy ?? value.assessed_by;
  const label = assessor === 'scripted_example' ? 'Scripted example' : assessor === 'analyst' ? 'Analyst assessment' : 'Unspecified assessor';
  return value.value.toLocaleString(undefined, { maximumFractionDigits: 3 }) + ' · ' + label;
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
// Versions 1 and 2 describe one proposition; version 3 holds several plus relationships.
function investigationView(report) {
  if (report.report === 'person_belief_investigation') return report;
  return {
    revision: report.revision, revision_reason: report.revision_reason, relationships: [], relationship_history: [],
    propositions: [{ proposition: report.proposition, people: report.people, history: report.history || [], supersedes: report.supersedes || {}, since_revision: 1 }]
  };
}
function propositionText(view, id) {
  return view.propositions.find((entry) => entry.proposition.id === id)?.proposition.text ?? id;
}
function render(report) {
  clearComparison();
  comparison = report;
  const view = investigationView(report);
  const ids = view.propositions.map((entry) => entry.proposition.id);
  if (!ids.includes(activeProposition)) activeProposition = ids[0];
  for (const select of [$('proposition-select'), $('link-from'), $('link-to')]) {
    const selected = select.value;
    select.replaceChildren();
    for (const entry of view.propositions) {
      const option = element('option', entry.proposition.text + ' · ' + entry.proposition.id);
      option.value = entry.proposition.id; select.append(option);
    }
    if (ids.includes(selected)) select.value = selected;
  }
  $('proposition-select').value = activeProposition;
  if ($('link-to').value === $('link-from').value && ids.length > 1) $('link-to').value = ids.find((id) => id !== $('link-from').value);
  const entry = view.propositions.find((item) => item.proposition.id === activeProposition);
  $('download-comparison').disabled = false;
  $('comparison-json').textContent = JSON.stringify(report, null, 2);
  const proposition = element('article');
  proposition.append(element('h3', entry.proposition.text), element('p', 'Scope: ' + entry.proposition.scope));
  proposition.append(element('small', 'One scoped proposition. Comparing people does not decide if it is true.'));
  $('proposition').append(proposition);
  for (const person of entry.people) $('comparison').append(personCard(person));
  $('current-revision').textContent = view.revision
    ? 'Current annotation revision ' + view.revision + ': ' + view.revision_reason
    : 'Version-1 annotations; earlier changes are not recorded.';
  if (entry.history.length) {
    $('history').append(element('h3', 'Attribution history'));
    for (const snapshot of entry.history) {
      const detail = element('details');
      detail.append(element('summary', 'Historical revision ' + snapshot.revision + ' — ' + snapshot.reason));
      detail.append(element('p', 'Investigator-supplied historical annotation snapshot — not the current assessment. Revalidated against the supplied evidence and current policy.'));
      for (const [newId, oldId] of Object.entries(snapshot.supersedes || {})) detail.append(element('p', newId + ' supersedes ' + oldId));
      for (const person of snapshot.people) detail.append(personCard(person, true));
      $('history').append(detail);
    }
  }
  for (const [newId, oldId] of Object.entries(entry.supersedes || {})) $('current-revision').append(element('span', ' · ' + newId + ' supersedes ' + oldId));
  renderRelationships(view);
  updateAssessmentOptions();
}
function relationshipCard(view, link, current) {
  const card = element('article'); card.className = 'person-card';
  const kinds = { implication: 'implies', equivalence: 'may be equivalent to', partial_overlap: 'partially overlaps' };
  card.append(element('h3', propositionText(view, link.from_proposition_id) + (link.directed ? ' → ' : ' ↔ ') + propositionText(view, link.to_proposition_id)));
  card.append(element('p', 'Suggested relationship: ' + kinds[link.kind] + (link.directed ? ' (this direction only; the reverse is not implied)' : '') + ' · ' + link.review_status));
  const details = element('dl'); details.className = 'provenance-list';
  appendDetail(details, 'Context', link.context);
  appendDetail(details, 'Reasoning', link.reasoning);
  appendDetail(details, 'Confidence (relationship)', score(link.confidence_relation));
  appendDetail(details, 'Suggested by', link.suggested_by.replaceAll('_', ' '));
  appendDetail(details, 'Review note', link.review_note);
  if (link.supersedes) appendDetail(details, 'Corrects', link.supersedes);
  card.append(details);
  card.append(element('small', 'Does not attribute either proposition to any person.'));
  if (current) {
    const controls = element('div'); controls.className = 'toolbar';
    for (const [label, action] of [['Accept', () => reviewRelationship(link.id, 'accepted')], ['Reject', () => reviewRelationship(link.id, 'rejected')], ['Correct', () => correctRelationship(link.id)]]) {
      const button = element('button', label + ' ' + link.id); button.addEventListener('click', action); controls.append(button);
    }
    card.append(controls);
  }
  return card;
}
function renderRelationships(view) {
  for (const link of view.relationships) $('relationships').append(relationshipCard(view, link, true));
  if (!view.relationships.length) $('relationships').append(element('p', 'No relationships between propositions.'));
  const earlier = view.relationship_history.filter((snapshot) => snapshot.relationships.length);
  if (earlier.length) {
    const detail = element('details');
    detail.append(element('summary', 'Relationship history'));
    for (const snapshot of earlier) {
      detail.append(element('h4', 'Revision ' + snapshot.revision + ' — ' + snapshot.reason));
      for (const link of snapshot.relationships) detail.append(relationshipCard(view, link, false));
    }
    $('relationships').append(detail);
  }
}
function addProposition() {
  try {
    const text = $('new-proposition-text').value.trim();
    const scope = $('new-proposition-scope').value.trim();
    if (!text || !scope) throw Error('A new proposition needs its exact wording and scope.');
    const snapshot = editableSnapshot();
    let next = snapshot.propositions.length + 1;
    while (snapshot.propositions.some((item) => item.id === 'proposition-' + next)) next++;
    snapshot.propositions.push({ id: 'proposition-' + next, text, scope });
    commitRevision(snapshot);
    $('new-proposition-text').value = ''; $('new-proposition-scope').value = '';
  } catch (error) { status(String(error.message || error), true); }
}
function suggestRelationship() {
  try {
    const snapshot = editableSnapshot();
    let next = 1;
    const used = allRelationshipIds();
    while (used.has('link-' + next)) next++;
    const link = {
      id: 'link-' + next, kind: $('link-kind').value,
      fromPropositionId: $('link-from').value, toPropositionId: $('link-to').value,
      context: $('link-context').value, reasoning: $('link-reasoning').value,
      suggestedBy: 'analyst', review: { status: 'suggested' }
    };
    const confidence = numeric('link-confidence');
    if (confidence !== null) link.confidenceRelation = { value: confidence, assessedBy: 'analyst' };
    if (linkCorrectionTarget !== null) {
      link.supersedes = linkCorrectionTarget;
      snapshot.relationships = snapshot.relationships.filter((item) => item.id !== linkCorrectionTarget);
    }
    snapshot.relationships.push(link);
    commitRevision(snapshot);
    cancelLinkCorrection();
    for (const id of ['link-context', 'link-reasoning', 'link-confidence', 'link-note']) $(id).value = '';
  } catch (error) { status(String(error.message || error), true); }
}
function reviewRelationship(id, statusValue) {
  try {
    const snapshot = editableSnapshot();
    const link = snapshot.relationships.find((item) => item.id === id);
    if (!link) throw Error('Relationship is no longer current.');
    link.review = { status: statusValue };
    const note = $('link-note').value.trim();
    if (note) link.review.note = note;
    commitRevision(snapshot);
    $('link-note').value = '';
  } catch (error) { status(String(error.message || error), true); }
}
function correctRelationship(id) {
  const link = editableSnapshot().relationships.find((item) => item.id === id);
  if (!link) return;
  $('link-from').value = link.fromPropositionId; $('link-to').value = link.toPropositionId;
  $('link-kind').value = link.kind; $('link-context').value = link.context; $('link-reasoning').value = link.reasoning;
  $('link-confidence').value = link.confidenceRelation?.value ?? '';
  linkCorrectionTarget = id;
  $('link-correction-status').textContent = 'Correcting ' + id + '. Suggesting creates a new relationship; the original stays in history.';
  $('cancel-link-correction').hidden = false;
  $('revision-reason').focus();
}
function cancelLinkCorrection() {
  linkCorrectionTarget = null;
  $('link-correction-status').textContent = '';
  $('cancel-link-correction').hidden = true;
}
function personCard(person, historical = false) {
    const card = element('article'); card.className = 'person-card';
    card.append(element('h3', person.display_name));
    const status = element('p', statusLabels[person.assessment_status] || person.assessment_status);
    status.className = 'stance-status'; status.dataset.status = person.assessment_status;
    card.append(status);
    card.append(element('p', 'Documented stance: ' + (person.stance ? stanceLabel(person.stance) : 'Not assessed')));
    card.append(element('small', 'Source-local person ' + person.subject_id + ' · ' + person.source_repository + ' / ' + person.scope_id));
    card.append(metric('Confidence (person)', score(person.confidence_person), 'How sure we are this person holds this position across cited observations. Not the truth of the proposition.'));
    card.append(element('p', 'Contributing observations: ' + (person.supporting_observation_ids.join(', ') || 'None — no current scored assessment')));
    if (person.has_conflicting_positions) card.append(element('p', 'Conflicting observations (both kept, no side chosen): ' + person.conflicting_observation_ids.join(', ')));
    const last = person.last_documented;
    card.append(metric('Last documented stance',
      last ? stanceLabel(last.stance) + ' · expressed ' + last.expressed_at.join(', ') + ' · ' + last.observation_ids.join(', ') : 'Unknown — no single dated stance',
      'Historical: what the person was observed to express, dated by when they said it.' + (last?.undated_observation_ids.length ? ' Undated observations not ordered: ' + last.undated_observation_ids.join(', ') + '.' : '')));
    const current = person.current_stance;
    card.append(metric('Current stance (tentative)',
      current.status === 'estimated'
        ? stanceLabel(current.stance) + (current.as_of ? ' · as of ' + current.as_of : '') + ' · ' + current.rationale
        : 'Unknown — no reviewed estimate',
      'A reviewable estimate, not a new observation. Elapsed time alone is not evidence.'));
    if (!person.observations.length) card.append(element('p', 'No admitted observations. Attribution unknown.'));
    for (const observation of person.observations) {
      const detail = element('details'); detail.open = true;
      detail.append(element('summary', observation.kind.replaceAll('_', ' ') + ' · ' + stanceLabel(observation.stance)));
      const quote = element('blockquote', observation.passage); detail.append(quote);
      detail.append(metric('Confidence (observation)', score(observation.confidence_observation), 'How strongly this passage implies this person holds the position.'));
      const conviction = observation.conviction
        ? score({ value: observation.conviction.value, assessed_by: observation.conviction.assessed_by }) + ' · ' + observation.conviction.basis.replaceAll('_', ' ')
        : 'Unknown — insufficient evidence';
      detail.append(metric('Conviction', conviction, 'The person’s own certainty about the proposition; self-reported and inferred estimates are distinct.'));
      const evidence = observation.evidence;
      const provenance = element('dl'); provenance.className = 'provenance-list';
      appendDetail(provenance, 'Expressed / published / discovered', [observation.expressed_at, observation.published_at, observation.discovered_at].map((value) => value ?? 'unknown').join(' / '));
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
    if (!historical) {
      for (const observation of person.observations) {
        const controls = element('div'); controls.className = 'toolbar';
        const correct = element('button', 'Correct ' + observation.id);
        correct.addEventListener('click', () => correctObservation(observation.id));
        const withdraw = element('button', 'Withdraw ' + observation.id);
        withdraw.addEventListener('click', () => withdrawObservation(observation.id));
        controls.append(correct, withdraw); card.append(controls);
      }
    }
    return card;
}

function runComparison() {
  if (!initialized) { status('Rust pipeline is not ready.', true); return; }
  try {
    const report = validateAnnotations($('attribution-input').value);
    render(report);
    status('Comparison ready.');
  } catch (error) { clearComparison(); status(String(error.message || error), true); }
}
$('theme').value = localStorage.getItem('belief-theme') || 'system';
function theme() { document.documentElement.dataset.theme = $('theme').value; localStorage.setItem('belief-theme', $('theme').value); }
$('theme').addEventListener('change', theme);
theme();
$('source-evidence').addEventListener('change', updatePersonName);
$('evidence-input').addEventListener('input', () => { cancelCorrection(); clearComparison(); updateEvidenceOptions(); });
$('attribution-input').addEventListener('input', () => { cancelCorrection(); clearComparison(); updatePersonName(); });
for (const [id, field] of [['evidence-file', 'evidence-input'], ['attribution-file', 'attribution-input']]) {
  $(id).addEventListener('change', async (event) => {
    try {
      const file = event.target.files[0];
      if (file) {
        const text = await readFile(file);
        const report = JSON.parse(compare_attributions_json(
          field === 'evidence-input' ? text : $('evidence-input').value,
          field === 'attribution-input' ? text : $('attribution-input').value,
          JSON.stringify({ BELIEF_POLICY_PROFILE: 'semantic_research' })
        ));
        $(field).value = text;
        cancelCorrection(); updateEvidenceOptions(); render(report);
        status('Comparison ready.');
      }
    } catch (error) { status(String(error.message || error), true); }
  });
}
$('load-example').addEventListener('click', loadExample);
$('compare').addEventListener('click', runComparison);
$('record').addEventListener('click', recordAttribution);
$('cancel-correction').addEventListener('click', cancelCorrection);
$('assessment-person').addEventListener('change', updateAssessmentSupport);
$('reassess').addEventListener('click', reassessPerson);
$('add-proposition').addEventListener('click', addProposition);
$('suggest-link').addEventListener('click', suggestRelationship);
$('cancel-link-correction').addEventListener('click', cancelLinkCorrection);
$('proposition-select').addEventListener('change', () => {
  activeProposition = $('proposition-select').value;
  cancelCorrection();
  if (comparison) render(comparison);
});
$('download-annotations').addEventListener('click', () => {
  try {
    validateAnnotations($('attribution-input').value);
    const annotations = JSON.parse($('attribution-input').value);
    if (annotations.schema !== 'belief_person_attributions' || ![1, 2, 3].includes(annotations.schemaVersion)) {
      throw Error('Open a valid person-attribution document before downloading.');
    }
    const url = URL.createObjectURL(new Blob([JSON.stringify(annotations, null, 2) + '\n'], { type: 'application/json' }));
    const anchor = element('a'); anchor.href = url; anchor.download = 'person-attributions.json'; anchor.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  } catch (error) { status(String(error.message || error), true); }
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
  $('reassess').disabled = false;
  $('add-proposition').disabled = false;
  $('suggest-link').disabled = false;
  $('download-annotations').disabled = false;
  await loadExample();
} catch (error) { status('Could not start the Rust attribution pipeline: ' + error.message, true); }
