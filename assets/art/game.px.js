// KaractiRage — pixel-art des prefabs de spawn (palette Dark Atelier).
// Construit avec : node tools/pxl/run.js build assets/art/game.px.js --out assets/generated

pal({
  k: "#0d0819", // contour / noir atelier
  t: "#1e293b", // bleu nuit
  w: "#f8fafc", // blanc éclat
  y: "#f59e0b", // or
  Y: "#fef08a", // or clair
  o: "#f97316", // orange magma
  b: "#38bdf8", // cyan / bleu ciel
  B: "#0ea5e9", // bleu
  g: "#34d399", // vert émeraude
  G: "#10b981", // vert sombre
  p: "#a855f7", // violet mystique
  P: "#7e22ce", // violet sombre
  r: "#ef4444", // rouge rubis
  R: "#450a0a", // rouge sombre / obsidienne
  s: "#94a3b8", // gris métal clair
  S: "#475569", // gris métal sombre
  n: "#b45309", // bois / cuivre
  N: "#7c2d12", // bois sombre
})

// --- Collectables / items ---------------------------------------------------

sprite("coin", { outline: "k" }, `
  .....yyyyyy.....
  ...yyyyyyyyyy...
  ..yyyyyyyyyyyy..
  ..yyYyyyyyYyyy..
  .yyYyyyyyyyYyyy.
  .yyYyywwwwyyYyy.
  .yyYyywwwwyyYyy.
  .yyYyywwwwyyYyy.
  .yyYyywwwwyyYyy.
  .yyYyyyyyyyYyyy.
  .yyYyyyyyyyYyyy.
  ..yyyyyyyyyyyy..
  ...yyyyyyyyyy...
  .....yyyyyy.....
`)

sprite("potion", { outline: "k" }, `
  ......ss........
  .....ssss.......
  ......bb........
  ......bb........
  .....bbbb.......
  ....bbrrbb......
  ...bbrrrrbb.....
  ..bbrrrrrrbb....
  ..bbrrrrrrbb....
  ..bbrrrrrrbb....
  ...bbrrrrbb.....
  ....bbrrbb......
  .....bbbb.......
  ......bb........
  .....bbbb.......
`)

sprite("key", { outline: "k" }, `
  ......yyyy......
  ......yyyy......
  ......yyyy......
  ......yyyy......
  ......yyyy......
  ....yyyy........
  ...yyyy.........
  ...yyyy.........
  ...yyyy.........
  ...yyy..........
  ..yyy...........
  ..yyy...........
  ..yyy...........
  ..yyy...........
`)

// --- Coffre & caisse ---------------------------------------------------------

sprite("chest", { outline: "k" }, `
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnYYnnnnYYnn..
  ..nnYYnnnnYYnn..
  ..nnnnnnnnnnnn..
  ..nnnNNNNNNnnn..
  ..nnNNNNNNNNnn..
  ..nnNNwwwwNNnn..
  ..nnNNwwwwNNnn..
  ..nnNNNNNNNNnn..
  ..nnNNNNNNNNnn..
  ..nnnNNNNNNnnn..
  ..nnnnnnnnnnnn..
`)

sprite("crate", { outline: "k" }, `
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nNnnnnnnnnNn..
  ..nNnnnnnnnnNn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nNnnnnnnnnNn..
  ..nNnnnnnnnnNn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
`)

// --- Porte -------------------------------------------------------------------

sprite("door", { outline: "k" }, `
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnNNnnnnNNnn..
  ..nnNNnnnnNNnn..
  ..nnNNnnnnNNnn..
  ..nnNNnnnnNNnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
  ..nnnnnnnnnnnn..
`)

// --- Dalle de déclenchement ---------------------------------------------------

sprite("plate", { outline: "k" }, `
  ..ssssssssssss..
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  .sggggggggggggs.
  ..ssssssssssss..
`)

// --- Danger ------------------------------------------------------------------

sprite("spikes", { outline: "k" }, `
  .s...s...s...s..
  .ss..ss..ss..ss.
  .sss.sss.sss.sss
  ssssssssssssssss
  .ssssssssssssss.
  ..SSSSSSSSSSSS..
  ..SSSSSSSSSSSS..
  ..SSSSSSSSSSSS..
  ..SSSSSSSSSSSS..
  ..SSSSSSSSSSSS..
`)

// --- Boss --------------------------------------------------------------------

sprite("boss", { outline: "k" }, `
  ..RR........RR..
  ..RRR......RRR..
  ..RRRR....RRRR..
  ..RRRRRRRRRRRR..
  ..RRRRRRRRRRRR..
  ..RRrRRRRRRrRR..
  ..RRrRRRRRRrRR..
  ..RRRRrrrrRRRR..
  ..RRRRrrrrRRRR..
  ..RRRRRRRRRRRR..
  ...RRRRRRRRRR...
  ...RRRRRRRRRR...
  ....RRRRRRRR....
  ....RRRRRRRR....
  ...RRRR..RRRR...
  ..RRRRR..RRRRR..
`)



// --- Atlas & animation --------------------------------------------------------

sheet("chars", ["coin", "potion", "key", "chest", "door", "platform", "spikes", "crate", "plate", "boss", "wall"], { pad: 1 })

// Pièce qui tourne sur elle-même (ellipse dont la largeur oscille).
anim("coin_spin", { w: 16, h: 16, frames: 8, fps: 10, outline: "k" }, (c) => {
  const rx = Math.max(2, Math.round(Math.abs(Math.cos(c.phase * Math.PI * 2)) * 7))
  c.ellipse(7, 7, rx, 7, "y")
  c.ellipse(7, 7, Math.max(1, rx - 2), 5, "Y")
})

// --- Plateforme (tuile 32x16, étirée horizontalement par le moteur) ----------

sprite("platform", { outline: "k" }, `
  .ggggggggggggggggggggggggggggggg.
  .ggggggggggggggggggggggggggggggg.
  .ggggggggggggggggggggggggggggggg.
  .GGggggggggggggggggggggggggggggG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
  .GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGG.
`)

// --- Mur (tuile 32x32) ---------------------------------------------------------

sprite("wall", { outline: "k" }, `
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .SSssssssssssssssssssssssssssssS.
  .SSssssssssssssssssssssssssssssS.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .SSssssssssssssssssssssssssssssS.
  .SSssssssssssssssssssssssssssssS.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .SSssssssssssssssssssssssssssssS.
  .SSssssssssssssssssssssssssssssS.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
  .sssssssssssssssssssssssssssssss.
`)
