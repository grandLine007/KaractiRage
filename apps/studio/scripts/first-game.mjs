/**
 * Mesure AC1 — « un nouveau jeu jouable en moins de 2 minutes ».
 *
 * Ce harnais sert le build de production (vite preview) puis rejoue dans Chrome
 * (headless) le parcours **automatisé** d'un nouvel utilisateur :
 *
 *   Hub → « Créer mon premier jeu » → nom + modèle → « Créer & ouvrir dans le
 *   Studio » → mode « Jouer » → canvas rendu, sans overlay bloquant.
 *
 * Il chronomètre ce parcours depuis le lancement de la navigation jusqu'à l'état
 * jouable, affiche le temps et échoue au-delà de 120 000 ms (seuil AC1).
 *
 * ⚠ Ce n'est PAS un chronométrage humain : c'est le temps **machine** du parcours
 * d'interface, mesuré sous Chrome headless avec l'IPC Tauri simulé. C'est une
 * borne basse (aucune hésitation, aucune lecture, aucun boot natif réel).
 *
 * L'IPC est simulé avec **exactement** `scripts/mock-ipc.js`, le mock du harnais
 * d'audit (`scripts/audit-ui.mjs`), car `vite preview` n'a pas de backend Tauri.
 * Le chemin WebGL2 est forcé de la même façon que l'audit (déterminisme GPU).
 *
 * Usage :
 *   node scripts/first-game.mjs [options]
 *
 *   --json                Sortie JSON `{ ok, elapsedMs, steps[], problems[], … }`.
 *   --timeout <ms>        Délai max pour atteindre l'état jouable (défaut 150 000,
 *                         soit le seuil AC1 + 30 s de marge pour mesurer un dépassement).
 *   --no-build            Réutilise le `dist/` existant au lieu de relancer le build.
 *   --url <url>           Mesure une URL déjà servie (aucun build, aucun preview).
 *   --out <dir>           Dossier des captures (défaut : tmp/karactirage-first-game-*).
 *
 * Sortie : code 0 si AC1 est respecté, 1 sinon (ou si l'état jouable / le contrôle
 * Échap en pile échoue). Aucune baseline n'est lue ni modifiée.
 */

import { chromium } from 'playwright-core';
import { spawn } from 'node:child_process';
import { mkdirSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import os from 'node:os';
import path from 'node:path';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const MOCK_PATH = path.join(__dirname, 'mock-ipc.js');
const PORT = 4174;
const BASE_URL = `http://127.0.0.1:${PORT}/`;
const AC1_THRESHOLD_MS = 120_000;
const DEFAULT_TIMEOUT_MS = 150_000;

// ---------------------------------------------------------------------------
// Arguments
// ---------------------------------------------------------------------------

function parseArgs(argv) {
  const options = {
    json: false,
    noBuild: false,
    timeoutMs: DEFAULT_TIMEOUT_MS,
    outDir: null,
    url: null,
    help: false,
  };

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--json') {
      options.json = true;
    } else if (arg === '--no-build') {
      options.noBuild = true;
    } else if (arg === '--timeout') {
      const value = Number(argv[++i]);
      if (!Number.isFinite(value) || value <= 0) {
        throw new Error('--timeout attend un nombre de millisecondes > 0');
      }
      options.timeoutMs = value;
    } else if (arg === '--out') {
      const value = argv[++i];
      if (!value) throw new Error('--out attend un dossier');
      options.outDir = path.resolve(value);
    } else if (arg === '--url') {
      const value = argv[++i];
      if (!value) throw new Error('--url attend une URL');
      options.url = value;
    } else if (arg === '--help' || arg === '-h') {
      options.help = true;
    } else {
      throw new Error(`Option inconnue : ${arg}`);
    }
  }

  return options;
}

const options = parseArgs(process.argv.slice(2));

