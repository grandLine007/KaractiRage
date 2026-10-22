// KaractiRage — pixel-art archétype Boss (arène). Palette Dark Atelier.
// Build : node tools/pxl/run.js build assets/art/boss.px.js --out assets/generated

pal({
  k: "#0d0819", t: "#1e293b", w: "#f8fafc", y: "#f59e0b", Y: "#fef08a",
  o: "#f97316", b: "#38bdf8", B: "#0ea5e9", g: "#34d399", G: "#10b981",
  p: "#a855f7", P: "#7e22ce", r: "#ef4444", R: "#450a0a", s: "#94a3b8",
  S: "#475569", n: "#b45309", N: "#7c2d12",
})

// --- Titan & éléments -----------------------------------------------------------

sprite("boss_titan", { outline: "k" }, `
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

sprite("boss_core", { outline: "k" }, `
  ..rrrr..
  .rrrrrr.
  rrrrrrrr
  rrrwwrrr
  rrrwwrrr
  rrrrrrrr
  .rrrrrr.
  ..rrrr..
`)

sprite("boss_projectile", { outline: "k" }, `
  ..ooo...
  .ooooo..
  oooooo..
  ooYYoo..
  ooYYoo..
  oooooo..
  .ooooo..
  ..ooo...
`)

sprite("boss_orb", { outline: "k" }, `
  ..gggg..
  .gggggg.
  gggggggg
  ggwwgggg
  ggwwgggg
  gggggggg
  .gggggg.
  ..gggg..
`)

// --- Tuiles & décors (sans contour) ---------------------------------------------

sprite("boss_arena_floor", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "R")
  c.dither(0, 0, 16, 16, "R", "S", 0.15)
  c.box(0, 0, 16, 16, "t")
})

sprite("boss_arena_wall", { w: 16, h: 16 }, (c) => {
  c.rect(0, 0, 16, 16, "P")
  c.box(0, 0, 16, 16, "t")
  c.rect(0, 0, 16, 3, "p")
  c.dither(0, 3, 16, 13, "P", "p", 0.18)
})

sprite("boss_crack", { w: 16, h: 16 }, (c) => {
  c.line(2, 2, 6, 5, "k", 1)
  c.line(6, 5, 4, 9, "k", 1)
  c.line(6, 5, 10, 7, "k", 1)
  c.line(10, 7, 13, 12, "k", 1)
  c.line(4, 9, 8, 13, "k", 1)
})

// --- Atlas & animation ----------------------------------------------------------

sheet("boss_chars", ["boss_titan", "boss_core", "boss_projectile", "boss_orb", "boss_arena_floor", "boss_arena_wall", "boss_crack"], { pad: 1 })

anim("boss_shockwave", { w: 16, h: 16, frames: 8, fps: 12 }, (c) => {
  const r = Math.round(c.phase * 7)
  c.ring(7, 7, r, "p")
  c.ring(7, 7, Math.max(0, r - 2), "r")
})
