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
 * Usage :
 *   node scripts/export-smoke.mjs <chemin-vers-html> [--json] [--timeout <ms>]
 *   node scripts/export-smoke.mjs --help
 *
 * Options :
 *   --json           sort un objet JSON machine (rien d'autre sur stdout) :
 *                    { ok, entities, consoleErrors, pageErrors,
 *                      networkRequests, problems }
 *   --timeout <ms>   délai global (défaut : 30 000 ms), pour ne jamais
 *                    suspendre un CI.
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

/** Schémas d'URL qui ne constituent PAS une requête réseau. */
const LOCAL_PROTOCOLS = ['file:', 'data:', 'blob:', 'about:', 'chrome:', 'devtools:'];

const USAGE = `Usage : node scripts/export-smoke.mjs <chemin-vers-html> [options]

Test de fumée d'un export HTML5 autonome KaractiRage (P1.5) : charge le fichier
en file:// dans un Chrome headless et vérifie qu'il tourne hors ligne, sans
erreur console, avec un paquet embarqué valide et un canvas.

Options :
  --json           sortie JSON machine { ok, entities, consoleErrors,
                   pageErrors, networkRequests, problems }
  --timeout <ms>   délai global en millisecondes (défaut : ${DEFAULT_TIMEOUT_MS})
  -h, --help       affiche cette aide

Codes de sortie :
  0  export conforme
  1  export défectueux (erreurs console, requêtes réseau, paquet, canvas…)
  2  usage incorrect, fichier introuvable, ou navigateur indisponible

Exemples :
  npm run export:smoke -- ./jeu-standalone.html
  node scripts/export-smoke.mjs ./jeu-standalone.html --json
`;

const argv = process.argv.slice(2);

/** Analyse très simple des arguments (aucune lib de CLI : script autonome). */
function parseArgs(args) {
  const opts = { file: null, json: false, help: false, timeoutMs: DEFAULT_TIMEOUT_MS };
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === '--json') {
      opts.json = true;
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
function usageError(message, json) {
  if (json) {
    process.stdout.write(
      JSON.stringify(
        { ok: false, entities: null, consoleErrors: [], pageErrors: [], networkRequests: [], problems: [message] },
        null,
        2,
      ) + '\n',
    );
  } else {
    console.error(`[export-smoke] ${message}`);
    console.error('[export-smoke] Voir : node scripts/export-smoke.mjs --help');
  }
  process.exit(2);
}

/**
 * Résultat vide : structure unique utilisée pour toutes les sorties JSON,
 * y compris en cas d'échec précoce.
 */
function emptyResult() {
  return { ok: false, entities: null, consoleErrors: [], pageErrors: [], networkRequests: [], problems: [] };
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

/** Exécute le test de fumée. Renvoie l'objet résultat + un code de sortie. */
async function runSmoke(filePath, timeoutMs) {
  const result = emptyResult();
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
  if (result.ok) {
    const plural = result.entities === 1 ? '' : 's';
    console.log(`[export-smoke] OK — ${result.entities} entité${plural}, 0 erreur, 0 requête réseau`);
    return;
  }
  console.error(`[export-smoke] ÉCHEC — ${result.problems.length} problème(s) sur ${filePath}`);
  for (const problem of result.problems) console.error(`[export-smoke]   • ${problem}`);
  console.error(
    `[export-smoke] résumé : ${result.entities ?? 0} entité(s), ` +
      `${errors} erreur(s) console/page, ${result.networkRequests.length} requête(s) réseau non-file://`,
  );
}

// `--json` doit rester accessible à la borne dure de timeout.
const parsed = parseArgs(argv);
const JSON_OUTPUT = Boolean(parsed.json);

async function main() {
  if (parsed.error) usageError(parsed.error, JSON_OUTPUT);
  if (parsed.help) {
    // L'aide documente l'usage ; code 2 (aucun test exécuté).
    process.stdout.write(USAGE);
    process.exit(2);
  }
  if (!parsed.file) usageError('chemin du fichier HTML manquant', JSON_OUTPUT);

  const filePath = path.resolve(process.cwd(), parsed.file);
  if (!existsSync(filePath)) usageError(`fichier introuvable : ${filePath}`, JSON_OUTPUT);
  if (!statSync(filePath).isFile()) usageError(`ce n'est pas un fichier : ${filePath}`, JSON_OUTPUT);

  let result;
  try {
    result = await runSmoke(filePath, parsed.timeoutMs);
  } catch (err) {
    // Erreur d'environnement (navigateur indisponible, sandbox…) : exit 2.
    const message = String(err?.message ?? err).split('\n')[0];
    if (JSON_OUTPUT) {
      const failed = { ...emptyResult(), problems: [message] };
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