if (options.help) {
  console.log(`Mesure AC1 — nouveau jeu jouable en < 2 min (parcours automatisé).

Usage : node scripts/first-game.mjs [--json] [--timeout <ms>] [--no-build] [--url <url>] [--out <dir>]

  --json         sortie JSON { ok, elapsedMs, steps[], problems[] }
  --timeout ms   délai max pour l'état jouable (défaut ${DEFAULT_TIMEOUT_MS} ms)
  --no-build     réutilise dist/ existant
  --url <url>    mesure une URL déjà servie (sinon vite preview sur ${BASE_URL})
  --out <dir>    dossier des captures`);
  process.exit(0);
}

/** Journal lisible : silencieux en mode JSON pour garder un stdout parsable. */
function log(message) {
  if (!options.json) console.log(message);
}

/** Journal toujours actif (erreurs / mode JSON) sur stderr. */
function logError(message) {
  console.error(message);
}

const OUT_DIR = options.outDir ?? path.join(
  os.tmpdir(),
  `karactirage-first-game-${new Date().toISOString().replace(/[:.]/g, '-')}`,
);
mkdirSync(OUT_DIR, { recursive: true });

const screenshots = [];

async function screenshot(page, name) {
  const file = path.join(OUT_DIR, name);
  try {
    await page.screenshot({ path: file, fullPage: false });
    screenshots.push(file);
  } catch {
    /* une capture ratée ne doit pas invalider la mesure */
  }
}

// ---------------------------------------------------------------------------
// Serveur & build (même motif que scripts/audit-ui.mjs)
// ---------------------------------------------------------------------------

function runCommand(cmd, args, cwd) {
  return new Promise((resolve, reject) => {
    const child = spawn(cmd, args, { cwd, stdio: ['ignore', 'pipe', 'pipe'] });
    let output = '';
    child.stdout?.on('data', (chunk) => { output += chunk; });
    child.stderr?.on('data', (chunk) => { output += chunk; });
    child.on('close', (code) => {
      if (code === 0) resolve(output);
      else reject(new Error(`${cmd} ${args.join(' ')} → code ${code}\n${output.slice(-1500)}`));
    });
    child.on('error', reject);
  });
}

function startPreview() {
  return spawn('npx', ['vite', 'preview', '--port', String(PORT), '--strictPort', '--host', '127.0.0.1'], {
    cwd: ROOT,
    stdio: 'ignore',
  });
}

async function waitForServer(url, timeoutMs = 30_000) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    try {
      const response = await fetch(url);
      if (response.ok) return;
    } catch {
      /* pas encore prêt */
    }
    await new Promise((resolve) => setTimeout(resolve, 250));
  }
  throw new Error(`Serveur vite preview injoignable sur ${url}`);
}

// ---------------------------------------------------------------------------
// État « jouable » — prédicat unique, sans dépendance au texte du parcours
// ---------------------------------------------------------------------------

/**
 * Renvoie un diagnostic de l'état jouable. Exécuté dans la page.
 *
 * Jouable = mode « Jouer » actif, canvas visible avec au moins une frame
 * transférée, aucun overlay bloquant (hub, chargement moteur, erreur) et rien
 * qui recouvre le centre du canvas.
 */
function readPlayableState() {
  const canvas = document.querySelector('[data-testid="canvas-viewport"]');
  if (!canvas) return { ok: false, reason: 'canvas absent' };

  const rect = canvas.getBoundingClientRect();
  if (rect.width < 24 || rect.height < 24) {
    return { ok: false, reason: `canvas non visible (${Math.round(rect.width)}×${Math.round(rect.height)})` };
  }
  if (canvas.width < 2 || canvas.height < 2) {
    return { ok: false, reason: 'canvas sans bitmap' };
  }

  const playButton = document.querySelector('[data-testid="header-modes"] button[title="Jouer (Play)"]');
  if (!playButton || playButton.getAttribute('aria-pressed') !== 'true') {
    return { ok: false, reason: 'mode « Jouer » non actif' };
  }

  if (document.querySelector('[data-testid="hub"]')) {
    return { ok: false, reason: 'hub encore ouvert' };
  }

  const text = document.body.innerText || '';
  for (const blocker of ['Préparation du moteur natif', 'Erreur du moteur', 'Compilation GPU']) {
    if (text.includes(blocker)) return { ok: false, reason: `overlay bloquant : ${blocker}` };
  }

  // Au moins une frame a été transférée au canvas (compteur `putImageData`
  // installé par preparePage : le buffer du mock est transparent, un échantillon
  // de pixels ne prouverait rien).
  if (!(window.__karactiFramePaints > 0)) {
    return { ok: false, reason: 'aucune frame transférée au canvas' };
  }

  const centerX = rect.left + rect.width / 2;
  const centerY = rect.top + rect.height / 2;
  const stack = document.elementsFromPoint(centerX, centerY);
  if (!stack.includes(canvas)) return { ok: false, reason: 'canvas recouvert par un overlay' };

  return {
    ok: true,
    canvas: { width: Math.round(rect.width), height: Math.round(rect.height) },
  };
}

