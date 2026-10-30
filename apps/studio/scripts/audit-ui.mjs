/**
 * Harnais d'audit UX KaractiRage Studio — WS0 / UX-001.
 *
 * Sert le build de production (vite preview), charge chaque écran dans Chrome
 * (headless) avec un mock IPC Tauri (scripts/mock-ipc.js), mesure header/canvas
 * et violations axe-core, puis produit `audit/report.json` + des captures PNG.
 *
 * Usage :
 *   node scripts/audit-ui.mjs [--strict]
 *
 *   --strict : sort en erreur (code 1) si axe détecte des violations
 *              critical/serious (gate CI — voir UX-002 / WS12).
 *   Sans --strict : mesure et rapporte uniquement (reproduction de la baseline).
 */

import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync, readFileSync, existsSync, copyFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const require = createRequire(import.meta.url);
const { PNG } = require('pngjs');

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const AUDIT_DIR = path.join(ROOT, 'audit');
const MOCK_PATH = path.join(__dirname, 'mock-ipc.js');
const AXE_PATH = path.join(ROOT, 'node_modules', 'axe-core', 'axe.min.js');

const PORT = 4173;
const BASE_URL = `http://127.0.0.1:${PORT}/`;
const STRICT = process.argv.includes('--strict');
const UPDATE_BASELINE = process.argv.includes('--update-baseline');
const BASELINE_DIR = path.join(AUDIT_DIR, 'baseline');

const VIEWPORTS = [
  { name: '1440x900', width: 1440, height: 900 },
  { name: '1024x720', width: 1024, height: 720 },
];

// Séquence de navigation linéaire (état persistant dans la page) :
// chaque étape s'exécute après la précédente, depuis l'écran Hub d'accueil.
const STEPS = [
  { id: 'hub', label: 'Hub' },
  { id: 'play', label: 'Play', run: (p) => p.getByTestId('hub').getByLabel('Fermer').click() },
  { id: 'edition', label: 'Édition', run: (p) => p.getByTestId('header-modes').getByText('Créer', { exact: false }).click() },
  { id: 'kvl-recettes', label: 'KVL Recettes', run: (p) => p.getByTestId('header-modes').getByText('Logique', { exact: false }).click() },
  { id: 'kvl-noeuds', label: 'KVL Nœuds', run: (p) => p.getByTestId('kv-palette').getByText('Nœuds', { exact: false }).click() },
  { id: 'pixel', label: 'Pixel Art', run: (p) => p.getByTestId('header-modes').getByText('Pixel', { exact: false }).click() },
  { id: 'vector', label: 'Vectoriel', run: (p) => p.getByTestId('header-modes').getByText('Vectoriel', { exact: false }).click() },
];

// Scénarios de pannes (UX-005) — une capture 1440×900 chacun.
const FAULTS = [
  { id: 'fault-null', label: 'Moteur absent (IPC null)', url: '?fault=null', run: (p) => p.getByTestId('hub').getByLabel('Fermer').click() },
  { id: 'fault-timeout', label: 'Commande lente (timeout)', url: '?fault=timeout', run: (p) => p.getByTestId('hub').getByLabel('Fermer').click() },
  {
    id: 'fault-empty',
    label: 'Catalogue vide',
    url: '?fault=empty',
    run: async (p) => {
      await p.getByTestId('hub').getByLabel('Fermer').click();
      await p.getByTestId('header-modes').getByText('Logique', { exact: false }).click();
      await p.getByTestId('kv-palette').getByText('Nœuds', { exact: false }).click();
    },
  },
];

function runCommand(cmd, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: 'inherit' });
    child.on('close', (code) => (code === 0 ? resolve() : reject(new Error(`${cmd} exit ${code}`))));
    child.on('error', reject);
  });
}

function startPreview() {
  return spawn('npx', ['vite', 'preview', '--port', String(PORT), '--strictPort', '--host', '127.0.0.1'], {
    cwd: ROOT,
    stdio: 'ignore',
  });
}

async function waitForServer(url, timeoutMs = 30000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const res = await fetch(url);
      if (res.ok) return;
    } catch {
      /* pas encore prêt */
    }
    await new Promise((r) => setTimeout(r, 250));
  }
  throw new Error(`Serveur vite preview injoignable sur ${url}`);
}

/** Mesure le header (hauteur, plus petite cible) et le canvas (dimensions/ratio). */
function measure(page) {
  return page.evaluate(() => {
    const header = document.querySelector('[data-testid="header"]');
    const hr = header ? header.getBoundingClientRect() : null;
    const targets = header ? [...header.querySelectorAll('button, [role="button"], a')] : [];
    const heights = targets.map((b) => b.getBoundingClientRect().height).filter((h) => h > 0);
    const canvas = document.querySelector('[data-testid="canvas-viewport"]');
    const cr = canvas ? canvas.getBoundingClientRect() : null;
    return {
      header_height: hr ? Math.round(hr.height) : null,
      header_target_count: heights.length,
      header_target_min: heights.length ? Math.round(Math.min(...heights)) : null,
      canvas: cr && cr.height > 0
        ? { width: Math.round(cr.width), height: Math.round(cr.height), ratio: Number((cr.width / cr.height).toFixed(3)) }
        : null,
    };
  });
}

