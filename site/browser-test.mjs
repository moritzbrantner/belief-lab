import { chromium } from 'playwright';
import { createServer } from 'node:http';
import { readFile } from 'node:fs/promises';
import { resolve, extname } from 'node:path';
const root = resolve(import.meta.dirname,'../target/pages');
const server = createServer(async (request,response) => {
  const path = resolve(root,'.'+new URL(request.url,'http://localhost').pathname.replace(/\/$/,'/index.html'));
  if (!path.startsWith(root+'/')) { response.writeHead(403).end(); return; }
  try {
    const data = await readFile(path);
    response.setHeader('Content-Type',({'.wasm':'application/wasm','.js':'text/javascript','.html':'text/html','.css':'text/css','.json':'application/json'})[extname(path)] || 'application/octet-stream');
    response.end(data);
  } catch (error) { response.writeHead(error.code === 'ENOENT' ? 404 : 500).end(); }
});
await new Promise(resolve => server.listen(0,'127.0.0.1',resolve));
const browser = await chromium.launch({headless:true});
try {
const page = await browser.newPage();
const errors=[]; page.on('pageerror',error=>errors.push(error.message));
await page.goto(`http://127.0.0.1:${server.address().port}/`);
await page.getByRole('button',{name:'Analyze evidence',exact:true}).click();
await page.getByRole('status').filter({hasText:'Analysis complete.'}).waitFor();
const report = JSON.parse(await page.locator('#report').textContent());
if (report.inferences[0].belief.value !== 0.95) throw Error('Wrong browser belief');
for (const example of ['semantic-research','correlated-evidence','multimodal-gates','sensitive-and-identity']) {
  await page.locator('#example').selectOption(example);
  await page.getByRole('button',{name:'Load example',exact:true}).click();
  await page.getByRole('status').filter({hasText:'Ready to analyze.'}).waitFor();
  await page.getByRole('button',{name:'Analyze evidence',exact:true}).click();
  await page.getByRole('status').filter({hasText:'Analysis complete.'}).waitFor();
}
await page.getByText('Explicit policy settings',{exact:true}).click();
await page.locator('#settings').fill('{"BELIEF_UNKNOWN":"true"}');
await page.getByRole('button',{name:'Analyze evidence',exact:true}).click();
await page.getByRole('status').filter({hasText:'Input rejected'}).waitFor();
await page.locator('#settings').fill('{}');
await page.locator('#evidence').fill('{');
await page.getByRole('button',{name:'Analyze evidence',exact:true}).click();
await page.getByRole('status').filter({hasText:'Input rejected'}).waitFor();
await page.locator('#language').selectOption('de');
await page.getByRole('button',{name:'Evidenz analysieren',exact:true}).waitFor();
await page.locator('#theme').selectOption('dark');
await page.setViewportSize({width:390,height:844});
if (await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth)) throw Error('Mobile horizontal overflow');


// Real Pages WebAssembly boundary: record and compare source-scoped person beliefs.
await page.setViewportSize({width:1240,height:900});
await page.goto('http://127.0.0.1:' + server.address().port + '/people.html');
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
let personReport = JSON.parse(await page.locator('#comparison-json').textContent());
const initialPersonReport = structuredClone(personReport);
if (personReport.people.length !== 2 || personReport.people[0].stance !== 'endorses' || personReport.people[1].stance !== 'endorses_negation') {
  throw Error('People do not have independently attributed opposing positions');
}
if (personReport.people[0].confidence_person.value !== 0.91 ||
    personReport.people[0].observations[1].confidence_observation.value !== 0.98 ||
    personReport.people[0].observations[1].conviction.value !== 0.3 ||
    personReport.people[0].observations[1].conviction.basis !== 'self_reported') {
  throw Error('Three score meanings were conflated');
}
if (personReport.people[0].confidence_person.assessedBy !== 'scripted_example') {
  throw Error('JSON assessment provenance was not preserved');
}
await page.getByText('0.91 · Scripted example', {exact:true}).waitFor();
await page.getByText('0.98 · Scripted example', {exact:true}).waitFor();
if (personReport.soft_truth !== undefined || personReport.truth !== undefined) {
  throw Error('Legacy inference score was presented as attribution or truth');
}
await page.getByText("I think the effect may last into 2027, but I'm only about 30% sure.", { exact:true }).waitFor();
await page.screenshot({path:resolve(root, '../person-belief-comparison.png'),fullPage:true});

