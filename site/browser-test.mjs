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
if (personReport.people.length !== 2 || personReport.people[0].stance !== 'endorses' || personReport.people[1].stance !== 'disputes') {
  throw Error('People do not have independently attributed opposing positions');
}
if (personReport.people[0].confidence_person.value !== 0.91 ||
    personReport.people[0].observations[1].confidence_observation.value !== 0.98 ||
    personReport.people[0].observations[1].conviction.value !== 0.3 ||
    personReport.people[0].observations[1].conviction.basis !== 'self_reported') {
  throw Error('Three score meanings were conflated');
}
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
await page.getByRole('button',{name:'Record attribution and compare'}).click();
await page.getByRole('status').filter({hasText:'Comparison ready.'}).waitFor();
personReport = JSON.parse(await page.locator('#comparison-json').textContent());
if (personReport.people[0].observations.length !== 3 ||
    personReport.people[0].confidence_person !== null ||
    personReport.people[0].observations[2].kind !== 'quotation_or_criticism') {
  throw Error('Recording new evidence failed to preserve it or invalidate stale confidence');
}
const [annotationDownload] = await Promise.all([
  page.waitForEvent('download'),
  page.getByRole('button',{name:'Download editable annotations'}).click()
]);
if (annotationDownload.suggestedFilename() !== 'person-attributions.json') {
  throw Error('Attribution annotation export has an unexpected name');
}
const exportedAnnotations = JSON.parse(await readFile(await annotationDownload.path(), 'utf8'));
if (!exportedAnnotations.observations.some((item) => item.id === 'manual-1')) {
  throw Error('Editable attribution export omitted the new source-backed observation');
}
await page.getByText('Inspect or import prepared source evidence and attribution annotations', {exact:true}).click();
const edited = JSON.parse(await page.locator('#attribution-input').inputValue());
edited.assessments[0].confidencePerson.semantics = 'soft_truth';
await page.locator('#attribution-input').fill(JSON.stringify(edited));
await page.getByRole('button',{name:'Compare positions'}).click();
await page.getByRole('status').filter({hasText:'unknown field'}).waitFor();

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
