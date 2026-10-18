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
if (personReport.people.length !== 2 || personReport.people[0].stance !== 'endorses' || personReport.people[1].stance !== 'disputes') {
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
await page.getByLabel('Observed stance').selectOption('undetermined');
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
  await replayPage.getByLabel('Observed stance').selectOption('undetermined');
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

if(errors.length) throw Error(errors.join('\n'));
console.log('Browser passed: evidence and person attribution, source spans, scores, contradictions, input rejection, mobile layout.');
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
