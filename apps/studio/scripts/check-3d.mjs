// Vérification d'interface du viewport 3D — exécute scene3d, le picking et le gizmo
// sur un paquet de rendu réel fourni par le mock (aucune fenêtre Tauri requise).
//
// Usage : npm run dev (port 5173), puis `node scripts/check-3d.mjs`.

import { chromium } from 'playwright-core';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const ici = dirname(fileURLToPath(import.meta.url));

const browser = await chromium.launch({ channel: 'chrome', headless: true });
const page = await browser.newPage({ viewport: { width: 1440, height: 900 } });
page.on('pageerror', e => console.log('[pageerror]', e.message.slice(0, 200)));

await page.addInitScript({ path: join(ici, 'mock-ipc.js') });
await page.goto('http://localhost:5173/');
await page.waitForTimeout(2500);

const echecs = [];
const verifie = (nom, condition) => {
  console.log(`${condition ? '✓' : '✗'} ${nom}`);
  if (!condition) echecs.push(nom);
};

// 1. Ferme le Hub et passe en mode Créer, puis bascule en vue 3D.
await page.keyboard.press('Escape');
await page.waitForTimeout(400);
await page.getByRole('button', { name: /^Créer/ }).first().click();
await page.waitForTimeout(500);
await page.getByRole('button', { name: /vue 3D/ }).click();
await page.waitForTimeout(2500);

verifie('canvas three.js présent', await page.locator('canvas').count() > 0);

// 2. Picking : clic au centre de la scène → l'entité 1 est sélectionnée.
const boite = await page.locator('canvas').first().boundingBox();
await page.mouse.click(boite.x + boite.width / 2, boite.y + boite.height / 2);
await page.waitForTimeout(600);
const selection = await page.evaluate(() => window.__scene3dDebug?.selection?.() ?? null);
verifie('clic centre → entité 1 sélectionnée', selection === 1);
const texteSelection = await page.evaluate(() => {
  const tous = [...document.querySelectorAll('span')].map(el => el.textContent ?? '').filter(t => t.includes('Sélection'));
  return tous.join(' | ');
});
console.log('  [debug] badges trouvés :', texteSelection || '(aucun)');
verifie('badge Sélection #1', texteSelection.includes('#1'));

// 3. Gizmo : les poignées apparaissent avec une sélection.
const gizmoVisible = await page.evaluate(() => window.__scene3dDebug?.gizmoVisible?.() ?? false);
verifie('poignées du gizmo visibles', gizmoVisible === true);

await browser.close();
if (echecs.length > 0) {
  console.log(`\n${echecs.length} échec(s) : ${echecs.join(' ; ')}`);
  process.exit(1);
}
console.log('\nTout est vert.');