/** Injecte axe-core (une fois) et renvoie les violations WCAG A/AA. */
async function runAxe(page) {
  const hasAxe = await page.evaluate(() => typeof window.axe !== 'undefined');
  if (!hasAxe) await page.addScriptTag({ path: AXE_PATH });
  return page.evaluate(async () => {
    const res = await window.axe.run(document, {
      runOnly: { type: 'tag', values: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'] },
    });
    return res.violations.map((v) => ({ id: v.id, impact: v.impact, count: v.nodes.length, help: v.help }));
  });
}

/** Pourcentage de pixels différents (>8 par canal) entre deux PNG de même taille. */
function pixelDiffPct(currentFile, baselineFile) {
  const a = PNG.sync.read(readFileSync(currentFile));
  const b = PNG.sync.read(readFileSync(baselineFile));
  if (a.width !== b.width || a.height !== b.height) return 100;
  let diff = 0;
  const total = a.width * a.height;
  for (let i = 0; i < total; i++) {
    const j = i * 4;
    if (Math.abs(a.data[j] - b.data[j]) > 8 || Math.abs(a.data[j + 1] - b.data[j + 1]) > 8 || Math.abs(a.data[j + 2] - b.data[j + 2]) > 8) diff++;
  }
  return Number(((diff / total) * 100).toFixed(3));
}

async function main() {
  const started = Date.now();
  console.log('[audit] build de production…');
  await runCommand('npm', ['run', 'build'], ROOT);

  mkdirSync(AUDIT_DIR, { recursive: true });
  const server = startPreview();

  try {
    await waitForServer(BASE_URL);
    console.log(`[audit] serveur prêt sur ${BASE_URL}`);

    const browser = await chromium.launch({ channel: 'chrome', headless: true });
    const results = [];

    try {
      for (const vp of VIEWPORTS) {
        const page = await browser.newPage({ viewport: { width: vp.width, height: vp.height } });
        await page.addInitScript({ path: MOCK_PATH });
        await page.goto(BASE_URL, { waitUntil: 'domcontentloaded' });
        await page.getByTestId('hub').waitFor({ state: 'visible', timeout: 15000 });

        for (const step of STEPS) {
          if (step.run) {
            await step.run(page);
            await page.waitForTimeout(600);
          } else {
            // Hub : laisser le boot (mock ready) et le rendu se stabiliser.
            await page.waitForTimeout(700);
          }

          const metrics = await measure(page);
          const axe = await runAxe(page);
          const shot = path.join(AUDIT_DIR, `${step.id}-${vp.name}.png`);
          await page.screenshot({ path: shot, fullPage: false });

          const serious = axe.filter((v) => v.impact === 'critical' || v.impact === 'serious').reduce((n, v) => n + v.count, 0);
          results.push({
            scenario: step.id,
            label: step.label,
            viewport: vp.name,
            ...metrics,
            axe,
            screenshot: path.relative(ROOT, shot),
          });
          console.log(
            `[audit] ${step.id} @${vp.name} header=${metrics.header_height ?? '—'}px cible_min=${metrics.header_target_min ?? '—'}px canvas_ratio=${metrics.canvas ? metrics.canvas.ratio : '—'} axe_crit/serious=${serious}`,
          );
        }
        await page.close();
      }

      // Scénarios de pannes (UX-005) — une page par panne, 1440×900.
      for (const fault of FAULTS) {
        const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
        await page.addInitScript({ path: MOCK_PATH });
        await page.goto(BASE_URL + fault.url, { waitUntil: 'domcontentloaded' });
        await page.getByTestId('hub').waitFor({ state: 'visible', timeout: 15000 });
        if (fault.run) {
          await fault.run(page);
          await page.waitForTimeout(700);
        }
        const metrics = await measure(page);
        const axe = await runAxe(page);
        const shot = path.join(AUDIT_DIR, `${fault.id}-1440x900.png`);
        await page.screenshot({ path: shot, fullPage: false });
        const serious = axe.filter((v) => v.impact === 'critical' || v.impact === 'serious').reduce((n, v) => n + v.count, 0);
        results.push({ scenario: fault.id, label: fault.label, viewport: '1440x900', ...metrics, axe, screenshot: path.relative(ROOT, shot) });
        console.log(`[audit] ${fault.id} @1440x900 axe_crit/serious=${serious}`);
        await page.close();
      }
    } finally {
      await browser.close();
    }

    // Baseline visuelle (UX-003) : mise à jour ou comparaison ≤ 0,5 %.
    if (UPDATE_BASELINE) {
      mkdirSync(BASELINE_DIR, { recursive: true });
      for (const r of results) {
        copyFileSync(path.join(ROOT, r.screenshot), path.join(BASELINE_DIR, path.basename(r.screenshot)));
      }
      console.log(`[audit] baseline mise à jour : ${results.length} captures dans audit/baseline/`);
    } else {
      let overThreshold = 0;
      for (const r of results) {
        const b = path.join(BASELINE_DIR, path.basename(r.screenshot));
        if (existsSync(b)) {
          r.baseline_diff_pct = pixelDiffPct(path.join(ROOT, r.screenshot), b);
          if (r.baseline_diff_pct > 0.5) overThreshold++;
        }
      }
      if (overThreshold > 0) console.warn(`[audit] ${overThreshold} capture(s) au-dessus du seuil 0,5 % vs baseline.`);
    }

    const report = {
      generated_at: new Date().toISOString(),
      duration_ms: Date.now() - started,
      scenarios: results,
    };
    writeFileSync(path.join(AUDIT_DIR, 'report.json'), JSON.stringify(report, null, 2));

    const serious = results.reduce(
      (n, r) => n + r.axe.filter((v) => v.impact === 'critical' || v.impact === 'serious').reduce((m, v) => m + v.count, 0),
      0,
    );
    console.log(`[audit] terminé en ${report.duration_ms} ms — ${results.length} captures, ${serious} violation(s) critical/serious (axe).`);
    if (STRICT && serious > 0) {
      console.error('[audit] --strict : échec (violations critical/serious détectées).');
      process.exitCode = 1;
    }
  } finally {
    server.kill('SIGTERM');
  }
}

main().catch((e) => {
  console.error('[audit] erreur :', e);
  process.exitCode = 1;
});
