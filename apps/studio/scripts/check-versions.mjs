// Vérifie que la version est identique partout (source de vérité : Cargo.toml workspace).
//
// Sans ce garde-fou, on obtient trois vérités divergentes : Cargo, package.json et le
// badge de l'interface (« v0.2.0 » codé en dur dans App.tsx avant v0.3.0).
//
// Usage : node scripts/check-versions.mjs   (CI : après `npm run lint`)

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const STUDIO = path.resolve(__dirname, '..');
const ROOT = path.resolve(STUDIO, '../..');

function workspaceVersion() {
  const cargo = readFileSync(path.join(ROOT, 'Cargo.toml'), 'utf8');
  const section = cargo.split(/^\[/m).find(block => block.startsWith('workspace.package]'));
  if (!section) throw new Error('[workspace.package] introuvable dans Cargo.toml');
  const match = section.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error('version absente de [workspace.package]');
  return match[1];
}

function packageJsonVersion() {
  return JSON.parse(readFileSync(path.join(STUDIO, 'package.json'), 'utf8')).version;
}

function tauriConfVersion() {
  return JSON.parse(readFileSync(path.join(STUDIO, 'src-tauri/tauri.conf.json'), 'utf8')).version;
}

function tauriCargoVersion() {
  const cargo = readFileSync(path.join(STUDIO, 'src-tauri/Cargo.toml'), 'utf8');
  if (/\bversion\.workspace\s*=\s*true/.test(cargo)) return 'workspace';
  const match = cargo.match(/^\s*version\s*=\s*"([^"]+)"/m);
  if (!match) throw new Error('version absente de src-tauri/Cargo.toml');
  return match[1];
}

const sources = {
  'Cargo.toml (workspace)': workspaceVersion(),
  'apps/studio/package.json': packageJsonVersion(),
  'apps/studio/src-tauri/tauri.conf.json': tauriConfVersion(),
  'apps/studio/src-tauri/Cargo.toml': tauriCargoVersion(),
};

const expected = sources['Cargo.toml (workspace)'];
const mismatches = Object.entries(sources).filter(([, value]) => value !== expected && value !== 'workspace');

for (const [source, value] of Object.entries(sources)) {
  console.log(`${value === expected || value === 'workspace' ? '✅' : '❌'} ${source}: ${value}`);
}

if (mismatches.length > 0) {
  console.error(`\n❌ Versions divergentes : ${expected} attendu partout (voir docs/VERSIONS.md).`);
  process.exit(1);
}

console.log(`\n✅ Version unique : ${expected}`);