/**
 * Attend l'état jouable en pollant la page (contrairement à waitForFunction, on
 * conserve le dernier diagnostic pour l'expliquer dans `problems`).
 */
async function waitForPlayable(page, deadline) {
  // Au moins une lecture, même si le budget est déjà dépassé : le diagnostic
  // doit décrire l'état réel, pas un « aucune lecture » trompeur.
  let last = await page.evaluate(readPlayableState);
  let at = last.ok ? Date.now() : null;
  while (!last.ok && Date.now() < deadline) {
    await page.waitForTimeout(50);
    last = await page.evaluate(readPlayableState);
    if (last.ok) at = Date.now();
  }
  return { state: last, at };
}

// ---------------------------------------------------------------------------
// Préparation d'une page (identique à scripts/audit-ui.mjs)
// ---------------------------------------------------------------------------

async function preparePage(page) {
  // Mock IPC Tauri exactement identique à l'audit : sans backend natif sous
  // Chrome headless, c'est lui qui fait basculer le frontend en mode Tauri.
  await page.addInitScript({ path: MOCK_PATH });
  // Chemin WebGL2 forcé, comme l'audit : le tier GPU ne doit pas dépendre de Chrome.
  await page.addInitScript(() => {
    try {
      Object.defineProperty(navigator, 'gpu', { get: () => undefined, configurable: true });
    } catch {
      /* moteur sans WebGPU : le chemin WebGL2 est déjà celui attendu */
    }
  });
  // Preuve de rendu : le framebuffer du mock est **entièrement transparent**
  // (`new Uint8Array(...)` à zéro), donc un échantillon de pixels ne peut pas
  // distinguer « frame écrite » de « canvas vierge ». On compte les transferts
  // `putImageData` du viewport : > 0 ⇒ le frontend a bien poussé une frame.
  await page.addInitScript(() => {
    window.__karactiFramePaints = 0;
    const proto = CanvasRenderingContext2D.prototype;
    const original = proto.putImageData;
    proto.putImageData = function (...args) {
      try {
        window.__karactiFramePaints += 1;
      } catch {
        /* compteur best-effort, jamais bloquant */
      }
      return original.apply(this, args);
    };
  });
}

/**
 * Collecte les erreurs de la page. Les exceptions non rattrapées sont fatales ;
 * les `console.error` sont du bruit à rapporter (le hub en émet déjà, hors
 * périmètre : `height` de `<rect>` négatif dans les vignettes).
 */
function watchRuntimeErrors(page) {
  const consoleErrors = [];
  const pageErrors = [];
  page.on('console', (message) => {
    if (message.type() === 'error') consoleErrors.push(message.text());
  });
  page.on('pageerror', (error) => pageErrors.push(`pageerror: ${error.message}`));
  return { consoleErrors, pageErrors };
}

function dedupe(list, max = 8) {
  return [...new Set(list.map((item) => String(item).replace(/\s+/g, ' ').trim()))]
    .filter(Boolean)
    .slice(0, max);
}

// ---------------------------------------------------------------------------
// AC1 — parcours « nouveau jeu → jouable »
// ---------------------------------------------------------------------------

