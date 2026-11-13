#!/usr/bin/env node
/**
 * Test de fumée navigateur d'un export HTML5 autonome KaractiRage — P1.5.
 *
 * Le Studio produit un export HTML5 « standalone » : un fichier unique qui
 * embarque le paquet du jeu dans
 *   <script id="embedded-package" type="application/json">…</script>
 * puis un <script type="module"> qui initialise un <canvas>.
 *
 * Ce harnais ouvre ce fichier dans Chrome/Chromium headless via `file://` et
 * vérifie qu'il s'exécute VRAIMENT hors ligne, sans erreur :
 *
 *   - le fichier existe et se charge ;
 *   - aucune erreur console (`console.error`) n'apparaît ;
 *   - aucune `pageerror` (exception non rattrapée) n'est levée ;
 *   - aucune requête réseau non-`file://` n'est émise (zéro appel CDN) ;
 *   - le script `#embedded-package` est présent, son JSON est valide et
 *     contient au moins une entité ;
 *   - au moins un `<canvas>` est présent dans le document.
 *
 * Avec `--expect-runtime`, le test va plus loin et vérifie que le jeu TOURNE :
 *
 *   - `window.__karacti_runtime` vaut `'running'` (attente jusqu'à 20 s) ;
 *   - l'état est explicitement `'absent'` (pas de runtime embarqué) ou
 *     `'error:<message>'` (échec de démarrage) sinon, avec un message clair ;
 *   - le canvas contient des pixels : plus de 1000 pixels non noirs, ou plus
 *     de 20 couleurs distinctes (échantillonnage `ctx.getImageData`).
 *
 * Avec `--expect-input`, le test prouve que le jeu est réellement JOUABLE :
 *
 *   - après `runtime=running`, valide le menu par `Entrée` (sans effet sur un
 *     export mono-scène déjà en jeu), puis capture le canvas ;
 *   - envoie `ArrowRight` (`keydown` sur `window`, maintenu ~800 ms) ;
 *   - attend ~1 s après le `keyup`, recapture, et exige une variation
 *     significative (> 2000 pixels modifiés) : le joueur a bougé.
 *   L'option implique `--expect-runtime` ; sans runtime, l'échec est propre.
 *
 * Avec `--expect-levels <n>`, le test vérifie que l'export embarque exactement
 * `n` niveaux (`window.__karacti_levels`, posé par le template après l'import
 * du paquet). L'option implique `--expect-runtime`.
 *
 * Usage :
 *   node scripts/export-smoke.mjs <chemin-vers-html> [--json] [--timeout <ms>]
 *                                 [--expect-runtime] [--expect-input]
 *                                 [--expect-levels <n>]
 *   node scripts/export-smoke.mjs --help
 *
 * Options :
 *   --json           sort un objet JSON machine (rien d'autre sur stdout) :
 *                    { ok, entities, consoleErrors, pageErrors,
 *                      networkRequests, runtime, expectedRuntime, levels,
 *                      expectedLevels, nonBlackPixels, canvasColors,
 *                      expectedInput, inputDelta, inputPixelsBefore,
 *                      inputPixelsAfter, problems }
 *   --timeout <ms>   délai global (défaut : 30 000 ms), pour ne jamais
 *                    suspendre un CI.
 *   --expect-runtime exige que le runtime WASM embarqué démarre et rende des
 *                    pixels (sinon échec propre, sans crash).
 *   --expect-input   exige que le clavier pilote le moteur : variation de
 *                    pixels > 2000 après un appui simulé sur `ArrowRight`.
 *   --expect-levels <n> exige que l'export embarque exactement `n` niveaux.
 *   -h, --help       affiche cette aide.
 *
 * Codes de sortie :
 *   0  export conforme (smoke test vert)
 *   1  export défectueux (erreurs console, réseau, paquet ou canvas invalide)
 *   2  usage incorrect, fichier introuvable, ou navigateur indisponible
 *
 * Dépendances : uniquement `playwright-core` (déjà présent dans apps/studio,
 * même motif de lancement que scripts/audit-ui.mjs). Aucune dépendance ajoutée.
 */

