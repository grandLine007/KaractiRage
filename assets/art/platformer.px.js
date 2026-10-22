// KaractiRage — pixel-art archétype Platformer. Palette Dark Atelier.
// Build : node tools/pxl/run.js build assets/art/platformer.px.js --out assets/generated

pal({
  k: "#0d0819", t: "#1e293b", w: "#f8fafc", y: "#f59e0b", Y: "#fef08a",
  o: "#f97316", b: "#38bdf8", B: "#0ea5e9", g: "#34d399", G: "#10b981",
  p: "#a855f7", P: "#7e22ce", r: "#ef4444", R: "#450a0a", s: "#94a3b8",
  S: "#475569", n: "#b45309", N: "#7c2d12",
})

// --- Héros & ennemis ----------------------------------------------------------

sprite("plat_hero", { outline: "k" }, `
  ....rrrrrrrr....
  ...rrrrrrrrrr...
  ...rrrrrrrrrr...
  ....wwwwwwww....
  ...wwwwwwwwww...
  ....wwwwwwww....
  ....bwwwwwwb....
  ...bbbbbbbbbb...
  ...bbbbbbbbbb...
  ....bbbbbbbb....
  ....bbBBBBbb....
  ....bbBBBBbb....
  .....bbbbbb.....
  ....bb....bb....
  ...bbb....bbb...
  ..bbb......bbb..
`)

sprite("plat_enemy", { outline: "k" }, `
  ................
  .....rrrrrr.....
  ...rrrrrrrrrr...
  ..rrrrrrrrrrrr..
  .rrrrrrrrrrrrrr.
  .rrrrrrrrrrrrrr.
  .rrrwwrrrrwwrrr.
  .rrrrrrrrrrrrrr.
  .rrrrrrrrrrrrrr.
  .rrrrrrrrrrrrrr.
  .rrrrrrrrrrrrrr.
  ..rrrrrrrrrrrr..
  ...rrrrrrrrrr...
  ..r.rr.rr.rr.r..
  ................
  ................
`)

// --- Décors & objets -----------------------------------------------------------

sprite("plat_coin", { outline: "k" }, `
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

sprite("plat_spike", { outline: "k" }, `
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

sprite("plat_crate", { outline: "k" }, `
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

sprite("plat_spring", { outline: "k" }, `
  ................
  ................
  ................
  ..ssssssssssss..
  ..ssssssssssss..
  ..ssssssssssss..
  ...s........s...
  ...s........s...
  ...s........s...
  ..ssssssssssss..
  ..ssssssssssss..
  .s............s.
  .s............s.
  .ss..........ss.
  ................
  ................
`)

sprite("plat_flag", { outline: "k" }, `
  ..ss............
  ..ss..yyyyyyyy..
  ..ss..yyyyyyyy..
  ..ss..wwwwwwww..
  ..ss..wwwwwwww..
  ..ss..yyyyyyyy..
  ..ss..yyyyyyyy..
  ..ss..wwwwwwww..
  ..ss..wwwwwwww..
  ..ss............
  ..ss............
  ..ss............
  ..ss............
  ..ss............
  ..ss............
  ................
`)

sprite("plat_cloud", { outline: "k" }, `
  ................
  ................
  ......wwww......
  ....wwwwwwww....
  ..wwwwwwwwwwww..
  .wwwwwwwwwwwwww.
  .wwwwwwwwwwwwww.
  .wwwwwwwwwwwwww.
  .wwwwwwwwwwwwww.
  ..wwwwwwwwwwww..
  ...wwwwwwwwww...
  ................
  ................
  ................
  ................
  ................
`)

// --- Tuiles (sans contour) -----------------------------------------------------

sprite("plat_platform", { w: 32, h: 16 }, (c) => {
  c.rect(0, 0, 32, 16, "n")
  c.rect(0, 0, 32, 4, "g")
  c.rect(0, 4, 32, 2, "G")
  c.noise(0, 0, 32, 16, "N", 0.08, 1)
  c.box(0, 0, 32, 16, "N")
})

sprite("plat_ground", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "n")
  c.dither(0, 0, 16, 16, "n", "N", 0.2)
  c.box(0, 0, 16, 16, "N")
})

// --- Atlas & animation ----------------------------------------------------------

sheet("plat_chars", ["plat_hero", "plat_enemy", "plat_coin", "plat_spike", "plat_crate", "plat_spring", "plat_flag", "plat_cloud"], { pad: 1 })

anim("plat_coin_spin", { w: 16, h: 16, frames: 8, fps: 10, outline: "k" }, (c) => {
  const rx = Math.max(2, Math.round(Math.abs(Math.cos(c.phase * Math.PI * 2)) * 7))
  c.ellipse(7, 7, rx, 7, "y")
  c.ellipse(7, 7, Math.max(1, rx - 2), 5, "Y")
})
