import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { readFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { chromium } from 'playwright';

const root = resolve(import.meta.dirname, '..');
const child = spawn(resolve(root, 'target/debug', process.platform === 'win32' ? 'belief.exe' : 'belief'), ['workbench'], { cwd: root, stdio: ['ignore', 'pipe', 'pipe'] });
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
  assert.deepEqual(errors, []);
  console.log(`Local workbench passed: fixture, policy refusal, upload, download, localization, mobile, session gating${process.argv.includes('--real') ? ', real Qwen3 inference' : ''}.`);
} finally {
  if (browser) await browser.close();
  child.kill('SIGTERM');
  await stopped;
}