import { chromium } from 'playwright-core';
import { existsSync, statSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

/** Délai global par défaut : 30 s (borne dure, jamais de CI suspendu). */
const DEFAULT_TIMEOUT_MS = 30_000;

/** Attente maximale du démarrage du runtime embarqué (--expect-runtime). */
const RUNTIME_TIMEOUT_MS = 20_000;

/** Seuils de contenu du canvas : plus de 1000 pixels non noirs… */
const PIXELS_MIN_NON_BLACK = 1000;

/** …ou plus de 20 couleurs distinctes. */
const COLORS_MIN_DISTINCT = 20;

/** Variation minimale de pixels pour prouver qu'une entrée agit (--expect-input). */
const INPUT_DIFF_MIN_PIXELS = 2000;

/** Durée d'appui simulée sur `ArrowRight` (ms). */
const INPUT_KEY_HOLD_MS = 800;

/** Attente après `Entrée` (menu → jeu) avant la capture initiale (ms). */
const INPUT_MENU_SETTLE_MS = 500;

/** Délai après le relâchement avant la recapture (ms). */
const INPUT_SETTLE_MS = 1000;

/** Schémas d'URL qui ne constituent PAS une requête réseau. */
const LOCAL_PROTOCOLS = ['file:', 'data:', 'blob:', 'about:', 'chrome:', 'devtools:'];

const USAGE = `Usage : node scripts/export-smoke.mjs <chemin-vers-html> [options]

Test de fumée d'un export HTML5 autonome KaractiRage (P1.5) : charge le fichier
en file:// dans un Chrome headless et vérifie qu'il tourne hors ligne, sans
erreur console, avec un paquet embarqué valide et un canvas.

Options :
  --json           sortie JSON machine { ok, entities, consoleErrors, pageErrors,
                   networkRequests, runtime, expectedRuntime, levels,
                   expectedLevels, nonBlackPixels, canvasColors, expectedInput,
                   inputDelta, inputPixelsBefore, inputPixelsAfter, problems }
  --timeout <ms>   délai global en millisecondes (défaut : ${DEFAULT_TIMEOUT_MS})
  --expect-runtime exige que le runtime WASM embarqué démarre (runtime=running)
                   et rende des pixels dans le canvas
  --expect-input   exige que le clavier pilote le moteur : appuie sur ArrowRight
                   et vérifie que plus de ${INPUT_DIFF_MIN_PIXELS} pixels changent
  --expect-levels <n> exige que l'export embarque exactement n niveaux
                   (window.__karacti_levels) ; implique --expect-runtime
  -h, --help       affiche cette aide

Codes de sortie :
  0  export conforme
  1  export défectueux (erreurs console, requêtes réseau, paquet, canvas…)
  2  usage incorrect, fichier introuvable, ou navigateur indisponible

Exemples :
  npm run export:smoke -- ./jeu-standalone.html
  node scripts/export-smoke.mjs ./jeu-standalone.html --expect-runtime
  node scripts/export-smoke.mjs ./jeu-standalone.html --expect-runtime --expect-input
  node scripts/export-smoke.mjs ./jeu-standalone.html --expect-runtime --expect-levels 3
  node scripts/export-smoke.mjs ./jeu-standalone.html --json
`;

const argv = process.argv.slice(2);

/** Analyse très simple des arguments (aucune lib de CLI : script autonome). */
function parseArgs(args) {
  const opts = {
    file: null,
    json: false,
    help: false,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    expectRuntime: false,
    expectInput: false,
    expectLevels: null,
  };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === '--json') {
      opts.json = true;
    } else if (arg === '--expect-runtime') {
      opts.expectRuntime = true;
    } else if (arg === '--expect-input') {
      opts.expectInput = true;
    } else if (arg === '--expect-levels') {
      const raw = args[++i];
      const n = Number(raw);
      if (!Number.isInteger(n) || n < 1) {
        return { error: `valeur invalide pour --expect-levels : ${raw ?? '(absente)'} (entier >= 1 attendu)` };
      }
      opts.expectLevels = n;
    } else if (arg.startsWith('--expect-levels=')) {
      const raw = arg.slice('--expect-levels='.length);
      const n = Number(raw);
      if (!Number.isInteger(n) || n < 1) {
        return { error: `valeur invalide pour --expect-levels : ${raw} (entier >= 1 attendu)` };
      }
      opts.expectLevels = n;
    } else if (arg === '-h' || arg === '--help') {
      opts.help = true;
    } else if (arg === '--timeout') {
      const raw = args[++i];
      const ms = Number(raw);
      if (!Number.isFinite(ms) || ms <= 0) return { error: `valeur invalide pour --timeout : ${raw ?? '(absente)'}` };
      opts.timeoutMs = ms;
    } else if (arg.startsWith('--timeout=')) {
      const raw = arg.slice('--timeout='.length);
      const ms = Number(raw);
      if (!Number.isFinite(ms) || ms <= 0) return { error: `valeur invalide pour --timeout : ${raw}` };
      opts.timeoutMs = ms;
    } else if (arg.startsWith('-')) {
      return { error: `option inconnue : ${arg}` };
    } else if (opts.file === null) {
      opts.file = arg;
    } else {
      return { error: `argument superflu : ${arg}` };
    }
  }
  return opts;
}

