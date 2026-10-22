// KaractiRage — pixel-art archétype Sokoban (puzzle). Palette Dark Atelier.
// Build : node tools/pxl/run.js build assets/art/sokoban.px.js --out assets/generated

pal({
  k: "#0d0819", t: "#1e293b", w: "#f8fafc", y: "#f59e0b", Y: "#fef08a",
  o: "#f97316", b: "#38bdf8", B: "#0ea5e9", g: "#34d399", G: "#10b981",
  p: "#a855f7", P: "#7e22ce", r: "#ef4444", R: "#450a0a", s: "#94a3b8",
  S: "#475569", n: "#b45309", N: "#7c2d12",
})

// --- Joueur & éléments de jeu ---------------------------------------------------

sprite("soko_player", { outline: "k" }, `
  ....yyyyyyyy....
  ....yyyyyyyy....
  ....ywwwwwy.....
  .....wwwww......
  ....wwwwwww.....
  ...bwwwwwwwb....
  ....bbbbbbb.....
  ....bbbbbbb.....
  ....bbbbbbb.....
  ....bbbbbbb.....
  .....bbbbb......
  ....bb...bb.....
  ...bbb...bbb....
  ...bbb...bbb....
  ..bbb.....bbb...
  ................
`)

sprite("soko_crate", { outline: "k" }, `
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

// --- Tuiles & décors (sans contour) ---------------------------------------------

sprite("soko_floor", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "S")
  c.dither(0, 0, 16, 16, "S", "s", 0.1)
  c.box(0, 0, 16, 16, "t")
})

sprite("soko_wall", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "S")
  c.box(0, 0, 16, 16, "t")
  c.rect(0, 0, 16, 4, "s")
  c.dither(0, 4, 16, 12, "S", "s", 0.15)
})

sprite("soko_wall_brick", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "R")
  c.box(0, 0, 16, 16, "S")
  c.hline(0, 4, 16, "S")
  c.hline(0, 8, 16, "S")
  c.hline(0, 12, 16, "S")
  c.vline(8, 0, 4, "S")
  c.vline(4, 4, 4, "S")
  c.vline(12, 4, 4, "S")
  c.vline(0, 8, 4, "S")
  c.vline(8, 12, 4, "S")
})

sprite("soko_plate", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "S")
  c.dither(0, 0, 16, 16, "S", "s", 0.12)
  c.ring(7, 7, 5, "b")
  c.circ(7, 7, 3, "B")
  c.circ(7, 7, 1, "w")
})

sprite("soko_crate_on", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "S")
  c.ring(7, 7, 6, "g")
  c.rect(2, 2, 12, 12, "n")
  c.box(2, 2, 12, 12, "N")
  c.vline(6, 2, 12, "N")
  c.vline(10, 2, 12, "N")
  c.hline(2, 6, 12, "N")
  c.hline(2, 10, 12, "N")
})

sprite("soko_goal", { w: 16, h: 16 }, (c) => {
  c.poly([8, 0, 15, 8, 8, 15, 0, 8], "g")
  c.poly([8, 3, 11, 8, 8, 12, 5, 8], "G")
  c.circ(8, 8, 1, "w")
})

// --- Atlas & animation ----------------------------------------------------------

sheet("soko_chars", ["soko_player", "soko_crate", "soko_floor", "soko_wall", "soko_wall_brick", "soko_plate", "soko_crate_on", "soko_goal"], { pad: 1 })

anim("soko_goal_pulse", { w: 16, h: 16, frames: 6, fps: 6 }, (c) => {
  const r = 4 + Math.round(Math.abs(Math.sin(c.phase * Math.PI * 2)) * 2)
  c.rect(0, 0, 16, 16, "S")
  c.ring(7, 7, r, "b")
  c.circ(7, 7, Math.max(1, r - 2), "B")
})