async function measureFirstGame(browser, baseUrl, timeoutMs) {
  const steps = [];
  const problems = [];
  const warnings = [];
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const runtimeErrors = watchRuntimeErrors(page);
  let elapsedMs = null;
  let hubReadyMs = null;
  let playableState = null;

  // Base de temps commune aux étapes et à la mesure AC1 (départ de la navigation).
  let t0 = Date.now();
  const mark = (id, label, detail = null) => {
    const step = { id, label, atMs: Date.now() - t0 };
    if (detail) step.detail = detail;
    steps.push(step);
  };

  try {
    await preparePage(page);

    // t0 : départ de la navigation, pas du chargement du module Node.
    t0 = Date.now();
    await page.goto(baseUrl, { waitUntil: 'domcontentloaded' });
    mark('navigation', 'Navigation vers le Studio');

    // 1. Hub des projets (écran d'accueil).
    await page.getByTestId('hub').waitFor({ state: 'visible', timeout: 30_000 });
    hubReadyMs = Date.now() - t0;
    mark('hub-ready', 'Hub des projets affiché', `hub prêt en ${hubReadyMs} ms`);

    // 2. « Créer mon premier jeu » (parcours premier utilisateur).
    const firstGameCta = page.locator('button').filter({ hasText: 'Créer mon premier jeu' }).first();
    await firstGameCta.waitFor({ state: 'visible', timeout: 10_000 });
    await firstGameCta.click();
    mark('first-game-click', '« Créer mon premier jeu » cliqué');

    // 3. Formulaire : nom + modèle de départ (un archétype = un jeu jouable).
    const nameInput = page.getByLabel('Nom du projet');
    await nameInput.waitFor({ state: 'visible', timeout: 10_000 });
    await nameInput.fill('Mon Premier Jeu — AC1');
    await page.getByLabel('Modèle de départ (optionnel)').selectOption('platformer');
    mark('form-filled', 'Nom du projet + modèle Platformer renseignés');

    // 4. Création → ouverture dans le Studio (mode Créer).
    await page.locator('button').filter({ hasText: 'Créer & ouvrir dans le Studio' }).first().click();
    mark('create-click', 'Création du projet lancée');
    await page.getByTestId('hub').waitFor({ state: 'detached', timeout: 15_000 });
    await page.getByTestId('canvas-viewport').waitFor({ state: 'visible', timeout: 15_000 });
    mark('studio-open', 'Projet ouvert dans le Studio');

    // 5. Passage en mode « Jouer ».
    await page.locator('[data-testid="header-modes"] button[title="Jouer (Play)"]').click();
    mark('play-mode', 'Mode « Jouer » activé');

    // 6. Attente de l'état jouable (canvas rendu, aucun overlay bloquant).
    const deadline = t0 + timeoutMs;
    const result = await waitForPlayable(page, deadline);
    playableState = result.state;
    if (!result.at) {
      problems.push(
        `État jouable non atteint dans le délai de ${timeoutMs} ms — dernier diagnostic : ${result.state?.reason ?? 'inconnu'}`,
      );
      elapsedMs = Date.now() - t0;
    } else {
      elapsedMs = result.at - t0;
      mark('playable', 'Canvas rendu, aucun overlay bloquant', 'état jouable atteint');
      if (elapsedMs > timeoutMs) {
        problems.push(
          `État jouable atteint après le budget de ${timeoutMs} ms (${elapsedMs} ms) — budget de harnais dépassé, le temps réel est conservé.`,
        );
      }
    }

    // `elapsedMs` est déjà figé : on laisse seulement la transition CSS des
    // boutons de mode se peindre pour que la capture de preuve soit lisible.
    await page.waitForTimeout(250);
    await screenshot(page, '04-playable.png');
  } catch (error) {
    problems.push(`Parcours interrompu : ${error?.message ?? error}`);
    if (elapsedMs === null) elapsedMs = Date.now() - t0;
    await screenshot(page, '99-echec.png').catch(() => {});
  } finally {
    if (runtimeErrors.pageErrors.length > 0) {
      problems.push(...dedupe(runtimeErrors.pageErrors).map((message) => `Exception non rattrapée : ${message}`));
    }
    if (runtimeErrors.consoleErrors.length > 0) {
      warnings.push(...dedupe(runtimeErrors.consoleErrors, 12).map((message) => `console.error : ${message}`));
    }
    await page.close().catch(() => {});
  }

  const pass = elapsedMs !== null && elapsedMs <= AC1_THRESHOLD_MS && playableState?.ok === true;
  return {
    ok: pass && problems.length === 0,
    elapsedMs: elapsedMs ?? Date.now() - t0,
    steps,
    problems,
    warnings,
    hubReadyMs,
    playable: playableState,
  };
}