await page.getByLabel('Evidence record').selectOption('e-quote');
await page.getByLabel('Attribution type').selectOption('quotation_or_criticism');
await page.getByLabel('Observed stance').selectOption('no_stance');
await page.getByLabel('Source passage').fill('A researcher was quoted, but the author did not endorse that statement.');
await page.getByLabel('Source span end offset').fill('73');
const unchangedAnnotations = await page.locator('#attribution-input').inputValue();
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Enter a revision reason'}).waitFor();
if (await page.locator('#attribution-input').inputValue() !== unchangedAnnotations) throw Error('Missing reason changed annotations');
await page.getByLabel('Revision reason').fill('Record quotation without attributing endorsement');
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (personReport.people[0].observations.length !== 3 ||
    personReport.people[0].confidence_person !== null ||
    personReport.people[0].observations[2].kind !== 'quotation_or_criticism') {
  throw Error('Recording new evidence failed to preserve it or invalidate stale confidence');
}
if (personReport.revision !== 2 || personReport.history.length !== 1 ||
    JSON.stringify(personReport.history[0].people) !== JSON.stringify(initialPersonReport.people)) {
  throw Error('Initial person assessments were not preserved in ordered history');
}
await page.getByText('Historical revision 1 — Imported version-1 baseline (original reason unknown)', {exact:true}).click();
await page.getByText('Investigator-supplied historical annotation snapshot — not the current assessment.', {exact:false}).waitFor();
await page.screenshot({path:resolve(root, '../attribution-history.png'),fullPage:true});
// Retrospective acceptance: preserve the investigator's exact quoted bytes,
// including whitespace, alongside the supplied source offsets (Atlas 1.4.3).
const exactPassage = '  A source-local quotation.\n';
await page.getByLabel('Source passage').fill(exactPassage);
await page.getByLabel('Revision reason').fill('Preserve exact quotation whitespace');
await page.getByLabel('Source span start offset').fill('17');
await page.getByLabel('Source span end offset').fill(String(17 + exactPassage.length));
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
const exactObservation = personReport.people[0].observations.find((item) => item.id === 'manual-2');
if (exactObservation?.passage !== exactPassage || exactObservation.span.start !== 17 ||
    exactObservation.span.end !== 17 + exactPassage.length) {
  throw Error('Source passage whitespace or offsets changed during recording');
}
const [annotationDownload] = await Promise.all([
  page.waitForEvent('download'),
  page.getByRole('button',{name:'Download editable annotations'}).click()
]);
if (annotationDownload.suggestedFilename() !== 'person-attributions.json') {
  throw Error('Attribution annotation export has an unexpected name');
}
const exportedAnnotations = JSON.parse(await readFile(await annotationDownload.path(), 'utf8'));
const exportedCurrent = exportedAnnotations.revisions.at(-1).document;
if (exportedAnnotations.schemaVersion !== 2 || exportedAnnotations.revisions.length !== 3) throw Error('Editable export omitted revision log');
if (!exportedCurrent.observations.some((item) => item.id === 'manual-1')) {
  throw Error('Editable attribution export omitted the new source-backed observation');
}
if (exportedCurrent.observations.find((item) => item.id === 'manual-2')?.passage !== exactPassage) {
  throw Error('Editable export changed the exact source passage');
}
// Correct then withdraw through the real Rust/WASM UI; originals stay historical.
await page.getByRole('button', {name:'Correct manual-2', exact:true}).click();
const correctedPassage = '  Corrected quotation.\n';
await page.getByLabel('Source passage').fill(correctedPassage);
await page.getByLabel('Source span end offset').fill(String(17 + correctedPassage.length));
await page.getByLabel('Revision reason').fill('Correct the supplied source passage');
await page.getByRole('button',{name:'Record attribution and compare'}).click();
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (personReport.revision !== 4 || personReport.supersedes['manual-3'] !== 'manual-2' ||
    personReport.people[0].observations.some((observation) => observation.id === 'manual-2') ||
    personReport.people[0].observations.find((observation) => observation.id === 'manual-3')?.passage !== correctedPassage ||
    personReport.history[2].people[0].observations.find((observation) => observation.id === 'manual-2')?.passage !== exactPassage) {
  throw Error('Correction did not preserve supersession and original passage history');
}
await page.screenshot({path:resolve(root, '../attribution-correction.png'),fullPage:true});
await page.getByLabel('Revision reason').fill('Withdraw the corrected quotation');
await page.getByRole('button', {name:'Withdraw manual-3', exact:true}).click();
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (personReport.revision !== 5 || personReport.people[0].observations.some((observation) => observation.id === 'manual-3') ||
    !personReport.history[3].people[0].observations.some((observation) => observation.id === 'manual-3')) {
  throw Error('Withdrawal erased history or retained a current contribution');
}
await page.getByLabel('Person to reassess').selectOption('blogger-a');
await page.getByLabel('Assessed stance').selectOption('endorses');
await page.getByLabel('Reviewed confidence (person)', {exact:true}).fill('0.91');
await page.getByLabel('Supporting observations').selectOption(['a-stated']);
for (const reason of ['Reassess the remaining supported position', 'Explicit review retaining the same score']) {
  await page.getByLabel('Revision reason').fill(reason);
  await page.getByRole('button',{name:'Reassess person and compare'}).click();
}
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (personReport.revision !== 7 || personReport.people[0].confidence_person.value !== 0.91 ||
    personReport.history[4].people[0].confidence_person !== null ||
    personReport.history[5].people[0].confidence_person.value !== 0.91 ||
    JSON.stringify(personReport.people[1]) !== JSON.stringify(initialPersonReport.people[1])) {
  throw Error('Explicit reassessment lost history or changed another person');
}
await page.locator('#comparison').getByText('Contributing observations: a-stated', {exact:true}).waitFor();
const [historyDownload] = await Promise.all([
  page.waitForEvent('download'), page.getByRole('button',{name:'Download editable annotations'}).click()
]);
const historyBytes = await readFile(await historyDownload.path());
const replayPage = await browser.newPage({viewport:{width:1240,height:900}});
replayPage.on('pageerror', (error) => errors.push(String(error)));
try {
  await replayPage.goto('http://127.0.0.1:' + server.address().port + '/people.html');
  await replayPage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
  await replayPage.locator('#attribution-file').setInputFiles({name:'history.json',mimeType:'application/json',buffer:historyBytes});
  await replayPage.waitForFunction(() => document.getElementById('current-revision').textContent.includes('Current annotation revision 7:'));
  const replayed = JSON.parse(await replayPage.locator('#comparison-json').textContent());
  if (JSON.stringify(replayed) !== JSON.stringify(personReport)) throw Error('History download/reimport did not replay exactly');
  const validText = await replayPage.locator('#attribution-input').inputValue();
  const malformedOld = JSON.parse(validText);
  malformedOld.revisions[0].document.people[0].sourceRepository = 'wrong-source';
  const emptyReason = JSON.parse(validText); emptyReason.revisions[0].reason = ' ';
  for (const bad of ['not JSON', JSON.stringify(malformedOld), JSON.stringify(emptyReason)]) {
    await replayPage.locator('#attribution-file').setInputFiles({name:'bad-history.json',mimeType:'application/json',buffer:Buffer.from(bad)});
    await replayPage.locator('#people-status[data-error=true]').waitFor();
    if (await replayPage.locator('#attribution-input').inputValue() !== validText ||
        JSON.stringify(JSON.parse(await replayPage.locator('#comparison-json').textContent())) !== JSON.stringify(replayed)) {
      throw Error('Invalid imported history replaced the valid current state');
    }
  }
  await replayPage.getByLabel('Evidence record').selectOption('e-quote');
  await replayPage.getByLabel('Attribution type').selectOption('quotation_or_criticism');
  await replayPage.getByLabel('Observed stance').selectOption('no_stance');
  await replayPage.getByLabel('Source passage', {exact:true}).fill('A continued annotation after transfer.');
  await replayPage.getByLabel('Source span end offset').fill('37');
  await replayPage.getByLabel('Revision reason').fill('Continue the transferred annotation history');
  await replayPage.getByRole('button',{name:'Record attribution and compare'}).click();
  const continued = JSON.parse(await replayPage.locator('#comparison-json').textContent());
  if (continued.revision !== 8 || continued.people[0].confidence_person !== null || continued.history.length !== 7) throw Error('Transferred history could not continue');
  const [continuedDownload] = await Promise.all([
    replayPage.waitForEvent('download'), replayPage.getByRole('button',{name:'Download editable annotations'}).click()
  ]);
  const continuedBytes = await readFile(await continuedDownload.path());
  await page.locator('#attribution-file').setInputFiles({name:'continued.json',mimeType:'application/json',buffer:continuedBytes});
  await page.waitForFunction(() => document.getElementById('current-revision').textContent.includes('Current annotation revision 8:'));
  if (JSON.stringify(JSON.parse(await page.locator('#comparison-json').textContent())) !== JSON.stringify(continued)) throw Error('Second history transfer lost revisions');
  await replayPage.setViewportSize({width:390,height:844});
  await replayPage.getByText('Historical revision 1 — Imported version-1 baseline (original reason unknown)', {exact:true}).click();
  if (await replayPage.evaluate(() => document.documentElement.scrollWidth > innerWidth)) throw Error('History overflows mobile viewport');
} finally { await replayPage.close(); }
// Decisions 1.4.25–1.4.31: dated last-documented stance beside an unknown current stance,
// explicit suspension as its own stance, and a reviewable current-stance estimate.
let stanceView = JSON.parse(await page.locator('#comparison-json').textContent());
const researcherView = stanceView.people.find((person) => person.id === 'researcher-b');
if (researcherView.last_documented?.stance !== 'endorses_negation' || researcherView.last_documented.expressed_at.join() !== '2024' ||
    researcherView.last_documented.undated_observation_ids.join() !== 'b-inferred' || researcherView.current_stance.status !== 'unknown') {
  throw Error('Old documented stance was promoted or its date lost');
}
await page.locator('#comparison').getByText('Endorses not-P · expressed 2024 · b-reported', {exact:true}).waitFor();
await page.locator('#comparison').getByText('Unknown — no reviewed estimate').first().waitFor();
await page.getByLabel('Evidence record').selectOption('e-blog');
await page.getByLabel('Attribution type').selectOption('inferred_belief');
await page.getByLabel('Observed stance').selectOption('suspends_judgment');
await page.getByLabel('Source passage', {exact:true}).fill("I honestly don't know yet whether it lasts.");
await page.getByLabel('Source span start offset').fill('0');
await page.getByLabel('Source span end offset').fill('43');
await page.getByLabel('Expressed on').fill('2026-09');
await page.getByLabel('Revision reason').fill('Record a later statement withholding judgment');
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'not inferred'}).waitFor();
await page.getByLabel('Attribution type').selectOption('explicit_endorsement');
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
stanceView = JSON.parse(await page.locator('#comparison-json').textContent());
const bloggerView = stanceView.people.find((person) => person.id === 'blogger-a');
if (bloggerView.assessment_status !== 'unassessed' || bloggerView.last_documented?.stance !== 'suspends_judgment' ||
    bloggerView.has_conflicting_positions || bloggerView.stance !== null) {
  throw Error('Explicit suspension was conflated with conflict, endorsement or an assessed stance');
}
await page.locator('#comparison').getByText(/^Explicitly suspends judgment · expressed 2026-09/).waitFor();
await page.getByLabel('Person to reassess').selectOption('researcher-b');
await page.locator('#assessment-scope').selectOption('current');
await page.getByLabel('Assessed stance').selectOption('endorses_negation');
await page.getByLabel('Reviewed confidence (person)', {exact:true}).fill('');
await page.getByLabel('Supporting observations').selectOption(['b-reported']);
await page.getByLabel('Estimate as of').fill('2026-10');
await page.getByLabel('Revision reason').fill('Estimate the present stance from old evidence');
await page.getByRole('button',{name:'Reassess person and compare'}).click();
await page.getByRole('status').filter({hasText:'rationale'}).waitFor();
await page.getByLabel('Rationale for a current-stance estimate').fill('Only a 2024 interview; no newer statement.');
await page.getByRole('button',{name:'Reassess person and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
stanceView = JSON.parse(await page.locator('#comparison-json').textContent());
const estimated = stanceView.people.find((person) => person.id === 'researcher-b');
if (estimated.current_stance.status !== 'estimated' || estimated.current_stance.as_of !== '2026-10' ||
    estimated.stance !== 'endorses_negation' || estimated.confidence_person.value !== 0.73 ||
    estimated.last_documented.expressed_at.join() !== '2024') {
  throw Error('Current-stance estimate replaced the documented history or assessment');
}
await page.locator('#comparison').getByText('Endorses not-P · as of 2026-10 · Only a 2024 interview; no newer statement.', {exact:true}).waitFor();
await page.screenshot({path:resolve(root, '../stance-states.png'),fullPage:true});

// Decision 1.4.33: a second proposition and reviewable relationships upgrade the file to version 3.
await page.getByLabel('New proposition text').fill("The intervention's effects last through 2026");
await page.getByLabel('New proposition scope').fill('Adults who received intervention X in 2026');
await page.getByLabel('Revision reason').fill('Add the 2026 proposition');
await page.getByRole('button',{name:'Add proposition'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
let investigationReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (investigationReport.report !== 'person_belief_investigation' || investigationReport.propositions.length !== 2 ||
    JSON.parse(await page.locator('#attribution-input').inputValue()).schemaVersion !== 3) {
  throw Error('Adding a proposition did not create a version-3 investigation');
}
await page.getByLabel('From proposition').selectOption('effects-2027');
await page.locator('#link-kind').selectOption('implication');
await page.getByLabel('To proposition').selectOption('proposition-2');
await page.getByLabel('Context in which it holds').fill('Same adults; "through" means continuously');
await page.getByLabel('Source-grounded reasoning').fill('Lasting through 2027 includes lasting through 2026.');
await page.getByLabel('Revision reason').fill('Suggest that P implies Q');
await page.getByRole('button',{name:'Suggest relationship'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
await page.getByLabel('Review note').fill('Scopes match');
await page.getByLabel('Revision reason').fill('Accept the implication');
await page.getByRole('button',{name:'Accept link-1', exact:true}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
investigationReport = JSON.parse(await page.locator('#comparison-json').textContent());
const qView = investigationReport.propositions.find((entry) => entry.proposition.id === 'proposition-2');
if (investigationReport.relationships.length !== 1 || investigationReport.relationships[0].review_status !== 'accepted' ||
    investigationReport.relationships[0].from_proposition_id !== 'effects-2027' || !investigationReport.relationships[0].directed ||
    qView.people.some((person) => person.assessment_status !== 'no_stance_evidence' || person.observations.length)) {
  throw Error('An accepted implication was reversed or created person attributions for Q');
}
await page.locator('#relationships').getByText('Suggested relationship: implies (this direction only; the reverse is not implied) · accepted', {exact:true}).waitFor();
await page.getByRole('button',{name:'Correct link-1', exact:true}).click();
await page.locator('#link-kind').selectOption('partial_overlap');
await page.getByLabel('Revision reason').fill('Only a partial overlap after review');
await page.getByRole('button',{name:'Suggest relationship'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
investigationReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (investigationReport.relationships[0].supersedes !== 'link-1' || investigationReport.relationships[0].directed ||
    !investigationReport.relationship_history.some((snapshot) => snapshot.relationships.some((link) => link.id === 'link-1'))) {
  throw Error('Relationship correction lost the original record');
}
await page.locator('#proposition-select').selectOption('proposition-2');
await page.getByLabel('Evidence record').selectOption('e-report');
await page.getByLabel('Attribution type').selectOption('reported_position');
await page.getByLabel('Observed stance').selectOption('endorses');
await page.getByLabel('Source passage', {exact:true}).fill('Researcher B agreed the effect holds through 2026.');
await page.getByLabel('Source span start offset').fill('0');
await page.getByLabel('Source span end offset').fill('50');
await page.getByLabel('Expressed on').fill('2024');
await page.getByLabel('Revision reason').fill('Record a Q-specific observation');
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
investigationReport = JSON.parse(await page.locator('#comparison-json').textContent());
const qResearcher = investigationReport.propositions.find((entry) => entry.proposition.id === 'proposition-2').people.find((person) => person.id === 'researcher-b');
const pResearcher = investigationReport.propositions.find((entry) => entry.proposition.id === 'effects-2027').people.find((person) => person.id === 'researcher-b');
if (qResearcher.observations.length !== 1 || qResearcher.assessment_status !== 'unassessed' ||
    pResearcher.stance !== 'endorses_negation' || pResearcher.observations.some((item) => item.passage.includes('2026'))) {
  throw Error('Observations leaked between propositions');
}
await page.screenshot({path:resolve(root, '../semantic-links.png'),fullPage:true});
const [investigationDownload] = await Promise.all([
  page.waitForEvent('download'), page.getByRole('button',{name:'Download editable annotations'}).click()
]);
const investigationBytes = await readFile(await investigationDownload.path());
const investigationPage = await browser.newPage({viewport:{width:1240,height:900}});
investigationPage.on('pageerror', (error) => errors.push(String(error)));
try {
  await investigationPage.goto('http://127.0.0.1:' + server.address().port + '/people.html');
  await investigationPage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
  await investigationPage.locator('#attribution-file').setInputFiles({name:'investigation.json',mimeType:'application/json',buffer:investigationBytes});
  await investigationPage.waitForFunction(() => document.getElementById('current-revision').textContent.includes('Record a Q-specific observation'));
  const replayedInvestigation = JSON.parse(await investigationPage.locator('#comparison-json').textContent());
  if (JSON.stringify(replayedInvestigation) !== JSON.stringify(investigationReport)) throw Error('Version-3 download/reimport did not replay exactly');
} finally { await investigationPage.close(); }
await page.locator('#proposition-select').selectOption('effects-2027');
await page.getByText('Inspect or import prepared source evidence and attribution annotations', {exact:true}).click();
const edited = JSON.parse(await page.locator('#attribution-input').inputValue());
edited.revisions[0].document.assessments[0].confidencePerson.semantics = 'soft_truth';
await page.locator('#attribution-input').fill(JSON.stringify(edited));
await page.getByRole('button',{name:'Compare positions'}).click();
await page.locator('#people-status[data-error=true]').waitFor();

await page.locator('#attribution-input').fill('not JSON');
await page.getByRole('button',{name:'Compare positions'}).click();
await page.locator('#people-status[data-error=true]').waitFor();
await page.setViewportSize({width:390,height:844});
if (await page.evaluate(() => document.documentElement.scrollWidth > innerWidth)) {
  throw Error('Person-belief comparison overflows mobile viewport');
}

// #35 slice 1: separate persistent investigations, project export, offline import, continued work.
const peopleUrl = 'http://127.0.0.1:' + server.address().port + '/people.html';
const workspace = await browser.newContext({viewport:{width:1240,height:900}});
const ready = (target) => target.getByRole('status').filter({hasText:/Comparison ready\.|Add a producer evidence batch/}).waitFor();
let projectPage = await workspace.newPage();
projectPage.on('pageerror', (error) => errors.push(String(error)));
await projectPage.goto(peopleUrl);
await ready(projectPage);
await projectPage.getByLabel('New investigation name').fill('Research B');
await projectPage.getByRole('button',{name:'Start new investigation'}).click();
await projectPage.waitForFunction(() => document.getElementById('investigation-select').value.startsWith('research-b-'));
await projectPage.getByRole('status').filter({hasText:'Add a producer evidence batch'}).waitFor();
const researchB = await projectPage.locator('#investigation-select').inputValue();
if (!researchB.startsWith('research-b-') || await projectPage.locator('#attribution-input').inputValue() !== '') throw Error('A new investigation was not empty and separate');
const exampleOption = await projectPage.locator('#investigation-select option', {hasText: 'Two-person example'}).getAttribute('value');
await projectPage.locator('#investigation-select').selectOption(exampleOption);
await projectPage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
const researchA = await projectPage.locator('#investigation-select').inputValue();
await projectPage.getByLabel('Revision reason').fill('Add a proposition only in A');
await projectPage.getByLabel('New proposition text').fill('A-only proposition');
await projectPage.getByLabel('New proposition scope').fill('Investigation A');
await projectPage.getByRole('button',{name:'Add proposition'}).click();
await projectPage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
await projectPage.waitForFunction(() => document.getElementById('investigation-select').options.length === 2);
await projectPage.close();
// Reopen the session: the last updated investigation (A) resumes; B stays empty and separate.
projectPage = await workspace.newPage();
projectPage.on('pageerror', (error) => errors.push(String(error)));
await projectPage.goto(peopleUrl);
await ready(projectPage);
let resumed = JSON.parse(await projectPage.locator('#comparison-json').textContent());
if (await projectPage.locator('#investigation-select').inputValue() !== researchA || resumed.propositions?.length !== 2) throw Error('Saved investigation A did not resume with its history');
await projectPage.locator('#investigation-select').selectOption(researchB);
await projectPage.getByRole('status').filter({hasText:'Add a producer evidence batch'}).waitFor();
if (await projectPage.locator('#attribution-input').inputValue() !== '' || await projectPage.locator('#evidence-input').inputValue() !== '[]') throw Error('Investigation B received A\'s data');
await projectPage.locator('#investigation-select').selectOption(researchA);
await projectPage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
const [projectDownload] = await Promise.all([
  projectPage.waitForEvent('download'), projectPage.getByRole('button',{name:'Export project'}).click()
]);
if (!projectDownload.suggestedFilename().endsWith('.belief-project.json')) throw Error('Unexpected project file name');
const projectBytes = await readFile(await projectDownload.path());
const exportedProject = JSON.parse(projectBytes);
if (exportedProject.investigation.id !== researchA || exportedProject.annotations.schemaVersion !== 3 || exportedProject.evidenceBatches.length !== 1) throw Error('Project export omitted investigation state');
await workspace.close();

// A fresh device: load the app, go offline, import the project, add evidence and continue.
const device = await browser.newContext({viewport:{width:1240,height:900}});
const devicePage = await device.newPage();
devicePage.on('pageerror', (error) => errors.push(String(error)));
await devicePage.goto(peopleUrl);
await ready(devicePage);
await device.setOffline(true);
await devicePage.locator('#project-file').setInputFiles({name:'a.belief-project.json',mimeType:'application/json',buffer:projectBytes});
await devicePage.waitForFunction((id) => document.getElementById('investigation-select').value === id, researchA);
const imported = JSON.parse(await devicePage.locator('#comparison-json').textContent());
if (JSON.stringify(imported) !== JSON.stringify(resumed)) throw Error('Imported project did not replay to the saved analysis');
const followUp = JSON.parse(await readFile(resolve(root, 'examples/person-attributions/evidence.json'), 'utf8'));
followUp.revision = 'sha256:follow-up-interview-v1';
followUp.schemaVersion = 2;
followUp.evidence = [{ ...followUp.evidence[2], id: 'e-follow-up', source: { ...followUp.evidence[2].source, recordId: 'paragraph-9' },
  mediaSpan: { start: { pts: 1500, timebase: { num: 1, den: 1000 } }, end: { pts: 4200, timebase: { num: 1, den: 1000 } } }, sceneIds: ['scene:3'] }];
await devicePage.getByLabel('Add a producer evidence batch').setInputFiles({name:'follow-up.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(followUp))});
await devicePage.waitForFunction(() => document.getElementById('current-investigation').textContent.includes('2 evidence batch(es)'));
await devicePage.locator('#proposition-select').selectOption('effects-2027');
await devicePage.getByLabel('Evidence record').selectOption('e-follow-up');
await devicePage.getByLabel('Attribution type').selectOption('explicit_endorsement');
await devicePage.getByLabel('Observed stance').selectOption('endorses_negation');
await devicePage.getByLabel('Source passage', {exact:true}).fill('I still doubt the 2027 claim.');
await devicePage.getByLabel('Source span start offset').fill('0');
await devicePage.getByLabel('Source span end offset').fill('29');
await devicePage.getByLabel('Expressed on').fill('2026-09');
await devicePage.getByLabel('Revision reason').fill('Continue offline with follow-up evidence');
await devicePage.getByRole('button',{name:'Record attribution and compare'}).click();
await devicePage.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
const continuedProject = JSON.parse(await devicePage.locator('#comparison-json').textContent());
const followResearcher = continuedProject.propositions.find((entry) => entry.proposition.id === 'effects-2027').people.find((person) => person.id === 'researcher-b');
await devicePage.locator('#comparison').getByText('1.5 s – 4.2 s · scenes scene:3', {exact:true}).waitFor();
if (!followResearcher.observations.some((item) => item.evidence.id === 'e-follow-up') || followResearcher.last_documented?.expressed_at.join() !== '2026-09') {
  throw Error('Offline continuation lost the new evidence or its date');
}
const [secondDownload] = await Promise.all([
  devicePage.waitForEvent('download'), devicePage.getByRole('button',{name:'Export project'}).click()
]);
const secondBytes = await readFile(await secondDownload.path());
await device.close();
const third = await browser.newContext();
const thirdPage = await third.newPage();
thirdPage.on('pageerror', (error) => errors.push(String(error)));
await thirdPage.goto(peopleUrl);
await ready(thirdPage);
await thirdPage.locator('#project-file').setInputFiles({name:'a2.belief-project.json',mimeType:'application/json',buffer:secondBytes});
await thirdPage.waitForFunction(() => document.getElementById('current-investigation').textContent.includes('2 evidence batch(es)'));
if (JSON.stringify(JSON.parse(await thirdPage.locator('#comparison-json').textContent())) !== JSON.stringify(continuedProject)) throw Error('Repeated transfer lost history');
const tampered = JSON.parse(secondBytes);
tampered.annotations.revisions[0].document.observations[0].passage = 'Rewritten history';
const before = await thirdPage.locator('#comparison-json').textContent();
await thirdPage.locator('#project-file').setInputFiles({name:'tampered.json',mimeType:'application/json',buffer:Buffer.from(JSON.stringify(tampered))});
await thirdPage.locator('#people-status[data-error=true]').waitFor();
if (await thirdPage.locator('#comparison-json').textContent() !== before) throw Error('A tampered project replaced the valid state');
await thirdPage.screenshot({path:resolve(root, '../investigations.png'),fullPage:true});
await third.close();

if(errors.length) throw Error(errors.join('\n'));
console.log('Browser passed: evidence and person attribution, source spans, scores, contradictions, stance states, semantic links, investigations, input rejection, mobile layout.');
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