/** Vrai si l'URL est locale (file:, data:, blob:…) et donc hors « réseau ». */
function isLocalUrl(url) {
  try {
    const protocol = new URL(url).protocol;
    return LOCAL_PROTOCOLS.includes(protocol);
  } catch {
    return false;
  }
}

/** Sortie d'erreur d'usage / d'environnement : message clair + exit 2. */
function usageError(message, json, expectRuntime = false, expectInput = false, expectLevels = null) {
  if (json) {
    const payload = {
      ...emptyResult(),
      expectedRuntime: expectRuntime,
      expectedInput: expectInput,
      expectedLevels: expectLevels,
      problems: [message],
    };
    process.stdout.write(JSON.stringify(payload, null, 2) + '\n');
  } else {
    console.error(`[export-smoke] ${message}`);
    console.error('[export-smoke] Voir : node scripts/export-smoke.mjs --help');
  }
  process.exit(2);
}

/**
 * Résultat vide : structure unique utilisée pour toutes les sorties JSON,
 * y compris en cas d'échec précoce.
 *
 * `runtime` vaut `'running' | 'absent' | 'error'` (ou `null` si l'état n'a pas
 * pu être observé), `nonBlackPixels`/`canvasColors` restent `null` quand le
 * canvas n'a pas été échantillonné (sans `--expect-runtime`).
 */
function emptyResult() {
  return {
    ok: false,
    entities: null,
    consoleErrors: [],
    pageErrors: [],
    networkRequests: [],
    runtime: null,
    expectedRuntime: false,
    levels: null,
    expectedLevels: null,
    nonBlackPixels: null,
    canvasColors: null,
    expectedInput: false,
    inputDelta: null,
    inputPixelsBefore: null,
    inputPixelsAfter: null,
    problems: [],
  };
}

/** Lance Chrome (canal stable) avec repli sur le Chromium fourni par Playwright. */
async function launchBrowser() {
  const attempts = [
    // Même motif que scripts/audit-ui.mjs : Chrome installé, headless.
    { channel: 'chrome', headless: true },
    // Repli : Chromium téléchargé par Playwright (utile en CI Linux).
    { headless: true },
  ];
  const failures = [];
  for (const options of attempts) {
    try {
      return await chromium.launch(options);
    } catch (err) {
      failures.push(`${options.channel ?? 'chromium'}: ${String(err?.message ?? err).split('\n')[0]}`);
    }
  }
  throw new Error(`aucun navigateur Chrome/Chromium n'a pu être lancé (${failures.join(' | ')})`);
}

/**
 * Lit `window.__karacti_runtime` sans échouer si la page n'est pas prête.
 * Renvoie la chaîne brute (`'running'`, `'absent'`, `'error:…'`) ou `null`.
 */
async function readRuntimeState(page) {
  return await page.evaluate(() =>
    typeof window.__karacti_runtime === 'string' ? window.__karacti_runtime : null,
  );
}

/**
 * Lit `window.__karacti_levels` (nombre de scènes embarquées), posé par le
 * template après un import de paquet réussi. `null` si absent ou invalide.
 */
async function readLevels(page) {
  return await page.evaluate(() =>
    Number.isInteger(window.__karacti_levels) ? window.__karacti_levels : null,
  );
}

/**
 * Attend que le runtime embarqué atteigne un état terminal.
 *
 * Renvoie `{ state: 'running' | 'absent' | 'error' | 'pending', raw, detail }`.
 * `pending` = ni démarré ni en erreur à l'échéance (timeout propre, pas un
 * crash) : le message d'échec du test dira lequel des deux manque.
 */
async function waitForRuntime(page, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let raw = null;
  let detail = null;
  for (;;) {
    try {
      raw = await readRuntimeState(page);
      detail = null;
    } catch (err) {
      // La page peut se fermer ou se figer : on ne masque pas la cause.
      raw = null;
      detail = String(err?.message ?? err).split('\n')[0];
    }
    if (raw === 'running') return { state: 'running', raw, detail: null };
    if (raw && raw.startsWith('error')) return { state: 'error', raw, detail: null };
    if (raw === 'absent') return { state: 'absent', raw, detail: null };
    if (Date.now() >= deadline) return { state: 'pending', raw, detail };
    await page.waitForTimeout(100);
  }
}

/**
 * Échantillonne le canvas : nombre de pixels non noirs et de couleurs
 * distinctes, via `ctx.getImageData` (jamais via le framebuffer wasm, pour
 * prouver que le rendu est bien recopié dans le canvas).
 */
