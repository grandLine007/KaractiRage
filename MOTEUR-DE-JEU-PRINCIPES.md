# Moteur de jeu révolutionnaire — Principes, algorithmes, technologies & plan de construction

> Document de référence. Objectif : un moteur 2D/3D où le **workflow** est la révolution
> (éditer en direct pendant que le jeu tourne, hot-reload total, rollback netcode déterministe),
> pas la checklist graphique classique.

---

## 1. Principes directeurs

### 1.1 Principes non négociables

| # | Principe | Conséquence concrète |
|---|----------|----------------------|
| P1 | **Les données avant les objets** | ECS (Entity Component System) : pas d'héritage profond, pas de `GameObject` monolithique |
| P2 | **Déterminisme de la simulation** | Même entrée → même état, sur toutes les plateformes ; entiers/fixed-point pour la logique de gameplay |
| P3 | **Le moteur EST l'éditeur** | Éditeur, profiler et jeu partagent le même processus et le même ECS |
| P4 | **Hot-reload partout** | Code, scripts, assets, prefabs rechargés à chaud, sans perdre l'état de jeu |
| P5 | **Timestep fixe, rendu variable** | Simulation à 60 Hz figée, interpolation pour l'affichage |
| P6 | **Data-driven** | Tout ce qui n'est pas du code système est un asset (JSON/RON/YAML/TOML) |
| P7 | **Ergonomie développeur > perf brute** | Un profilage sans douleur vaut mieux qu'un 5 % de perf en moins |
| P8 | **Périmètre MVP strict** | Vertical slice jouable en 4-6 semaines ; tout le reste attend |

### 1.2 Principes d'architecture

- **Dépendances en étoile** : le `core` ne dépend de rien ; les couches supérieures dépendent du core, jamais l'inverse.
- **Data-Oriented Design (DOD)** : organiser la mémoire par *type d'accès*, pas par *entité*.
- **POD partout dans la sim** : les composants de gameplay sont trivialment sérialisables (replay, rollback, sauvegarde, réseau).
- **Systèmes purs** : `fn système(world, dt)` — pas d'effets de bord cachés, testable sans renderer.
- **Fail fast, log structurel** : erreurs typées (`thiserror`), logging filtrable (`tracing`), assertions en debug.
- **Les features qui ne servent pas la slice sont refusées.**

---

## 2. Modèle de programme : ECS

### 2.1 Pourquoi ECS et pas OOP

- **Parallélisme gratuit** : deux systèmes qui touchent des composants disjoints s'exécutent en parallèle.
- **Cache-friendly** : colonnes contiguës (archétypes) vs. objets dispersés en heap.
- **Serialization instantanée** : sauvegarde = dump des colonnes ; replays = dump + inputs.
- **Hot-swap** : ajouter un composant à chaud ne casse rien.

### 2.2 Les trois modèles d'ECS (et lequel choisir)

| Modèle | Exemples | Avantage | Inconvénient |
|--------|----------|----------|--------------|
| **Archétypal** (tables) | `bevy_ecs`, `EnTT`, Flecs | Excellente localité, itérations très rapides | Ajout/suppression de composant = migration de table |
| **Slotmap + sparse sets** | `hecs`, `legion` | Ajout/retrait O(1), simple | Itération moins continue |
| **Query-based** | `flecs` (world query) | Flexibilité | Coût par requête |

**Choix retenu : archétypal pour les données chaudes** (transform, render, physique) **+ sparse set pour les données froides** (script state, IA). Implémentation maison ou `bevy_ecs` réutilisé en premier prototype (voir §4).

### 2.3 Briques ECS indispensables

