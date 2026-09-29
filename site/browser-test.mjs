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

if(errors.length) throw Error(errors.join('\n'));
console.log('Browser passed: all 5 examples, malformed input, localization, theme, mobile layout.');
} finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