async function sampleCanvas(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ok: false, reason: 'aucun canvas dans le document' };
    const ctx = canvas.getContext('2d');
    if (!ctx) return { ok: false, reason: 'contexte 2D indisponible' };
    const width = canvas.width;
    const height = canvas.height;
    if (!width || !height) return { ok: false, reason: `canvas de taille nulle (${width}x${height})` };
    let data;
    try {
      data = ctx.getImageData(0, 0, width, height).data;
    } catch (err) {
      return { ok: false, reason: `getImageData impossible : ${String(err?.message ?? err)}` };
    }
    let nonBlack = 0;
    const colors = new Set();
    for (let i = 0; i < data.length; i += 4) {
      const r = data[i];
      const g = data[i + 1];
      const b = data[i + 2];
      if (r !== 0 || g !== 0 || b !== 0) nonBlack++;
      // Plafond volontaire : au-delà du seuil, le nombre exact de couleurs
      // n'apporte plus rien et coûterait de la mémoire.
      if (colors.size <= 64) colors.add((r << 16) | (g << 8) | b);
    }
    return { ok: true, width, height, nonBlack, colors: colors.size };
  });
}

/**
 * Attend que le canvas ait du contenu (> 1000 pixels non noirs OU > 20
 * couleurs distinctes). Le rendu peut arriver quelques frames après
 * `running` : on ré-échantillonne jusqu'à l'échéance.
 */
async function waitForCanvasContent(page, timeoutMs) {
  const deadline = Date.now() + timeoutMs;
  let last = { ok: false, reason: 'canvas jamais échantillonné' };
  for (;;) {
    last = await sampleCanvas(page);
    if (last.ok && (last.nonBlack > PIXELS_MIN_NON_BLACK || last.colors > COLORS_MIN_DISTINCT)) {
      return last;
    }
    if (Date.now() >= deadline) return last;
    await page.waitForTimeout(100);
  }
}

/**
 * Capture l'image complète du canvas et la conserve DANS la page
 * (`window.__karacti_input_before`) : inutile de transférer 3,84 Mo de pixels
 * par le protocole Playwright avant/après l'entrée.
 */
async function captureInputSnapshot(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (!canvas) return { ok: false, reason: 'aucun canvas dans le document' };
    const ctx = canvas.getContext('2d');
    if (!ctx) return { ok: false, reason: 'contexte 2D indisponible' };
    if (!canvas.width || !canvas.height) {
      return { ok: false, reason: `canvas de taille nulle (${canvas.width}x${canvas.height})` };
    }
    try {
      const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      window.__karacti_input_before = data;
      let nonBlack = 0;
      for (let i = 0; i < data.length; i += 4) {
        if (data[i] !== 0 || data[i + 1] !== 0 || data[i + 2] !== 0) nonBlack++;
      }
      return { ok: true, width: canvas.width, height: canvas.height, nonBlack };
    } catch (err) {
      return { ok: false, reason: `getImageData impossible : ${String(err?.message ?? err)}` };
    }
  });
}

/**
 * Envoie `Entrée` (`keydown` puis `keyup`) : sur un export multi-niveaux, le
 * joueur démarre au menu et cette touche lance le niveau 1. Sur un export
 * mono-scène, le jeu est déjà en cours et `Entrée` est sans effet d'écran.
 */