- **Entity** : `u64` généré par génération-index → free-list sûre (pas d'ID réutilisé à tort).
- **Component** : struct POD, `#[derive(Component, Clone, Copy, Serialize, Deserialize)]`.
- **Resource** : singleton global (horloge, RNG graine, config, input).
- **Query** : `Query<(&Transform, &Velocity), Without<Frozen>>`.
- **System schedule** : ordre déclaratif, stages (`pre_update`, `fixed_update`, `update`, `post_update`, `render`).
- **Change detection** : bit "changé ce tick" par composant → le renderer ne ré-upload que ce qui bouge.
- **Events** : file à double buffer (lue ce tick, purgée ensuite).

### 2.4 Schedule et parallélisme

```
                ┌─ input systems ─┐
main thread ────┤                 │
                └─ fixed sim tick ┤
                                  ├─→ physics ─→ collision ─→ gameplay ─→ script
worker pool ──────────────────────┘
render thread ─── extract (état figé) → cull → graph → submit
```

- **Auto-scheduling** : déclarer les accès en lecture/écriture du système → graphe DAG topologique.
- **Parallel executor** : coloration du DAG en lots sans conflit (algo classique de graphe des conflits).
- **Double-buffering** : la sim écrit dans le buffer A pendant que le render lit B (copie minime, zéro lock).

---

## 3. Boucle de jeu et temps

### 3.1 La boucle

```
while running:
    poll_events()                    # entrées
    accumulator += frame_dt
    while accumulator >= TICK:       # TICK = 1/60 s
        fixed_update(TICK)           # sim pure, déterministe
        accumulator -= TICK
    alpha = accumulator / TICK       # 0..1
    render(interpolate(alpha))       # rendu variable
```

- **Piège à éviter** : le *spiral of death* — si `accumulator` explose, la borner (`max_steps`).
- **Piège à éviter** : ne jamais lire l'horloge murale dans la sim → utiliser le tick logique.

### 3.2 Horloge

- `Instant` monotone pour la boucle ; **tick entier** pour la sim.
- Interpolation : `lerp(prev_transform, curr_transform, alpha)` — stocker l'état N-1.
- Pause/step : la sim n'avance que sur commande du debugger.

---

## 4. Technologies open source (par couche)

### 4.1 Noyau & langage

| Techno | Licence | Rôle | Alternative |
|--------|---------|------|-------------|
| **Rust** | MIT/Apache | Langage principal : sécurité mémoire sans GC (GC = pauses imprévisibles) | **Zig** (plus proche du métal), **C++20** |
| **`rustc`/LLVM** | Apache w/ LLVM exceptions | Back-end codegen | — |
| **`wasmtime` / `wasmer`** | Apache/MIT | Runtime de scripting sandboxé | — |

### 4.2 Fenêtre, entrées, plateformes

| Techno | Licence | Rôle |
|--------|---------|------|
| **`winit`** | Apache/MIT | Fenêtre, events souris/clavier/gamepad, DPI |
| **`gilrs`** | MIT | Gamepads (Xbox, DualSense, Switch) multiplateforme |
| **`raw-window-handle`** | MIT/Apache | Passer la fenêtre à wgpu/gfx |

### 4.3 Rendu

| Techno | Licence | Rôle | Notes |
|--------|---------|------|-------|
| **`wgpu`** | Apache/MIT | API unifiée Vulkan/Metal/DX12/WebGPU | Le standard de fait en Rust |
| **`glam`** | MIT | Maths (vec/mat4), SIMD | Alternative : `nalgebra` |
| **`naga`** | Apache/MIT | WGSL → SPIR-V/HLSL/GLSL (shader cross-compilation) | Inclus dans wgpu |
| **`meshopt` / `meshoptimizer`** | MIT | Mesh optimization, LOD (Simplify) | Indispensable |
| **`mikktspace`** | Zlib | Génération de tangentes (normal mapping) | Standard |
| **`bitflags`** | MIT/Apache | Flags de rendu | — |
| **Pipelines** | — | Forward+ / **clustered** pour commencer, différé (deferred) plus tard | Ne pas commencer en deferred |

**Formats d'assets shader/texture** : **WGSL** (shaders), **KTX2 + Basis Universal** (textures compressées universelles), **glTF 2.0** (meshes/scènes).

### 4.4 Assets & hot-reload

| Techno | Licence | Rôle |
|--------|---------|------|
| **`notify`** | CC0/ Apache | Watcher filesystem (hot-reload) |
| **`ron`** (Rusty Object Notation) | Apache/MIT | Formats d'assets typés Rust |
| **`serde` + `bincode`/`postcard`** | MIT/Apache | Sérialisation rapide (deltas réseau, replays) |
| **Content-addressing (Blake3)** | CC0 | Cache d'assets : hash = clé, build incrémental |
| **`gltf`** | MIT | Loader glTF |
| **`image` / `texgen`** | MIT | Décodage images (PNG, EXR, HDR) |
| **`hound`** | ISC | WAV |
| **`symphonia`** | MPL-2.0 | Décodeurs audio (MP3, OGG, FLAC) |

**Pipeline d'assets** : source → *importer* (thread dédié) → *cache* (Blake3) → *loader* (à chaud) → handle typé avec génération (les références deviennent invalides proprement).

### 4.5 Audio

| Techno | Licence | Rôle |
|--------|---------|------|
| **`cpal`** | Apache/MIT | Sortie audio multiplateforme |
| **`rodio`** | Apache/MIT | Mixer haut niveau (ou mixer maison) |
| **FMOD / Wwise** | propriétaire (gratuit sous seuil) | Alternative pro — à ne pas embarquer dès le départ |

Graphe audio : mixer → buses (musique/SFX/UI) → reverb/send → master. Spatialisation **HRTF** (`spline interpolation` sur le campo).

### 4.6 Physique & collision

| Techno | Licence | Rôle |
|--------|---------|------|
| **`parry2d`/`parry3d`** | Apache-2.0 | Collisions, détection, queries (sorte de Rapier sans la dynamique) |
| **`rapier2d`/`rapier3d`** | Apache-2.0 | Dynamique complète (basée sur parry) |
| **`nalgebra`** | Apache/BSD | Maths pour rapier |
| **`salva2d/3d`** | Apache | Fluides (plus tard) |

**Alternative maison pour le MVP** : cercles/AABB + solver de vitesse (impulse), CDT pour les formes concaves. Souvent suffisant pour un premier jeu, et ça force à comprendre le domaine.

### 4.7 Réseau (rollback)

| Techno | Licence | Rôle |
|--------|---------|------|
| **`quinn`** (QUIC) | MIT/Apache | Transport fiable/absolu moderne |
| **`laminar`** / **`renet`** | MIT | UDP semi-fiable (reliability layer) |
| **`snow`** / **Noise protocol** | MIT | Chiffrement (optionnel) |
| **`binnacle`/`varint`** | — | Compression delta (varint, quantification) |
| **`ggrs`** | MIT | **Rollback P2P prêt à l'emploi** (style GGPO) — forte candidate |

### 4.8 Scripting & gameplay

| Techno | Licence | Rôle | Notes |
|--------|---------|------|-------|
| **`mlua`** (Lua 5.4/LuaJIT) | MIT | Scripting léger, hot-reload | Le plus pragmatique |
| **`rhai`** | MIT | Script type Rust, sandbox natif | Alternative sans FFI |
| **`wasmtime` + WASI** | Apache | Scripts en Rust/C/C++ sandboxés | Perf max, isolation max |
| **`unreal-reflection`-like** | — | Génération de bindings Rust↔script via macro | `#[derive(Script)]` |

**Choix** : **Lua (`mlua`)** pour le gameplay + **WASM** pour les modules lourds (IA, procédural).

### 4.9 IA & navigation

- **`hecs`/membres du schedule** pour les systèmes d'IA.
- **Comportements** : state machines → **GOAP** (Goal-Oriented Action Planning) → **behavior trees**.
- **Navigation** : **Recast + Detour** (`recast` crates, licence Zlib) — standard du mesh nav.
- **Pathfinding** : **A\*** sur grile / **JPS** (Jump Point Search) ; **flow fields** pour les foules (Dijkstra multi-sources inversé).
- **Steering** : arrive, separation, flocking (Boids : cohésion + séparation + alignement).

### 4.10 Outils & outillage

| Techno | Licence | Rôle |
|--------|---------|------|
| **`tracing` + `tracing-subscriber`** | MIT | Logs structurés, spans de perf |
| **`tracy`** (client Rust : `tracy-client`) | BSD-3 | Profiler frame-by-frame, zones instrumentées |
| **`puffin`** | MIT | Alternative légère |
| **`egui`** | MIT/Apache | Éditeur d'outils, HUD debug, inspector ECS |
| **`bevy-inspector-egui`** (ou maison) | MIT | Live inspection des entités |
| **`criterion`** | MIT/Apache | Benchmarks statistiques |
| **`proptest` / `insta`** | MIT | Tests property + golden snapshots (replays !) |

### 4.11 Build & CI

- **`cargo` + workspaces** monorepo (`engine-core`, `engine-render`, `engine-assets`, `editor`, `game`).
- **`cargo-nextest`** : tests parallèles.
- **CI** : build Linux/Windows/macOS + golden tests de sim + benchmarks gardés (thrash detection).
- **Licences** : `cargo-deny` (bannit GPL/propriétaire si incompatible).

---

## 5. Algorithmes essentiels (par domaine)

### 5.1 Maths & transform

- **Linéaire** : vecteurs, matrices 4×4, quaternion pour les rotations (lerp nlerp/slerp).
- **ECS transform hierarchy** : parent/enfant avec propagation de matrice monde → **dirty flags** (ne recalculer que les sous-arbres modifiés).
- **Interpolation** : `lerp`, `smoothstep`, easing (pour le feel).
- **Déterminisme** : pas de `f32` dans la sim critique → **fixed-point Q16.16** ou `f64` + fonctions transcendentales *implémentées maison* (sinon `sin()` diverge entre plateformes et le rollback casse).

### 5.2 Rendu

- **Culling** : frustum culling (plans de la frustum vs AABB), occlusion culling (**HiZ**, puis **portal/occlusion queries** sur GPU).
- **Hiérarchie de rendu** : BVH ou octree pour la culling spatiale.
- **Rendering graph** : dépendances entre passes (shadow → gbuffer → lighting → post) → exécution topologique, aliasing de ressources.
- **Instancing** : batcher par mesh/matériau, **indirect draw** (`draw_indirect`).
- **Éclairage** : PBR (Cook-Torrance, IBL précalculé avec **irradiance map** + **prefiltered env map** + **BRDF LUT**), cascaded shadow maps (CSM).
- **Clustered/Forward+** : grile 3D de clusters (xyz) → listes de lumières par cluster (compute shader).
- **Post** : bloom (downsample/upsample pyramid, dual filtering), tonemapping (ACES), FXAA/SMAA.
- **Particules** : CPU pour le MVP, **GPU-driven** ensuite (simulation en compute, compaction atomique).
- **Skeletal animation** : palettes de matrices par squelette, **anim blending** (cross-fade, additive layers), compression de tracks (quaternion key reduction).

### 5.3 Monde ouvert & streaming

- **Octree / quadtree / loose octree** pour le PVS spatial.
- **Chunks + LOD par distance** ; **virtual texturing** (clipmap) pour les terrains.
- **Coordonnées monde double** : `f32` local + `i32` chunk (évite la précision qui tremble loin de l'origine).

### 5.4 Collision & physique

- **Broad-phase** : **Dynamic AABB Tree** (BDBVH, type Box2D) ou **SAP** (Sweep and Prune).
- **Narrow-phase** : SAT (polygones), GJK + EPA (convexes), détection de cercles/AABBs en O(1).
- **Resolution** : séparation positionnelle + **impulse solver** itératif ; **Sequential Impulses** (Erin Catto).
- **Détection continue** : **CCD** par *swept AABB* ou *conservative advancement*.
- **Déterminisme** : itérations ordonnées par **entity id** (jamais l'ordre de la hashmap), mêmes nombres d'itérations partout.
- **Spatial hash** pour les particules/débris.

### 5.5 Réseau & rollback (le cœur "révolutionnaire")

**Modèle GGPO / rollback :**

1. Chaque joueur envoie ses inputs (tick, buttons) avec **prediction**.
2. Simulation locale **prédit** l'état du futur.
3. Quand les inputs remote arrivent en retard : **resimuler** les ticks sans repli.
4. Pour que ça marche : **rollback-safe** = la sim est une fonction pure `state', = f(state, inputs)`.

**Techniques associées :**

- **Quantification des inputs** : 12 bits par manette (stick 8 bit ×2, boutons bitmask).
- **Delta compression** : XOR avec l'état précédent + varint / *posit* / *zstd*.
- **Input prediction** : last-input-wins ou *side-specific*.
- **Rollback budget** : max 8 ticks de resim ; si dépasse → *delay compensation*.
- **Anti-desync** : hash déterministe de l'état (xxhash/blake3) échangé périodiquement → si divergence, rollback forcé ou resync complet.
- **P2P NAT traversal** : **ICE/STUN** (`webrtc-ice`), fallback serveur relais.

### 5.6 IA

- **A\*** avec heuristiques admissibles (octile/manhattan), **JPS** pour grilles, **flow fields** pour foules.
- **GOAP** : planification par recherche inverse sur un graphe d'actions.
- **HTN** (Hierarchical Task Networks) : plus prédictible que GOAP pour un jeu.
- **Influence maps** : champs de potentiel (BFS décroissant) pour le contrôle territorial.
- **Utility AI** : score continu par action (plus souple qu'un state machine).
- **Navigation** : Recast (build du navmesh) + Detour (path corridor + raycast sur le mesh).

### 5.7 Éditeur & outillage

- **Undo/redo** : **command pattern** avec *inverse* (ou **event sourcing** : l'état = pile des commands).
- **Hot reload de code** : `dylib` en (`libloading`) + migration de l'état ECS (les systèmes sont des ressources versionnées) ; sinon reload des scripts (Lua) qui est infiniment plus simple → **commencer par là**.
- **Détection de dérive** : l'éditeur affiche l'état ECS live (inspector), diff par composant.
- **Replays** : log d'inputs + hash → resimulation bit-à-bit = test de non-régression ultime.

### 5.8 Performance & data-oriented

- **Structure of Arrays (SoA)** pour les composants chauds.
- **SlotMap** pour les entités (pointeurs stables malgré le moves).
- **Job system** : worker pool + tâches dépendantes (**rayon** au départ, maison ensuite pour le contrôle).
- **Cache-friendly** : itérer en colonnes, éviter les `Vec<Box<dyn>>`.
- **Profiling d'abord** : Tracy zones sur chaque système avant toute micro-opti.

---

## 6. Comment je les agencerais (architecture)

### 6.1 Vue en couches

```
┌──────────────────────────────────────────────────────────┐
│  game/            gameplay, scripts Lua, data RON        │
├──────────────────────────────────────────────────────────┤
│  editor/          egui, inspector, undo/redo, viewport    │
├───────────────┬────────────────┬─────────────────────────┤
│  render/      │  physics/      │  audio/                 │
│  wgpu graph   │  rapier ou     │  cpal + mixer           │
│  materials    │  maison        │  buses + spatial        │
├───────────────┴────────────────┴─────────────────────────┤
│  net/           ggrs rollback, quinn/laminar, delta       │
├──────────────────────────────────────────────────────────┤
│  ai/            A*, flow fields, navmesh, utility/HTN     │
├──────────────────────────────────────────────────────────┤
│  assets/        notify, blake3 cache, handles typés       │
├──────────────────────────────────────────────────────────┤
│  script/        mlua + wasmtime, bindings générés        │
├──────────────────────────────────────────────────────────┤
│  core/          ECS, schedule, time, events, RNG, math    │  ← zéro dépendance
└──────────────────────────────────────────────────────────┘
```

**Règle des dépendances** : les flèches ne descendent que vers `core`. `editor` et `game` sont des *clients* du moteur, pas des parties de celui-ci (le moteur tourne sans eux).

### 6.2 Workspaces Cargo

```
engine/
├── crates/
│   ├── core/         # ECS, schedule, time, events, rng (déterministe)
│   ├── math/         # glam wrapper, fixed-point, easing
│   ├── assets/       # pipeline, handles, hot-reload (notify+blake3)
│   ├── render/       # wgpu, graph, materials, passes
│   ├── audio/        # cpal, mixer
│   ├── physics/      # rapier2d/3d ou maison
│   ├── net/          # ggrs + transport + delta
│   ├── script/       # mlua + wasmtime + derive macros
│   ├── ai/           # pathfinding, nav, behaviors
│   ├── editor/       # egui + inspector + undo
│   └── devtools/     # tracing, tracy, console, replays
├── apps/
│   ├── sandbox/      # demo qui tourne (la vertical slice)
│   └── editor-app/   # l'éditeur
└── assets/           # source assets + import settings
```

### 6.3 Le flux de frame (détail)

```
OS events → InputMap (ressource, snapshot par tick)
   ↓
[Fixed ×N]  1. système input_apply
            2. script pre_update
            3. IA (decide → écrit des intentions)
            4. gameplay (intentions → forces)
            5. physics.step  → collision events
            6. script post_physics (réagit aux collisions)
            7. advance tick, hash state (rollback/net)
   ↓
Extract  : copie des données nécessaires au render (double buffer)
Cull     : frustum + HiZ → liste de draws
Graph    : shadow → gbuffer/forward → lighting → post → UI
Submit   : wgpu queue (sur render thread)
   ↓
Net      : envoyer inputs, recevoir, resimuler si rollback
```

### 6.4 Gestion de l'état pour le rollback

```rust
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq)]
struct GameState {           // TOUT le reste est dérivé ou hors-sim
    tick: u32,
    players: [PlayerState; 2],
    projectiles: [Proj; 64],
    rng: u64,                // RNG inclus dans l'état !
}
```
- État ≤ **quelques dizaines de KB** → snapshot à chaque tick ultra-rapide.
- `fn step(state: &mut GameState, inputs: &[Input; N])` : fonction pure.
- Test : `assert_eq!(hash(simulate(replay_a)), hash(simulate(replay_b)))` en CI.

---

## 7. Comment je le construirais (roadmap)

### Phase 0 — Fondations (semaine 1-2)
- [ ] Workspace Cargo + `core` ECS (slotmap d'abord, simple) + schedule basique.
- [ ] `winit` + `wgpu` : triangle animé, clear color, résolution DPI.
- [ ] Boucle à timestep fixe + interpolation.
- [ ] `tracing` + zones Tracy dès le départ.
- [ ] **Sortie** : une fenêtre à 60 fps stable avec ECS visible dans un overlay debug.

### Phase 1 — Vertical slice jouable (semaine 3-6)  ← *le vrai jalon*
- [ ] Input mapping (clavier + manette `gilrs`).
- [ ] Transform hierarchy + camera 2D.
- [ ] Physics maison minimale (cercles, impulse solver) **ou** rapier2d.
- [ ] Assets hot-reload (notify + handles à génération) — **caractéristique #1**.
- [ ] Script Lua (`mlua`) : un comportement de gameplay écrit en script, modifiable en tournant.
- [ ] Éditeur egui intégré : pause, step, inspector des entités, edit des valeurs en live.
- [ ] **Sortie** : un mini-jeu (ex. * asteroids amélioré * ou platformer) jouable, éditable à chaud.

### Phase 2 — Rendu honnête (semaine 7-10)
- [ ] Materials + textures (KTX2/Basis), sprites/meshes, batching/instancing.
- [ ] Culling (frustum → HiZ), render graph.
- [ ] PBR minimal + IBL, CSM si 3D.
- [ ] Particules CPU + audio mixer avec buses.
- [ ] **Sortie** : le slice a l'air propre à 60 fps sur machine modeste (benchmarks `criterion`).

### Phase 3 — Rollback & réseau (semaine 11-14)  ← *la révolution*
- [ ] État sérialisable complet + `fn step` pure + RNG dans l'état.
- [ ] **`ggrs`** : 2 joueurs locaux d'abord (prediction locale), puis P2P.
- [ ] Transport (`laminar`/QUIC), NAT traversal STUN, lobby simple.
- [ ] Hash de sync toutes les N secondes + resync.
- [ ] **Replay system** : enregistrer les inputs → resim bit-à-bit.
- [ ] **Sortie** : jeu 2 joueurs en ligne avec rollback invisible + replays reproductibles en CI.

### Phase 4 — Outils & profondeur (semaine 15-18)
- [ ] Undo/redo (command pattern) dans l'éditeur.
- [ ] Édition de niveau persistante (RON), prefabs.
- [ ] IA : A\*/JPS + navmesh (Recast) + behavior/utility.
- [ ] Animation squelettique + blending.
- [ ] Hot-reload de **code** (dylib + migration d'état) — le plus dur, optionnel.

### Phase 5 — Ouverture (semaine 19+)
- [ ] Multiplateforme (Windows/macOS/Linux d'abord ; `wgpu` aide).
- [ ] Packaging assets, build release, profiling pass.
- [ ] Documentation + exemples = API publique stabilisée.

### Les 3 règles d'exécution
1. **Une slice jouable avant tout** — un moteur non joué n'existe pas.
2. **Mesurer avant d'optimiser** — Tracy ouvre à chaque sprint.
3. **Le déterminisme est un test** — golden replay dans la CI, sinon le rollback ne viendra jamais.

---

## 8. Ce qu'il ne faut PAS réinventer

| ❌ Éviter de coder soi-même | ✅ Utiliser |
|-----------------------------|-------------|
| Backend graphique | `wgpu` (Vulkan/Metal/DX12/WebGPU) |
| Compression textures | **Basis Universal/KTX2** |
| Format de mesh | **glTF 2.0** |
| Audio device | `cpal` |
| Physique 3D complète | `rapier` (ou maison uniquement pour du 2D simple) |
| Rollback netcode | **`ggrs`** (implémentation GGPO éprouvée) |
| Navmesh | **Recast/Detour** |
| Maths SIMD | `glam` |
| Décodeurs média | `symphonia`, `image` |
| Shaders cross-compile | `naga` (via wgpu) |
| Profiler | **Tracy** |

À coder **maison** en revanche : ECS (ou `bevy_ecs`), schedule, pipeline d'assets/hot-reload, render graph, éditeur — c'est *là* que se trouve l'identité du moteur.

---

## 9. Références & lecture

- **Data-Oriented Design** — Mike Acton (CppCon) / *Data-Oriented Design* (Richard Fabian)
- **"ECS back and forth"** — Sander Mertens (Flecs)
- **Rollback** — Tony Zurovec, *Fightin' Physics of Fighting Games* (GGPO); GDC: *".Rollback Networking in Mortal Kombat"*
- **Erin Catto** — GDC physics tutorials (Box2D), *Sequential Impulses*
- **Real-Time Rendering** (4e éd.) — Akenine-Möller et al.
- **GPU Gems 3** — clustered forward+, astuce HiZ culling
- **Catlike Coding** — séries Rendering / Hex Maps (gratuit, très pédagogique)
- **Bevy engine** — code source pour l'archétypal ECS et le schedule
- **wgpu samples** — github.com/gfx-rs/wgpu/tree/master/examples

---

*Dernière mise à jour : plan MVP v1 — cible : slice jouable + rollback en ~14 semaines.*