// ---------------------------------------------------------------------------
// Contrôle Échap en pile (barre IA inline ⊕ panneau Sauvegardes)
// ---------------------------------------------------------------------------

/**
 * Vérifie la pile Échap dans une page neuve, **dans les deux ordres d'ouverture**
 * (LIFO strict) :
 *
 *   A. panneau Sauvegardes ouvert, PUIS barre IA (Ctrl+K) :
 *      1er Échap → barre IA fermée, panneau visible, focus ✦ ;
 *      2e Échap → panneau fermé, focus bouton « Sauvegarde ».
 *   B. barre IA ouverte, PUIS panneau Sauvegardes :
 *      1er Échap → panneau fermé, barre IA visible ;
 *      2e Échap → barre IA fermée, focus ✦.
 *
 * Puis : H ouvre le hub, Échap le ferme (les modales ne passent qu'après les
 * couches), et non-régression Ctrl+2 / Ctrl+1 / toggle Ctrl+K.
 */
async function checkEscapeStack(browser, baseUrl) {
  const checks = [];
  const problems = [];
  const warnings = [];
  let phase = 'initialisation';
  const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
  const runtimeErrors = watchRuntimeErrors(page);

  const check = (id, label, ok, detail = null) => {
    checks.push({ id, label, ok: Boolean(ok), ...(detail ? { detail } : {}) });
    if (!ok) problems.push(`${label}${detail ? ` — ${detail}` : ''}`);
  };

  const activeElement = () => page.evaluate(() => {
    const element = document.activeElement;
    return {
      testId: element?.getAttribute?.('data-testid') ?? null,
      ariaLabel: element?.getAttribute?.('aria-label') ?? null,
      tag: element?.tagName?.toLowerCase() ?? null,
    };
  });

  // La restauration du focus passe par un setTimeout(0) côté App : on laisse au
  // navigateur le temps de l'appliquer avant de juger (sinon course de timers).
  const waitForFocus = async (match, timeoutMs = 2000) => {
    const deadline = Date.now() + timeoutMs;
    let focus = await activeElement();
    while (Date.now() < deadline && !match(focus)) {
      await page.waitForTimeout(50);
      focus = await activeElement();
    }
    return focus;
  };

  const describeFocus = (focus) => focus.ariaLabel ?? focus.testId ?? focus.tag;

  // Laisse React vider ses effets passifs (focus de la barre IA) avant de presser
  // Échap : sans cela, le test course le commit et ne mesure pas le comportement
  // réel (un humain ne presse pas Échap dans la même milliseconde).
  const settle = () => page.waitForTimeout(250);

  try {
    await preparePage(page);
    await page.goto(baseUrl, { waitUntil: 'domcontentloaded' });
    await page.getByTestId('hub').waitFor({ state: 'visible', timeout: 15_000 });
    await page.getByTestId('hub').getByLabel('Fermer').click();
    await page.getByTestId('header-modes').getByText('Créer', { exact: false }).click();
    await page.waitForTimeout(250);

    // --- A. Panneau d'abord, puis barre IA ---------------------------------
    phase = 'A : ouverture panneau puis barre IA';
    await page.getByLabel('Sauvegarde', { exact: true }).click();
    await page.locator('#save-slots-panel').waitFor({ state: 'visible', timeout: 5000 });
    await page.keyboard.press('Control+k');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'visible', timeout: 5000 });
    await settle();
    await screenshot(page, '05-ordre-a-panneau-puis-ia.png');

    phase = 'A1 : 1er Échap (barre IA)';
    await page.keyboard.press('Escape');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'detached', timeout: 3000 });
    const panelAfterA1 = await page.locator('#save-slots-panel').isVisible();
    check(
      'A1-ia-ferme',
      'Ordre A — 1er Échap ferme la barre IA ouverte en dernier',
      panelAfterA1,
      panelAfterA1 ? 'panneau toujours visible' : 'panneau fermé prématurément',
    );
    const focusA1 = await waitForFocus((focus) => (focus.testId ?? '').startsWith('ai-sparkle'));
    check(
      'A1-focus',
      'Ordre A — focus rendu à une affordance ✦',
      (focusA1.testId ?? '').startsWith('ai-sparkle'),
      `focus réel : ${describeFocus(focusA1)}`,
    );
    await screenshot(page, '06-ordre-a-ia-fermee.png');
    await settle();

    phase = 'A2 : 2e Échap (panneau)';
    await page.keyboard.press('Escape');
    await page.locator('#save-slots-panel').waitFor({ state: 'detached', timeout: 3000 });
    const focusA2 = await waitForFocus((focus) => focus.ariaLabel === 'Sauvegarde');
    check(
      'A2-panneau-ferme',
      'Ordre A — 2e Échap ferme le panneau, focus au bouton « Sauvegarde »',
      focusA2.ariaLabel === 'Sauvegarde',
      `focus réel : ${describeFocus(focusA2)}`,
    );
    await screenshot(page, '07-ordre-a-panneau-ferme.png');

    // --- B. Barre IA d'abord, puis panneau ---------------------------------
    phase = 'B : ouverture barre IA puis panneau';
    await page.keyboard.press('Control+k');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'visible', timeout: 5000 });
    await settle();
    await page.getByLabel('Sauvegarde', { exact: true }).click();
    await page.locator('#save-slots-panel').waitFor({ state: 'visible', timeout: 5000 });
    await settle();
    await screenshot(page, '08-ordre-b-ia-puis-panneau.png');

    phase = 'B1 : 1er Échap (panneau)';
    await page.keyboard.press('Escape');
    await page.locator('#save-slots-panel').waitFor({ state: 'detached', timeout: 3000 });
    const aiAfterB1 = await page.getByTestId('inline-ai-bar').isVisible();
    check(
      'B1-panneau-ferme',
      'Ordre B — 1er Échap ferme le panneau ouvert en dernier, la barre IA reste',
      aiAfterB1,
      aiAfterB1 ? 'barre IA toujours visible' : 'barre IA fermée prématurément',
    );
    const focusB1 = await waitForFocus((focus) => focus.ariaLabel === 'Sauvegarde');
    check(
      'B1-focus',
      'Ordre B — focus rendu au bouton « Sauvegarde »',
      focusB1.ariaLabel === 'Sauvegarde',
      `focus réel : ${describeFocus(focusB1)}`,
    );
    await screenshot(page, '09-ordre-b-panneau-ferme.png');
    await settle();

    phase = 'B2 : 2e Échap (barre IA)';
    await page.keyboard.press('Escape');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'detached', timeout: 3000 });
    const focusB2 = await waitForFocus((focus) => (focus.testId ?? '').startsWith('ai-sparkle'));
    check(
      'B2-ia-ferme',
      'Ordre B — 2e Échap ferme la barre IA, focus rendu à une affordance ✦',
      (focusB2.testId ?? '').startsWith('ai-sparkle'),
      `focus réel : ${describeFocus(focusB2)}`,
    );
    await screenshot(page, '10-ordre-b-ia-fermee.png');

    // --- Modales : seulement quand aucune couche n'est ouverte -------------
    phase = 'modales : hub';
    await page.keyboard.press('h');
    await page.getByTestId('hub').waitFor({ state: 'visible', timeout: 3000 });
    await page.keyboard.press('Escape');
    await page.getByTestId('hub').waitFor({ state: 'detached', timeout: 3000 });
    check('modal-escape', 'Échap ferme encore le hub quand aucune couche n’est ouverte', true);

    // --- Non-régression des raccourcis de mode et du toggle Ctrl+K ---------
    phase = 'raccourcis : Ctrl+2 / Ctrl+K / Ctrl+1';
    await page.keyboard.press('Control+2');
    await page.waitForTimeout(100);
    const editPressed = await page
      .locator('[data-testid="header-modes"] button[title="Créer — Atelier Studio"]')
      .getAttribute('aria-pressed');
    check('shortcut-ctrl-2', 'Ctrl+2 passe en mode Créer', editPressed === 'true', `aria-pressed=${editPressed}`);
    // Le champ IA est focalisé à l'ouverture : Ctrl+K y est volontairement ignoré
    // (les saisies gardent leurs raccourcis). On rend le focus au viewport avant
    // de tester le toggle, comme le ferait un utilisateur qui clique dans la scène.
    await page.keyboard.press('Control+k');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'visible', timeout: 3000 });
    await settle();
    await page.locator('[data-testid="canvas-viewport"]').focus();
    await page.keyboard.press('Control+k');
    await page.getByTestId('inline-ai-bar').waitFor({ state: 'detached', timeout: 3000 });
    check('shortcut-ctrl-k', 'Ctrl+K ouvre la barre IA puis la referme hors champ de saisie', true);
    await page.keyboard.press('Control+1');
    await page.waitForTimeout(100);
    const playPressed = await page
      .locator('[data-testid="header-modes"] button[title="Jouer (Play)"]')
      .getAttribute('aria-pressed');
    check('shortcut-ctrl-1', 'Ctrl+1 repasse en mode Jouer', playPressed === 'true', `aria-pressed=${playPressed}`);
  } catch (error) {
    problems.push(`Contrôle Échap interrompu (${phase}) : ${error?.message ?? error}`);
  } finally {
    if (runtimeErrors.pageErrors.length > 0) {
      problems.push(...dedupe(runtimeErrors.pageErrors).map((message) => `Exception non rattrapée : ${message}`));
    }
    if (runtimeErrors.consoleErrors.length > 0) {
      warnings.push(...dedupe(runtimeErrors.consoleErrors, 12).map((message) => `console.error : ${message}`));
    }
    await page.close().catch(() => {});
  }

  return {
    ok: problems.length === 0 && checks.every((entry) => entry.ok),
    checks,
    problems,
    warnings,
  };
}