async function pressEnter(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (canvas && typeof canvas.focus === 'function') canvas.focus();
    const down = new KeyboardEvent('keydown', {
      code: 'Enter',
      key: 'Enter',
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(down);
    const up = new KeyboardEvent('keyup', {
      code: 'Enter',
      key: 'Enter',
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(up);
    return { defaultPrevented: down.defaultPrevented };
  });
}

/**
 * Focus le canvas puis envoie un `keydown` `ArrowRight` sur `window`, comme le
 * ferait un vrai clavier. `defaultPrevented` permet de constater que le
 * template bloque bien le défilement de la page.
 */
async function pressArrowRight(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    if (canvas && typeof canvas.focus === 'function') canvas.focus();
    const event = new KeyboardEvent('keydown', {
      code: 'ArrowRight',
      key: 'ArrowRight',
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(event);
    return { defaultPrevented: event.defaultPrevented };
  });
}

/** Envoie le `keyup` correspondant, pour arrêter le déplacement du joueur. */
async function releaseArrowRight(page) {
  return await page.evaluate(() => {
    const event = new KeyboardEvent('keyup', {
      code: 'ArrowRight',
      key: 'ArrowRight',
      bubbles: true,
      cancelable: true,
    });
    window.dispatchEvent(event);
    return true;
  });
}

/**
 * Recapture le canvas et compare à l'instantané mémorisé dans la page :
 * nombre de pixels modifiés (canal RVBA) et pixels non noirs après coup.
 */
async function diffInputSnapshot(page) {
  return await page.evaluate(() => {
    const canvas = document.querySelector('canvas');
    const before = window.__karacti_input_before;
    if (!canvas) return { ok: false, reason: 'aucun canvas dans le document' };
    if (!before) return { ok: false, reason: 'instantané initial absent' };
    const ctx = canvas.getContext('2d');
    if (!ctx) return { ok: false, reason: 'contexte 2D indisponible' };
    try {
      const after = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
      if (after.length !== before.length) {
        return { ok: false, reason: `taille du canvas changée (${before.length} → ${after.length} octets)` };
      }
      let delta = 0;
      let afterNonBlack = 0;
      for (let i = 0; i < after.length; i += 4) {
        if (
          after[i] !== before[i] ||
          after[i + 1] !== before[i + 1] ||
          after[i + 2] !== before[i + 2] ||
          after[i + 3] !== before[i + 3]
        ) {
          delta++;
        }
        if (after[i] !== 0 || after[i + 1] !== 0 || after[i + 2] !== 0) afterNonBlack++;
      }
      delete window.__karacti_input_before;
      return { ok: true, delta, afterNonBlack };
    } catch (err) {
      return { ok: false, reason: `getImageData impossible : ${String(err?.message ?? err)}` };
    }
  });
}

/**
 * Sonde d'entrée : capture, appuie sur ArrowRight ~800 ms, relâche, attend 1 s,
 * recapture et renvoie la variation mesurée. Toute erreur devient un problème
 * explicite (jamais un crash du harnais).
 */
async function runInputProbe(page, startedAt, timeoutMs) {
  const problems = [];
  const remaining = timeoutMs - (Date.now() - startedAt);
  if (remaining < INPUT_KEY_HOLD_MS + INPUT_SETTLE_MS + 1_500) {
    problems.push(
      `budget insuffisant pour --expect-input : ${remaining} ms restants, ` +
        `il en faut au moins ${INPUT_KEY_HOLD_MS + INPUT_SETTLE_MS + 1_500} (augmenter --timeout)`,
    );
    return { problems, delta: null, beforeNonBlack: null, afterNonBlack: null, defaultPrevented: null };
  }

  // Quitte l'écran Menu avant de mesurer le déplacement : un export
  // multi-niveaux démarre sur le menu, où le monde est figé. Sans `Entrée`,
  // ArrowRight ne prouverait rien. Sur un export mono-scène, c'est un no-op.
  try {
    await pressEnter(page);
  } catch (err) {
    problems.push(`--expect-input : envoi de Entrée impossible — ${String(err?.message ?? err).split('\n')[0]}`);
  }
  await page.waitForTimeout(INPUT_MENU_SETTLE_MS);

  const before = await captureInputSnapshot(page);
  if (!before.ok) {
    problems.push(`--expect-input : capture initiale impossible — ${before.reason}`);
    return { problems, delta: null, beforeNonBlack: null, afterNonBlack: null, defaultPrevented: null };
  }

  let defaultPrevented = null;
  try {
    const pressed = await pressArrowRight(page);
    defaultPrevented = pressed.defaultPrevented;
  } catch (err) {
    problems.push(`--expect-input : envoi de keydown impossible — ${String(err?.message ?? err).split('\n')[0]}`);
  }

  await page.waitForTimeout(INPUT_KEY_HOLD_MS);

  try {
    await releaseArrowRight(page);
  } catch (err) {
    problems.push(`--expect-input : envoi de keyup impossible — ${String(err?.message ?? err).split('\n')[0]}`);
  }

  await page.waitForTimeout(INPUT_SETTLE_MS);

  const after = await diffInputSnapshot(page);
  if (!after.ok) {
    problems.push(`--expect-input : comparaison impossible — ${after.reason}`);
    return {
      problems,
      delta: null,
      beforeNonBlack: before.nonBlack,
      afterNonBlack: null,
      defaultPrevented,
    };
  }

  if (after.delta <= INPUT_DIFF_MIN_PIXELS) {
    problems.push(
      `--expect-input : le clavier ne pilote pas le moteur — seulement ${after.delta} pixel(s) ` +
        `modifié(s) après ArrowRight (seuil : > ${INPUT_DIFF_MIN_PIXELS})`,
    );
  }

  return {
    problems,
    delta: after.delta,
    beforeNonBlack: before.nonBlack,
    afterNonBlack: after.afterNonBlack,
    defaultPrevented,
  };
}

/** Exécute le test de fumée. Renvoie l'objet résultat + un code de sortie. */
async function runSmoke(filePath, timeoutMs, expectRuntime = false, expectInput = false, expectLevels = null) {
  const result = emptyResult();
  result.expectedRuntime = expectRuntime;
  result.expectedInput = expectInput;
  result.expectedLevels = expectLevels;
  const url = pathToFileURL(filePath).href;
  const startedAt = Date.now();
  let browser = null;

  // Borne dure : si une étape Playwright se bloque, on coupe le CI nous-mêmes.
  const killer = setTimeout(() => {
    const message = `délai global de ${timeoutMs} ms dépassé (fichier ou navigateur bloqué)`;
    result.problems.push(message);
    if (JSON_OUTPUT) {
      process.stdout.write(JSON.stringify({ ...result, ok: false }, null, 2) + '\n');
    } else {
      console.error(`[export-smoke] ÉCHEC — ${message}`);
    }
    // process.exit coupe aussi Chromium : on ne laisse jamais un process pendant.
    process.exit(1);
  }, timeoutMs);

  try {
    browser = await launchBrowser();
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    page.setDefaultTimeout(Math.max(1_000, Math.min(15_000, Math.floor(timeoutMs / 2))));

    // Collecte des signaux d'échec, branchée AVANT la navigation.
    page.on('console', (msg) => {
      if (msg.type() === 'error') result.consoleErrors.push(msg.text());
    });
    page.on('pageerror', (err) => {
      result.pageErrors.push(String(err?.message ?? err).split('\n')[0]);
    });
    page.on('request', (req) => {
      const reqUrl = req.url();
      if (!isLocalUrl(reqUrl) && !result.networkRequests.includes(reqUrl)) result.networkRequests.push(reqUrl);
    });

    let loaded = false;
    try {
      await page.goto(url, { waitUntil: 'load', timeout: Math.min(15_000, timeoutMs) });
      loaded = true;
    } catch (err) {
      result.problems.push(`chargement impossible de ${url} : ${String(err?.message ?? err).split('\n')[0]}`);
    }

    if (loaded) {
      // Laisse le module d'initialisation démarrer (canvas, paquet, runtime)
      // et les éventuelles erreurs/requêtes tardives se manifester.
      await page.waitForTimeout(500);

      // Instantané DOM : paquet embarqué, validité JSON, nombre d'entités, canvas.
      // Toute erreur d'analyse est un échec du test (exit 1), pas une panne d'outil.
      let snapshot = null;
      try {
        snapshot = await page.evaluate(() => {
          const tag = document.getElementById('embedded-package');
          const canvas = document.querySelector('canvas');
          const snap = {
            hasEmbedded: Boolean(tag),
            embeddedEmpty: false,
            jsonOk: false,
            jsonError: null,
            entities: null,
            entitiesFieldValid: false,
            hasCanvas: Boolean(canvas),
          };
          if (!tag) return snap;
          const raw = (tag.textContent ?? '').trim();
          if (raw.length === 0) {
            snap.embeddedEmpty = true;
            return snap;
          }
          let pkg;
          try {
            pkg = JSON.parse(raw);
          } catch (err) {
            snap.jsonError = String(err?.message ?? err);
            return snap;
          }
          snap.jsonOk = true;
          if (Array.isArray(pkg?.entities)) {
            snap.entitiesFieldValid = true;
            snap.entities = pkg.entities.length;
          }
          return snap;
        });
      } catch (err) {
        result.problems.push(`analyse du DOM impossible : ${String(err?.message ?? err).split('\n')[0]}`);
      }

      // --- Contrôles structurels -------------------------------------------------
      if (snapshot) {
        if (!snapshot.hasEmbedded) {
          result.problems.push('script #embedded-package absent : le paquet du jeu n\'est pas embarqué');
        } else if (snapshot.embeddedEmpty) {
          result.problems.push('script #embedded-package vide : aucun JSON à analyser');
        } else if (!snapshot.jsonOk) {
          result.problems.push(`JSON de #embedded-package invalide : ${snapshot.jsonError}`);
        } else if (!snapshot.entitiesFieldValid) {
          result.problems.push('paquet embarqué sans champ "entities" (tableau attendu)');
        } else {
          result.entities = snapshot.entities;
          if (snapshot.entities === 0) result.problems.push('paquet embarqué sans entités (entities = 0)');
        }

        if (!snapshot.hasCanvas) {
          result.problems.push('aucun <canvas> dans le document : le jeu ne peut pas s\'afficher');
        }
      }

      // --- Runtime WASM embarqué (uniquement avec --expect-runtime) -----------
      // Sans l'option, le comportement historique est conservé : on ne juge pas
      // le démarrage du moteur, on se contente de constater son état.
      if (expectRuntime) {
        const budgetRuntime = Math.max(
          2_000,
          Math.min(RUNTIME_TIMEOUT_MS, timeoutMs - (Date.now() - startedAt) - 1_500),
        );
        const observation = await waitForRuntime(page, budgetRuntime);
        result.runtime = observation.state === 'pending' ? null : observation.state;
        if (observation.state !== 'running') {
          let constat;
          if (observation.state === 'absent') {
            constat =
              'état observé : absent — ce HTML ne contient aucun runtime embarqué (export sans dist/pkg ?)';
          } else if (observation.state === 'error') {
            constat = `le moteur a échoué : ${observation.raw}`;
          } else if (observation.raw) {
            constat = `état observé : ${observation.raw}`;
          } else {
            constat = `window.__karacti_runtime jamais défini après ${budgetRuntime} ms`;
          }
          result.problems.push(`runtime attendu "running" mais ${constat}`);
        } else {
          // --- Niveaux embarqués (contrat du template multi-niveaux) -------
          // Le template pose `window.__karacti_levels` après un import réussi.
          // `--expect-runtime` exige au moins 1 niveau ; `--expect-levels <n>`
          // exige la valeur exacte (3 pour le jeu de référence).
          const levels = await readLevels(page).catch(() => null);
          result.levels = levels;
          if (levels === null) {
            result.problems.push(
              'window.__karacti_levels jamais défini après un import réussi ' +
                '(HTML produit par un ancien moteur ?)',
            );
          } else if (levels < 1) {
            result.problems.push(`window.__karacti_levels = ${levels} : au moins 1 niveau attendu`);
          } else if (expectLevels !== null && levels !== expectLevels) {
            result.problems.push(
              `--expect-levels : ${levels} niveau(x) embarqué(s), ${expectLevels} attendu(s)`,
            );
          }

          const budgetPixels = Math.max(
            2_000,
            Math.min(5_000, timeoutMs - (Date.now() - startedAt) - 1_000),
          );
          const sampling = await waitForCanvasContent(page, budgetPixels);
          if (!sampling.ok) {
            result.problems.push(`canvas non échantillonnable : ${sampling.reason}`);
          } else {
            result.nonBlackPixels = sampling.nonBlack;
            result.canvasColors = sampling.colors;
            if (
              !(
                sampling.nonBlack > PIXELS_MIN_NON_BLACK ||
                sampling.colors > COLORS_MIN_DISTINCT
              )
            ) {
              result.problems.push(
                `runtime running mais canvas quasi vide : ${sampling.nonBlack} pixel(s) non noir(s), ` +
                  `${sampling.colors} couleur(s) distincte(s) ` +
                  `(seuil : > ${PIXELS_MIN_NON_BLACK} pixels ou > ${COLORS_MIN_DISTINCT} couleurs)`,
              );
            }
          }

          // --- Preuve de jouabilité (uniquement avec --expect-input) --------
          // Le runtime tourne : on vérifie que le clavier le pilote vraiment.
          if (expectInput) {
            const probe = await runInputProbe(page, startedAt, timeoutMs);
            result.inputDelta = probe.delta;
            result.inputPixelsBefore = probe.beforeNonBlack;
            result.inputPixelsAfter = probe.afterNonBlack;
            for (const problem of probe.problems) result.problems.push(problem);
            if (probe.defaultPrevented === false) {
              result.problems.push(
                '--expect-input : ArrowRight n\'a pas été preventDefault() — la page risque de défiler pendant le jeu',
              );
            }
            // Le moteur doit toujours tourner après l'entrée : une touche ne
            // doit jamais casser la boucle de rendu.
            const etatApres = await readRuntimeState(page).catch(() => null);
            if (etatApres !== 'running') {
              result.problems.push(
                `--expect-input : le runtime n'est plus "running" après l'entrée (${etatApres ?? 'indéfini'})`,
              );
            }
          }
        }
      } else {
        // Constat sans attente : renseigne le JSON, jamais bloquant.
        try {
          const raw = await readRuntimeState(page);
          result.runtime = raw === null ? null : raw.startsWith('error') ? 'error' : raw;
        } catch {
          result.runtime = null;
        }
      }
    }

    // --- Contrôles runtime : erreurs console, exceptions, réseau ----------------
    for (const text of result.consoleErrors) result.problems.push(`erreur console : ${text}`);
    for (const text of result.pageErrors) result.problems.push(`exception page : ${text}`);
    for (const reqUrl of result.networkRequests) result.problems.push(`requête réseau non-file:// : ${reqUrl}`);

    result.ok = result.problems.length === 0;
    result.durationMs = Date.now() - startedAt;
    return result;
  } finally {
    clearTimeout(killer);
    if (browser) {
      // Fermeture best-effort : ne doit jamais masquer le verdict.
      await browser.close().catch(() => {});
    }
  }
}

/** Rapport humain : une ligne « OK » compacte, ou la liste des problèmes. */
function printReport(filePath, result) {
  const errors = result.consoleErrors.length + result.pageErrors.length;
  // Le détail runtime n'apparaît qu'en mode --expect-runtime : sans l'option,
  // la sortie humaine historique reste strictement inchangée.
  const runtimeSuffix = result.expectedRuntime ? `, runtime=${result.runtime ?? 'indéfini'}` : '';
  const levelsSuffix =
    result.expectedLevels !== null
      ? `, niveaux=${result.levels ?? 'indéfini'}/${result.expectedLevels}`
      : result.expectedRuntime
        ? `, niveaux=${result.levels ?? 'indéfini'}`
        : '';
  const pixelsSuffix =
    result.expectedRuntime && result.nonBlackPixels !== null
      ? `, ${result.nonBlackPixels} pixels non noirs (${result.canvasColors} couleurs)`
      : '';
  // Le détail d'entrée n'apparaît qu'avec --expect-input.
  const inputSuffix = result.expectedInput
    ? `, entrée=${result.inputDelta ?? 'non mesurée'} pixels modifiés` +
      (result.inputPixelsBefore !== null && result.inputPixelsAfter !== null
        ? ` (${result.inputPixelsBefore} → ${result.inputPixelsAfter} pixels non noirs)`
        : '')
    : '';
  if (result.ok) {
    const plural = result.entities === 1 ? '' : 's';
    console.log(
      `[export-smoke] OK — ${result.entities} entité${plural}, 0 erreur, 0 requête réseau` +
        `${runtimeSuffix}${levelsSuffix}${pixelsSuffix}${inputSuffix}`,
    );
    return;
  }
  console.error(`[export-smoke] ÉCHEC — ${result.problems.length} problème(s) sur ${filePath}`);
  for (const problem of result.problems) console.error(`[export-smoke]   • ${problem}`);
  console.error(
    `[export-smoke] résumé : ${result.entities ?? 0} entité(s), ` +
      `${errors} erreur(s) console/page, ${result.networkRequests.length} requête(s) réseau non-file://` +
      runtimeSuffix + levelsSuffix + inputSuffix,
  );
}

// `--json` doit rester accessible à la borne dure de timeout.
const parsed = parseArgs(argv);
const JSON_OUTPUT = Boolean(parsed.json);

async function main() {
  const expectInput = Boolean(parsed.expectInput);
  const expectLevels = parsed.expectLevels ?? null;
  // Prouver l'entrée ou le nombre de niveaux suppose un moteur qui tourne :
  // --expect-input et --expect-levels impliquent --expect-runtime (un fichier
  // sans runtime échoue proprement, sans crash).
  const expectRuntime = Boolean(parsed.expectRuntime || expectInput || expectLevels !== null);
  if (parsed.error) usageError(parsed.error, JSON_OUTPUT, expectRuntime, expectInput, expectLevels);
  if (parsed.help) {
    // L'aide documente l'usage ; code 2 (aucun test exécuté).
    process.stdout.write(USAGE);
    process.exit(2);
  }
  if (!parsed.file) usageError('chemin du fichier HTML manquant', JSON_OUTPUT, expectRuntime, expectInput, expectLevels);

  const filePath = path.resolve(process.cwd(), parsed.file);
  if (!existsSync(filePath)) usageError(`fichier introuvable : ${filePath}`, JSON_OUTPUT, expectRuntime, expectInput, expectLevels);
  if (!statSync(filePath).isFile()) usageError(`ce n'est pas un fichier : ${filePath}`, JSON_OUTPUT, expectRuntime, expectInput, expectLevels);

  let result;
  try {
    result = await runSmoke(filePath, parsed.timeoutMs, expectRuntime, expectInput, expectLevels);
  } catch (err) {
    // Erreur d'environnement (navigateur indisponible, sandbox…) : exit 2.
    const message = String(err?.message ?? err).split('\n')[0];
    if (JSON_OUTPUT) {
      const failed = {
        ...emptyResult(),
        expectedRuntime: expectRuntime,
        expectedInput: expectInput,
        expectedLevels: expectLevels,
        problems: [message],
      };
      process.stdout.write(JSON.stringify(failed, null, 2) + '\n');
    } else {
      console.error(`[export-smoke] impossible d'exécuter le test : ${message}`);
    }
    process.exit(2);
  }

  if (JSON_OUTPUT) {
    // Sortie machine : uniquement l'objet demandé sur stdout.
    const { durationMs, ...payload } = result;
    void durationMs;
    process.stdout.write(JSON.stringify(payload, null, 2) + '\n');
  } else {
    printReport(filePath, result);
  }
  process.exitCode = result.ok ? 0 : 1;
}

main().catch((err) => {
  console.error('[export-smoke] erreur inattendue :', err);
  process.exit(1);
});
