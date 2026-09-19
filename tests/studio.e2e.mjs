import assert from 'node:assert/strict';
import { mkdir } from 'node:fs/promises';
import { chromium, expect } from '@playwright/test';

const base = process.env.DAIW_URL ?? 'http://127.0.0.1:3000';
const evidence = process.env.DAIW_EVIDENCE_DIR ?? '.playwright-mcp/final';
await mkdir(evidence, { recursive: true });
const browser = process.env.DAIW_CDP
  ? await chromium.connectOverCDP(process.env.DAIW_CDP)
  : await chromium.launch({ headless: true });
const context = await browser.newContext({ viewport: { width: 1440, height: 960 }, acceptDownloads: true });
const page = await context.newPage();
const errors = [];
page.on('pageerror', (error) => errors.push(error.message));
const name = `Studio QA ${Date.now()}`;

try {
  await page.goto(base);
  await page.locator('button.project-title').click();
  const dialog = page.getByRole('dialog');
  await dialog.getByRole('textbox', { name: 'New project name', exact: true }).fill(name);
  await dialog.getByRole('button', { name: 'Create project', exact: true }).click();
  await expect(page.getByRole('banner')).toContainText(name);
  await page.getByRole('button', { name: 'Close projects', exact: true }).click();

  const main = page.locator('main');
  await expect(page.getByRole('button', { name: /Final Chorus, starts/ })).toBeVisible();
  await expect(page.getByRole('tab', { name: 'This instance', exact: true })).toHaveAttribute('aria-selected', 'true');
  const grid = page.getByRole('button', { name: /notes\. Click or tap the grid/ });
  await expect(grid).toBeEnabled();
  await expect(grid).toHaveAttribute('aria-label', /^4 notes\./);
  await grid.click({ position: { x: 40, y: 10 } });
  await expect(grid).toHaveAttribute('aria-label', /^5 notes\./);
  await expect(grid).toBeEnabled();
  await grid.click({ position: { x: 90, y: 10 } });
  await expect(grid).toHaveAttribute('aria-label', /^6 notes\./);
  await page.getByRole('button', { name: /Chorus II, starts/ }).click();
  await expect(grid).toHaveAttribute('aria-label', /^4 notes\./);
  await page.getByRole('button', { name: /Chorus I, starts/ }).click();
  await expect(grid).toHaveAttribute('aria-label', /^6 notes\./);
  await page.screenshot({ path: `${evidence}/arrangement-desktop.png`, fullPage: true });

  const tempo = page.getByRole('spinbutton', { name: 'TEMPO BPM', exact: true });
  await tempo.fill('108');
  await tempo.press('Enter');
  await expect(tempo).toHaveValue('108');
  await page.getByRole('button', { name: 'Undo last change', exact: true }).click();
  await expect(tempo).toHaveValue('112');
  await page.getByRole('button', { name: 'Redo change', exact: true }).click();
  await expect(tempo).toHaveValue('108');

  await page.getByRole('button', { name: 'Review', exact: true }).click();
  await page.getByRole('textbox', { name: /Change Chorus/ }).fill('An octave up');
  await page.getByRole('button', { name: 'Create proposal', exact: true }).click();
  await expect(main.locator('article')).toContainText('Transpose notes');
  await page.getByRole('textbox', { name: /Change Chorus/ }).fill('An octave up and softer');
  await page.getByRole('button', { name: 'Create proposal', exact: true }).click();
  const proposal = main.locator('article');
  await expect(proposal).toContainText('Scale note velocities');
  await proposal.getByRole('button', { name: 'Prepare A/B', exact: true }).click();
  await expect(proposal.getByRole('button', { name: 'Listen B', exact: true })).toBeEnabled({ timeout: 30000 });
  await page.getByRole('button', { name: 'Start playback', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Pause playback', exact: true })).toBeVisible();
  await proposal.getByRole('button', { name: 'Listen B', exact: true }).click();
  await proposal.getByRole('checkbox', { name: 'Loudness match', exact: true }).check();
  await page.getByRole('button', { name: 'Stop playback', exact: true }).click();
  await proposal.getByRole('checkbox', { name: /Change 1.*Transpose/ }).check();
  await proposal.getByRole('button', { name: 'Approve 1', exact: true }).click();
  await expect(proposal).toContainText(/partially\s?accepted/i);
  await page.screenshot({ path: `${evidence}/review-desktop.png`, fullPage: true });

  await page.getByRole('button', { name: 'Mixer', exact: true }).click();
  const fader = page.locator('input[type="range"]').first();
  const nextGain = Number(await fader.inputValue()) + 0.5;
  const [faderResponse] = await Promise.all([
    page.waitForResponse(async (response) => response.url().endsWith('/api/rpc') && (await response.json()).state?.tracks?.[0]?.gain === nextGain),
    fader.press('ArrowUp'),
  ]);
  assert.equal(faderResponse.status(), 200, 'Mixer keyboard edit commits a typed track command');
  await page.getByRole('button', { name: 'Start playback', exact: true }).click();
  await expect(page.locator('.master-strip output')).not.toContainText('∞', { timeout: 30000 });
  await page.screenshot({ path: `${evidence}/mixer-desktop.png`, fullPage: true });
  await page.getByRole('button', { name: 'Stop playback', exact: true }).click();

  await page.locator('button.project-title').click();
  await expect(dialog).toContainText('Valid canonical project');
  await page.screenshot({ path: `${evidence}/projects-desktop.png`, fullPage: true });
  await page.keyboard.press('Escape');
  await expect(dialog).not.toBeVisible();
  await page.reload();
  await expect(page.getByRole('banner')).toContainText(name);
  await expect(page.getByRole('spinbutton', { name: 'TEMPO BPM', exact: true })).toHaveValue('108');

  for (const width of [1280, 768, 375]) {
    await page.setViewportSize({ width, height: 900 });
    await page.getByRole('button', { name: 'Arrange', exact: true }).click();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true, `Body overflow at ${width}px`);
    await page.screenshot({ path: `${evidence}/arrangement-${width}.png`, fullPage: true });
    await page.getByRole('button', { name: 'Review', exact: true }).click();
    await page.screenshot({ path: `${evidence}/review-${width}.png`, fullPage: true });
    await page.getByRole('button', { name: 'Mixer', exact: true }).click();
    await page.screenshot({ path: `${evidence}/mixer-${width}.png`, fullPage: true });
    await page.locator('button.project-title').click();
    await expect(dialog).toContainText('Valid canonical project');
    await page.screenshot({ path: `${evidence}/projects-${width}.png`, fullPage: true });
    await page.keyboard.press('Escape');
  }
  assert.deepEqual(errors, [], 'No uncaught browser exceptions');
  process.stdout.write(`Browser workflows passed: create, edit, undo/redo, proposal, A/B, partial approval, metering, health, reload, 4 responsive widths. Evidence: ${evidence}\n`);
} finally {
  await context.close();
  await browser.close();
}