// ---------------------------------------------------------------------------
// Rapport
// ---------------------------------------------------------------------------

function formatSeconds(ms) {
  return `${(ms / 1000).toFixed(2).replace('.', ',')} s`;
}

/**
 * Formulation strictement machine : on ne prétend jamais avoir chronométré un
 * humain. La marge sous le seuil est celle du parcours automatisé.
 */
function humanPaceHint(ms) {
  return `parcours automatisé : ${formatSeconds(ms)} (${ms} ms) — temps machine ` +
    `(Chrome headless, IPC Tauri simulé), pas un chronométrage humain.`;
}

function printReport(report) {
  // Trace JSON persistante à côté des captures (même en mode lisible).
  const reportFile = path.join(OUT_DIR, 'report.json');
  try {
    writeFileSync(reportFile, JSON.stringify(report, null, 2));
  } catch {
    /* la trace écrite est un bonus : ne jamais faire échouer la mesure */
  }

  if (options.json) {
    console.log(JSON.stringify(report, null, 2));
    return;
  }

  log('');
  log(`[first-game] ${humanPaceHint(report.elapsedMs)}`);
  log(`[first-game] seuil AC1 : ${AC1_THRESHOLD_MS} ms (${formatSeconds(AC1_THRESHOLD_MS)}) → ${report.ac1.pass ? 'OK' : 'DÉPASSÉ'}`);
  if (report.ac1.pass) {
    log(`[first-game] marge machine sous le seuil : ${Math.round((AC1_THRESHOLD_MS - report.elapsedMs) / 1000)} s (un humain ajoute la lecture et la manipulation, non mesurées).`);
  } else {
    log('[first-game] dépassement déjà constaté côté machine : disqualifiant avant toute latence humaine.');
  }
  if (report.hubReadyMs !== null) {
    log(`[first-game] hub prêt depuis la navigation : ${report.hubReadyMs} ms`);
  }
  if (report.playable?.ok) {
    log(`[first-game] état jouable : canvas ${report.playable.canvas?.width}×${report.playable.canvas?.height}, aucun overlay bloquant`);
  }
  log('[first-game] étapes :');
  for (const step of report.steps) {
    log(`  - ${step.id.padEnd(16)} ${String(step.atMs).padStart(7)} ms  ${step.label}`);
  }
  if (report.interaction) {
    log(`[first-game] contrôle Échap en pile : ${report.interaction.ok ? 'OK' : 'ÉCHEC'}`);
    for (const entry of report.interaction.checks) {
      log(`  - ${entry.ok ? 'OK  ' : 'ÉCHEC'} ${entry.id.padEnd(20)} ${entry.label}${entry.detail ? ` (${entry.detail})` : ''}`);
    }
  }
  if (report.warnings.length > 0) {
    log('[first-game] avertissements (non bloquants) :');
    for (const warning of report.warnings) log(`  … ${warning}`);
  }
  if (report.problems.length > 0) {
    log('[first-game] problèmes :');
    for (const problem of report.problems) log(`  ⚠ ${problem}`);
  }
  log('');
  log('[first-game] ⚠ Mesure machine uniquement : Chrome headless + IPC Tauri simulé (scripts/mock-ipc.js),');
  log('             sans boot natif. Ce n’est pas un chronométrage humain — c’est une borne basse.');
  if (screenshots.length > 0) {
    log(`[first-game] captures : ${screenshots.join(' , ')}`);
  }
  log(`[first-game] rapport JSON : ${reportFile}`);
  log(
    `[first-game] AC1 : ${report.ac1.pass ? 'respecté' : 'NON respecté'} · ` +
    `contrôle Échap : ${report.interaction?.ok ? 'OK' : 'ÉCHEC'} → ${report.ok ? 'SUCCÈS' : 'ÉCHEC'}`,
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

async function main() {
  const report = {
    ok: false,
    elapsedMs: 0,
    steps: [],
    problems: [],
    warnings: [],
    ac1: { thresholdMs: AC1_THRESHOLD_MS, pass: false },
    hubReadyMs: null,
    playable: null,
    interaction: null,
    measuredFrom: 'navigation',
    machineOnly: true,
    mockIpc: {
      used: true,
      path: 'scripts/mock-ipc.js',
      reason: 'Chrome headless n’a pas de backend Tauri : même mock IPC que scripts/audit-ui.mjs',
    },
    screenshots,
  };

  let server = null;
  let browser = null;

  try {
    if (!options.url) {
      if (!options.noBuild) {
        log('[first-game] build de production…');
        await runCommand('npm', ['run', 'build'], ROOT);
      }
      server = startPreview();
      await waitForServer(BASE_URL);
      log(`[first-game] serveur prêt sur ${BASE_URL}`);
    }

    browser = await chromium.launch({ channel: 'chrome', headless: true });
    const baseUrl = options.url ?? BASE_URL;

    log('[first-game] mesure AC1…');
    const measure = await measureFirstGame(browser, baseUrl, options.timeoutMs);
    report.elapsedMs = measure.elapsedMs;
    report.steps = measure.steps;
    report.hubReadyMs = measure.hubReadyMs;
    report.playable = measure.playable;
    report.problems.push(...measure.problems);
    report.warnings.push(...measure.warnings);

    log('[first-game] contrôle Échap en pile…');
    report.interaction = await checkEscapeStack(browser, baseUrl);
    report.problems.push(...report.interaction.problems.map((problem) => `Échap : ${problem}`));
    report.warnings.push(...report.interaction.warnings);

    report.ac1 = {
      thresholdMs: AC1_THRESHOLD_MS,
      pass: measure.elapsedMs <= AC1_THRESHOLD_MS && measure.playable?.ok === true,
    };
    report.screenshots = [...screenshots];
    report.ok = report.ac1.pass && report.problems.length === 0;
  } catch (error) {
    report.problems.push(`Harnais : ${error?.message ?? error}`);
    report.ok = false;
  } finally {
    if (browser) await browser.close().catch(() => {});
    if (server) server.kill('SIGTERM');
  }

  printReport(report);
  process.exitCode = report.ok ? 0 : 1;
}

main().catch((error) => {
  logError(`[first-game] erreur : ${error?.stack ?? error}`);
  process.exitCode = 1;
});
