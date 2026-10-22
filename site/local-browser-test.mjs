import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve } from 'node:path';
import { chromium } from 'playwright';

const root = resolve(import.meta.dirname, '..');
// Saved investigations go to a temporary directory, never the user's data directory.
const dataDir = await mkdtemp(resolve(tmpdir(), 'belief-lab-native-'));
const child = spawn(resolve(root, 'target/debug', process.platform === 'win32' ? 'belief.exe' : 'belief'), ['workbench', '--data-dir', dataDir], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
// Never print the session capability URL in test output.
let diagnostics = '';
child.stderr.on('data', (data) => { diagnostics += data.toString(); });
const stopped = once(child, 'exit');
let browser;
try {
  const url = await new Promise((resolve, reject) => {
    let output = '';
    const timer = setTimeout(() => reject(new Error('Workbench startup timed out')), 15000);
    child.once('error', (error) => { clearTimeout(timer); reject(error); });
    child.once('exit', () => { clearTimeout(timer); reject(new Error(`Workbench stopped: ${diagnostics}`)); });
    child.stdout.on('data', (data) => {
      output += data;
      if (output.includes('\n')) { clearTimeout(timer); resolve(output.split('\n')[0]); }
    });
  });
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', (error) => errors.push(error.message));
  await page.goto(url);
  await page.getByRole('status').filter({ hasText: 'Ready to analyze.' }).waitFor();
  await page.getByLabel('Model', { exact: true }).selectOption('fixture');
  await page.getByRole('button', { name: 'Run analysis', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Analysis complete.' }).waitFor();
  let result = JSON.parse(await page.locator('#json').textContent());
  assert.equal(result.belief.value, 0.95);
  assert.equal(result.decision.provider, 'fixture');
  await page.getByText('Scripted result — no model ran.', { exact: true }).waitFor();
  const downloadPromise = page.waitForEvent('download');
  await page.getByRole('button', { name: 'Download result', exact: true }).click();
  const download = await downloadPromise;
  assert.deepEqual(JSON.parse(await readFile(await download.path(), 'utf8')), result);

  const request = JSON.parse(await page.getByLabel('Decision request JSON').inputValue());
  request.policy.BELIEF_POLICY_PROFILE = 'observe_only';
  await page.getByLabel('Decision request JSON').fill(JSON.stringify(request));
  await page.getByRole('button', { name: 'Run analysis', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Request failed' }).waitFor();
  assert.equal(JSON.parse(await page.locator('#json').textContent()).error.code, 'policy_denied');
  await page.getByLabel('Request file', { exact: true }).setInputFiles(resolve(root, 'examples/decisions/fixture.json'));
  await page.getByRole('status').filter({ hasText: 'Ready to analyze.' }).waitFor();
  await page.getByRole('button', { name: 'Run analysis', exact: true }).click();
  await page.getByRole('status').filter({ hasText: 'Analysis complete.' }).waitFor();

  if (process.argv.includes('--real')) {
    await page.getByLabel('Model', { exact: true }).selectOption('phone');
    await page.getByRole('button', { name: 'Run analysis', exact: true }).click();
    assert.equal(await page.getByRole('button', { name: 'Run analysis', exact: true }).isDisabled(), true);
    await page.getByRole('status').filter({ hasText: 'Analysis complete.' }).waitFor({ timeout: 600000 });
    result = JSON.parse(await page.locator('#json').textContent());
    assert.equal(result.decision.provider, 'semif');
    assert.equal(result.decision.model, 'Qwen/Qwen3-0.6B');
    assert.equal(result.decision.selectedOption, 'supports');
    assert.equal(result.belief.semantics, 'soft_truth');
  }
  await page.getByLabel('Language', { exact: true }).selectOption('de');
  await page.getByRole('button', { name: 'Analyse starten', exact: true }).waitFor();
  await page.getByLabel('Darstellung', { exact: true }).selectOption('dark');
  await page.setViewportSize({ width: 390, height: 844 });
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
  const withoutToken = await browser.newPage();
  await withoutToken.goto(url.split('#')[0]);
  await withoutToken.getByRole('status').filter({ hasText: 'Open the full session URL' }).waitFor();
  assert.equal(await withoutToken.getByRole('button', { name: 'Run analysis', exact: true }).isDisabled(), true);
  // The same investigation pages, saved as validated files by the native workbench (#35).
  const peopleUrl = url.replace('/#token=', '/people.html#token=');
  const investigations = await browser.newPage();
  investigations.on('pageerror', (error) => errors.push(error.message));
  await investigations.goto(peopleUrl);
  await investigations.getByRole('status').filter({ hasText: 'Comparison ready.' }).waitFor();
  await investigations.getByText('Saved on this computer by the local workbench.', { exact: true }).waitFor();
  const saved = await readdir(dataDir);
  assert.equal(saved.length, 1);
  const stored = JSON.parse(await readFile(resolve(dataDir, saved[0]), 'utf8'));
  assert.equal(stored.investigation.name, 'Two-person example');
  await investigations.getByLabel('New investigation name').fill('Native research');
  await investigations.getByRole('button', { name: 'Start new investigation' }).click();
  await investigations.waitForFunction(() => document.getElementById('investigation-select').value.startsWith('native-research-'));
  await investigations.reload();
  await investigations.getByRole('status').filter({ hasText: /Comparison ready\.|Add a producer evidence batch/ }).waitFor();
  assert.equal((await readdir(dataDir)).length, 2);
  assert.equal(await investigations.locator('#investigation-select option').count(), 2);
  const tokenless = await browser.newPage();
  await tokenless.goto(peopleUrl.split('#')[0]);
  await tokenless.getByText('Saved in this browser (IndexedDB). Export a project file to move it to another device.', { exact: true }).waitFor();
  assert.equal((await readdir(dataDir)).length, 2, 'a page without the session token reached local storage');
  assert.deepEqual(errors, []);
  console.log(`Local workbench passed: fixture, policy refusal, upload, download, localization, mobile, session gating, saved investigations${process.argv.includes('--real') ? ', real Qwen3 inference' : ''}.`);
} finally {
  if (browser) await browser.close();
  child.kill('SIGTERM');
  await stopped;
  await rm(dataDir, { recursive: true, force: true });
}
