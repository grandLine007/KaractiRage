// parseur/écrivain minimal pour les sources .px.js (pxl).
// Gère ce dont l'éditeur a besoin : pal({...}), sprite("id", {opts}, `grille`)
// et la détection des sprites procéduraux (sprite(..., (c) => {...})) en lecture seule.

export interface PixelPalette {
  key: string;
  hex: string;
}

export interface SpriteDef {
  id: string;
  kind: 'grid' | 'procedural';
  opts: string;   // texte brut des options (ex. `{ outline: "k" }`) ou ''
  grid: string[]; // lignes (kind === 'grid'), sinon []
  raw: string;    // bloc source complet
  start: number;  // offset dans la source (-1 = nouvel asset non encore écrit)
  end: number;
}

export interface ParsedArt {
  palettes: PixelPalette[];
  sprites: SpriteDef[];
}

const GRID_RE = /sprite\(\s*"([^"]+)"\s*(?:,\s*(\{[^{}]*\}))?\s*,\s*`([\s\S]*?)`\s*\)/g;
const PROC_RE = /sprite\(\s*"([^"]+)"\s*(?:,\s*(\{[^{}]*\}))?\s*,\s*\(c\)\s*=>[\s\S]*?\n\}\)/g;

function parsePalettes(source: string): PixelPalette[] {
  const m = /pal\(\{([\s\S]*?)\}\)/.exec(source);
  if (!m) return [];
  const out: PixelPalette[] = [];
  const re = /([A-Za-z0-9])\s*:\s*"([#0-9a-fA-F]{3,8})"/g;
  let mm: RegExpExecArray | null;
  while ((mm = re.exec(m[1]))) {
    out.push({ key: mm[1], hex: mm[2] });
  }
  return out;
}

function gridToRows(grid: string): string[] {
  const rows = grid.split('\n');
  while (rows.length && rows[0].trim() === '') rows.shift();
  while (rows.length && rows[rows.length - 1].trim() === '') rows.pop();
  return rows.map(r => r.trim());
}

export function parseArt(source: string): ParsedArt {
  const palettes = parsePalettes(source);
  const sprites: SpriteDef[] = [];
  const taken = new Set<number>();

  for (const m of source.matchAll(GRID_RE)) {
    const idx = m.index!;
    taken.add(idx);
    sprites.push({
      id: m[1],
      kind: 'grid',
      opts: (m[2] ?? '').trim(),
      grid: gridToRows(m[3]),
      raw: m[0],
      start: idx,
      end: idx + m[0].length,
    });
  }
  for (const m of source.matchAll(PROC_RE)) {
    const idx = m.index!;
    if (taken.has(idx)) continue;
    sprites.push({
      id: m[1],
      kind: 'procedural',
      opts: (m[2] ?? '').trim(),
      grid: [],
      raw: m[0],
      start: idx,
      end: idx + m[0].length,
    });
  }
  sprites.sort((a, b) => a.start - b.start);
  return { palettes, sprites };
}

export function serializePalettes(palettes: PixelPalette[]): string {
  return `pal({\n${palettes.map(p => `  ${p.key}: "${p.hex}"`).join(',\n')}\n})`;
}

export function serializeGridSprite(id: string, opts: string, grid: string[]): string {
  const w = grid.reduce((m, r) => Math.max(m, r.length), 0);
  const rows = grid.map(r => r.padEnd(w, '.'));
  const optsText = opts ? `, ${opts}` : '';
  return `sprite("${id}"${optsText}, \`\n${rows.join('\n')}\n\`)`;
}

export function replacePalBlock(source: string, palettes: PixelPalette[]): string {
  const re = /pal\(\{[\s\S]*?\}\)/;
  const m = re.exec(source);
  if (!m) {
    return serializePalettes(palettes) + '\n\n' + source;
  }
  return source.slice(0, m.index) + serializePalettes(palettes) + source.slice(m.index + m[0].length);
}

export function buildSource(original: string, palettes: PixelPalette[], sprites: SpriteDef[]): string {
  const grid = sprites.filter(s => s.kind === 'grid');
  // Retire tous les blocs grille existants (offsets du source d'origine, fin → début).
  let src = original;
  const existing = grid.filter(s => s.start >= 0).sort((a, b) => b.start - a.start);
  for (const s of existing) src = src.slice(0, s.start) + src.slice(s.end);
  // Réinsère tous les blocs grille (dans l'ordre) à la position du premier.
  const firsts = grid.filter(s => s.start >= 0).map(s => s.start);
  const insertAt = firsts.length ? Math.min(...firsts) : src.length;
  const blocks = grid.map(s => serializeGridSprite(s.id, s.opts, s.grid));
  const insert = blocks.join('\n\n') + (blocks.length ? '\n\n' : '');
  src = src.slice(0, insertAt) + insert + src.slice(insertAt);
  return replacePalBlock(src, palettes);
}

export function newSpriteBlock(id: string, w = 16, h = 16): string {
  const rows = Array.from({ length: h }, () => '.'.repeat(w));
  return serializeGridSprite(id, '{ outline: "k" }', rows);
}

export function newArtSource(name: string): string {
  const pal = `pal({\n  k: "#0d0819", t: "#1e293b", w: "#f8fafc", y: "#f59e0b", Y: "#fef08a",\n  o: "#f97316", b: "#38bdf8", B: "#0ea5e9", g: "#34d399", G: "#10b981",\n  p: "#a855f7", P: "#7e22ce", r: "#ef4444", R: "#450a0a", s: "#94a3b8",\n  S: "#475569", n: "#b45309", N: "#7c2d12",\n})`;
  return `// KaractiRage — source pixel-art "${name}". Palette Dark Atelier.\n// Build : node tools/pxl/run.js build assets/art/${name}.px.js --out assets/generated\n\n${pal}\n\n${newSpriteBlock('sprite_01')}\n`;
}
