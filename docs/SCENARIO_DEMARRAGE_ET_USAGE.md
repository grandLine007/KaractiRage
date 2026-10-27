# Scénario de démarrage & d'utilisation — KaractiRage Studio

> **Version 1.2** · 27/10/2026 · Périmètre : studio desktop **Tauri v2** (React 19 + moteur Rust natif).
> **Révision 1.2** — ajout des parcours Vectoriel & Mesh 3D (§6), carte des fichiers (§8.3), plafond boot 60 s.
> **Périmètre de la machine à états (§4)** : boot moteur + cycle de vie du viewport uniquement.
> Export (§6.J) et IA (§6.I) sont des **parcours**, volontairement hors de ce tableau.

## 1. Les trois promesses (contrat d'expérience)

1. **La fenêtre est le premier pixel.** Rien de bloquant avant elle : le boot moteur
   tourne sur un thread détaché (`run()` dans `apps/studio/src-tauri/src/lib.rs`).
2. **L'UI dit toujours où elle en est.** Jamais de zone morte silencieuse :
   « Préparation du moteur natif », « Compilation GPU… », « Moteur indisponible » + **Réessayer**.
3. **La 3D ne se paie jamais deux fois — aux bascules 2D↔3D.** Un device GPU, une sonde
   par session, un chunk préchauffé ; les bascules utilisent pause/resume, jamais de recréation.
   > ⚠️ **Hors périmètre de cette promesse** : les ateliers plein écran KVL/Pixel démontent
   > le viewport (`App.tsx` rend `KvlGraphEditor`/`PxlArtWorkspace` à sa place), le device
   > GPU est alors libéré puis recréé au retour. Trou connu → **P0-2**.

## 2. Budgets cibles & conditions de mesure

**Conditions de mesure** (sinon les chiffres ne veulent rien dire) :
- build **release** (pas `cargo tauri dev`) ;
- **10 lancements à froid** (processus frais), on retient **p50 et p95** ;
- deux matériaux de référence : Apple **M-series** et **Windows bas de gamme** (iGPU).

| Jalon | Cible | État de la mesure | Marque |
|---|---|---|---|
| Fenêtre visible | ≤ 300 ms (p50) | ⛔ à instrumenter (P0-1) | `shell-mounted` |
| Boot `NativeEngine::init()` | ≤ 1 200 ms (p50) | ✅ mesuré & affiché | `EngineBootStatus.init_ms` |
| Premier pixel 2D | boot + 1 frame | ⛔ à instrumenter (P0-1) | `first-2d-frame` |
| Chunk three.js prêt | ≤ idle (2,5 s) | ⛔ à instrumenter (P0-1) | `three-loaded` |
| 1re frame 3D | ≤ 600 ms après 1re bascule | ⛔ à instrumenter (P0-1) | `renderer-init` / `first-3d-frame` |
| Bascule 2D↔3D | 1 frame, **zéro** réinit GPU | ✅ comptable (compteur `renderer-init`) | absence de 2ᵉ `renderer-init` |
| Bascule 2D↔3D ×10 | FPS stable **±10 %**, mémoire plate | partiel (HUD + DevTools Memory) | — |

> **Terminologie** : « 60 FPS déterministe » est incorrect pour le FPS *affiché*.
> Le **pas de simulation** est déterministe (120 Hz fixe) ; le **FPS affiché** est mesuré.
> On écrira : « Boucle fixe 60 Hz (pas déterministe) • FPS affiché ≥ 55 (HUD) » — cf. P1-4.

## 3. Chronologie nominale (trois pistes parallèles)

```text
T+0        RUST      run() : EngineState::new() + begin_boot() → spawn(thread) → tauri::Builder → fenêtre
T+0..300   WEBVIEW   index.html → main.tsx → <App/>                      [shell-mounted]
                     ├─ HeaderBar + Hiérarchie visibles : l'app est déjà "utilisable"
                     ├─ viewport : spinner « Préparation du moteur natif… »
                     ├─ effet 1 : prewarm3D() appelé au montage (App.tsx:73),
                     │            exécuté en requestIdleCallback (timeout 2500 ms)
                     └─ effet 2 : getEngineStatus() → POLLING 250 ms jusqu'à ready/failed
T+0..300   VIEWPORT  montage GameViewport
                     ├─ invoke('engine_canvas_width') ← attend la fin du boot
                     │   (≈ init_ms en nominal ; plafond 60 s en cas de panne, démarrage à froid) — P0-3
                     └─ detectRenderCapabilities() ← sonde mémoïsée (1/session)
T+0..1200  RUST      locate_scene + load_from_scene + SvgCache + Canvas::new(1200×800)
                     + warmup_glyph_cache                                → finish_boot()
T+init_ms  RUST      finish_boot : init_ms ← elapsed ; error/engine remplis ; booting=false ;
                     **Condvar notify_all → réveille uniquement les commands IPC en attente**
                     (aucun événement Tauri `emit` : le POLLING reste la source de vérité UI)
T+init_ms  VIEWPORT  canvas_width/height répondent → canvas 1200×800, isLoaded=true
                     ├─ boucle 2D : engine_frame(dt) → ImageData → putImageData  [first-2d-frame]
                     └─ télémétrie 1×/10 frames → FPS/CPU/score/statut (affichés au HUD)
T+idle     WEBVIEW   prewarm3D : import('./scene3d') (chunk en cache)      [three-loaded]
T+bascule  VIEWPORT  clic « Box » → view3d=true
                     ├─ 1re fois : startScene3D → new WebGPURenderer → await init() [renderer-init]
                     │            placeholder « Compilation GPU… » (sinon 3d/load_failed)
                     └─ ensuite  : ctl.resume() (0 ms, device conservé)
                     boucle RAF : 1 IPC max en vol, interpolation prev→curr    [first-3d-frame]
T+fermeture RUST    CloseRequested → kill du serveur pxl (pas d'orphelin)   [serveur pxl : §8.4]
                     React unmount → ctl.dispose() → renderer.dispose() + DOM retiré
```

**Perception utilisateur :** fenêtre immédiate, **Hub des Projets** affiché d'emblée
(`isHubOpen = true` au montage), le moteur bootant en arrière-plan derrière l'overlay.
Une fois le Hub fermé (ou un projet choisi), le viewport apparaît : spinner localisé puis
l'image 2D d'un bloc, avec un statut honnête « Moteur KaractiRage Prêt (N ms) » (la mention
« 60 FPS Déterministe » est à réécrire — cf. §2 / P1-4).

## 4. Machine à états — boot moteur + viewport (périmètre délimité)

> Export et IA sont des parcours (§6), volontairement hors de ce tableau (voir §1, note de périmètre).

| État | Déclencheur | Rendu |
|---|---|---|
| `boot/shell` | `<App/>` monté | chrome + spinner viewport |
| `boot/retrying` | clic **Réessayer** | spinner + statut « relance… » (à ajouter, P0-4) |
| `boot/ready` | `engine_canvas_width` OK | frames 2D, télémétrie |
| `boot/failed` | rejet IPC / `state=failed` | panneau « Erreur du moteur » + **Réessayer** |
| `play` | mode par défaut | flux 2D + entrées clavier/souris |
| `edit` | `Tab` / segment | toolbar + hiérarchie + inspecteur + poll 160 ms |
| `kvl` / `pixel` | segment | atelier plein écran (viewport démonté → P0-2) |
| `3d/load_failed` | `import('./scene3d')` rejeté | panneau + **Réessayer** (à ajouter, P0-5) |
| `3d/compiling` | 1re bascule | « Compilation GPU… » |
| `3d/live` | `renderer.init()` OK | scène instanciée + picking |
| `3d/failed` | init GPU KO | « Vue 3D indisponible » + retour 2D |

## 5. Scénarios dégradés & reprise

> Colonne **preuve** : `code` = vérifiable par lecture (fichier:ligne) · `recette` = à
> confirmer par la recette §7 · `à faire` = non implémenté (backlog). Une case sans preuve
> n'est pas une validation.

| Panne | Comportement attendu | État | Preuve |
|---|---|---|---|
| Boot lent (> 1 s) | spinner borné, app navigable, aucun gel UI | fait | `code` (`lib.rs` thread + deadline) |
| Boot échoue | `state=failed` + message + **Réessayer** | fait | `code` (polling `App.tsx`) + `recette` §7.9 |
| Retry après échec | erreur effacée **avant** relance, une seule init | fait | `code` (`engine_retry_boot` + `begin_boot`) |
| Chunk three.js KO | panneau + **Réessayer**, pas de spinner éternel | à faire | P0-5 |
| Init GPU KO | repli WebGL2 puis échec propre + retour 2D | fait | `code` (`scene3d.ts` `await init`) |
| Ni WebGPU ni WebGL2 | badge `LOGICIEL`, 3D refusée proprement | fait | `code` (`capabilities.ts`) |
| Conteneur masqué au resize | pas de division par zéro | fait | `code` (`scene3d.ts` `resize()`) |
| Retour KVL/Pixel | device 3D conservé (pas de recompilation) | **à faire** | P0-2 (promesse §1.3) |
| `unwrap()` sur lock en échec | message compréhensible | à faire | P2-1 |

## 6. Parcours d'utilisation nominal (bout en bout)

**A. Premier lancement** — ouverture sur le **Hub des Projets** (choisir un archétype,
ouvrir un projet ou créer une recette). Le moteur boote en arrière-plan derrière l'overlay :
la scène n'est pas lancée d'emblée à l'écran.
**B. Choisir / créer** — `H` *Hub* : les **Derniers projets** s'affichent en tête de l'onglet
d'accueil (un clic = jouer, boutons Jouer/Ouvrir/Retirer) ; sinon ouvrir un projet du registre,
« Ouvrir la galerie » → **fenêtre dédiée « 5 archétypes de jeu en 1 clic »** (cartes riches :
description, contenus, contrôles, boutons **Démo 2D** / **Démo 3D** — la démo 3D ouvre la vue
RenderPacket sur une scène 3D thématique de l'archétype), `W` *Assistant de recette* → projet jouable + toast, `I` *Intention IA*.
**C. Jouer** — flèches/WASD/ZQSD, `Espace` saut, `E` interaction ; clic gauche = interaction,
glisser-déposer = spawn ; `Pause` fige la boucle ; radar/grille à la demande.
**D. Éditer** — `Tab` : `V` sélection, `B` pinceau, `X` gomme, `T` tuile ; hiérarchie →
sélection ; inspecteur (transform, dupliquer, supprimer) ; `Ctrl+S`, `Ctrl+Z/Y`, `Suppr`.
**E. Logique** — *LOGIQUE KVL* : graphe nodal plein écran (viewport démonté → P0-2).
**F. Pixel Art** — *PIXEL ART* : atelier plein écran (viewport démonté → P0-2).
**G. Vectoriel** — *VECTORIEL* : import d'image PNG → vectorisation native (median-cut +
marching squares + Douglas-Peucker → SVG), ou import SVG direct ; éditeur de courbes par
nœuds (drag&drop, édition de points), sauvegarde du modèle éditable (`assets/vectors/*.vec.json`)
et spawn dans la scène.
**H. 3D** — bouton **Box** : 1re fois « Compilation GPU… » puis scène instanciée ; le **badge
affiche le backend réellement actif** (`GPU WEBGPU` ou `GPU WEBGL2`) — sur macOS (WKWebView),
WebGPU est récent/variable : **WebGL2 est souvent le nominal**. Clic = picking, glisser sur le
plan XZ = déplacement d'entité ; retours 2D↔3D instantanés (device conservé). Palette
**Mesh 3D** : meshes procéduraux (assemblages de primitives) à spawner puis bascule vue 3D.
**I. Boucle de confiance IA** — génération → bandeau « Patch IA en révision » → `Entrée` accepte / `Échap` rejette (rollback).
**J. Export** — bouton export → **un fichier HTML unique autonome** téléchargé : logique +
assets **inline** (aucune dépendance au système de fichiers) ; les polices Google sont
chargées en ligne (repli polices système hors ligne). Ouvert en **`file://` (double-clic)**
sur Chrome/Firefox.
**K. Fermeture** — serveur pxl tué (pas d'orphelin), device 3D libéré.

## 7. Recette d'acceptation (prérequis + 10 étapes)

**Prérequis** : build release · instrumentation **P0-1** posée · injection de panne **P0-4** posée.
Sans eux, les étapes 1, 2, 3 et 9 ne sont pas exécutables.

1. Lancement à froid → **fenêtre ≤ 300 ms (p50)** (marque `shell-mounted`), chrome complet.
2. Statut passe « préparation » → « Prêt (N ms) » sans action (polling, `init_ms` affiché).
3. Première image 2D (`first-2d-frame`) ; **FPS affiché ≥ 55 pendant 10 s** (lecture **HUD**, pas à l'œil).
4. `Tab` → toolbar/hiérarchie/inspecteur < 100 ms ; `Tab` retour → jeu reprend.
5. `H` → ouverture projet registre → scène remplacée.
6. `W` → recette → jeu jouable + toast.
7. **Box** → « Compilation GPU… » puis scène 3D ; **noter le backend réel affiché** (WEBGPU ou WEBGL2) ; clic sélectionne (id affiché).
8. 2D ↔ 3D ×10 → **aucune** recompilation (compteur `renderer-init`), FPS stable ±10 %, mémoire plate (DevTools Memory).
9. Boot forcé en échec (`KARACTI_FORCE_BOOT_FAIL=1`) → panneau + **Réessayer** → état jouable (état `boot/retrying`).
10. Export → **double-clic** le `.html` (file://, Chrome/Firefox) → le jeu tourne (hors polices si offline).

## 8. Annexes

### 8.1 Raccourcis clavier

| Touche | Action |
|---|---|
| `Tab` | bascule JOUER ↔ ATELIER STUDIO |
| `H` / `I` / `W` | Hub projets / Intention IA / Assistant recette |
| `V` `B` `X` `T` | outils : sélection / pinceau / gomme / tuile (mode édition) |
| `Ctrl+S` `Ctrl+Z` `Ctrl+Y` `Suppr` | enregistrer / annuler / rétablir / effacer |
| `Espace` `E` | saut / interaction (jeu) |
| `Entrée` / `Échap` | accepter / rejeter un patch IA |
| flèches · WASD · ZQSD | déplacements (jeu) |

### 8.2 Modes de diagnostic

- `?scene=3d` : tranche verticale 3D seule (`Scene3DView`).
- `?spike=3d` : banc d'essai three.js pur (`Spike3D`).
- Chargés à la volée dans `main.tsx`, sans polluer le bundle du Studio.

### 8.3 Carte des fichiers impliqués

| Fichier | Rôle dans le scénario |
|---|---|
| `apps/studio/src-tauri/src/lib.rs` | `EngineState` (boot différé, `engine_status`, `engine_retry_boot`) |
| `apps/studio/src/main.tsx` | entrées (`App`, `?scene=3d`, `?spike=3d`) |
| `apps/studio/src/App.tsx` | montage + `prewarm3D()` + statut de boot |
| `apps/studio/src/components/GameViewport.tsx` | boucle 2D, keep-alive 3D, panneaux d'erreur |
| `apps/studio/src/render3d/prewarm.ts` | préchauffage 3D en idle |
| `apps/studio/src/render3d/capabilities.ts` | sonde GPU mémoïsée |
| `apps/studio/src/render3d/scene3d.ts` | renderer, init, interpolation, pause/resume |
| `apps/studio/src/services/engineBridge.ts` | `getEngineStatus` / `retryEngineBoot` |
| `apps/studio/src/components/PixelEditor.tsx` / `PxlArtWorkspace.tsx` | atelier Pixel Art (grille, sauvegarde+build) |
| `apps/studio/src/components/VectorWorkspace.tsx` + `services/vectorBridge.ts` | éditeur vectoriel + pont Tauri (import / vectorisation / spawn) |
| `apps/studio/src/components/SpawnPalette.tsx` + `services/archetypes.ts` | palette de spawn 2D/3D/Mesh et galerie d'archétypes |
| `crates/engine-core/src/trace.rs` + `vector.rs` | vectorisation (median-cut, marching squares, Douglas-Peucker) + modèle vectoriel éditable |
| `crates/engine-core/src/mesh_assets.rs` | bibliothèque de meshes 3D procéduraux |

### 8.4 Définitions

- **serveur pxl** : processus Node `tools/pxl/run.js` lancé à la demande pour servir les
  assets générés par l'atelier Pixel Art ; tué à l'arrêt/fermeture (`lib.rs`, `PxlServe`).
- **keep-alive 3D** : renderer three.js monté une fois et conservé (pause/resume) entre
  deux vues 3D, pour ne pas recréer le device GPU.
- **file://** : protocole d'ouverture locale ; OK ici car l'export est un fichier unique
  inline (pas de `fetch`/modules externes côté jeu).

### 8.5 Backlog (suivi)

> IDs stables **P0/P1/P2** à reporter dans l'outil de suivi du chantier (non reliés ici
> volontairement : ce document n'arbitre pas un état de vérité qu'il ne détient pas).

- **P0-1** — Instrumentation `performance.mark/measure` (`shell-mounted`, `first-2d-frame`,
  `three-loaded`, `renderer-init`, `first-3d-frame`). Prérequis de la recette §7.
- **P0-2** — Conserver le device 3D en modes KVL/Pixel (viewport monté-masqué). Rend vraie
  la promesse §1.3 au-delà des bascules 2D↔3D.
- **P0-3** — `engine_canvas_width/height` sans verrou (constantes 1200×800) : sortir les
  dimensions du chemin critique, seul `engine_frame` attend le moteur.
- **P0-4** — Injection de panne déterministe (`KARACTI_FORCE_BOOT_FAIL=1`) + état `boot/retrying`.
- **P0-5** — État `3d/load_failed` (échec `import('./scene3d')`) + bouton **Réessayer**.
- **P1-1** — Événement Tauri `engine_status_changed` (emit au `finish_boot`) pour remplacer le polling.
- **P1-2** — Gater `prewarm3D()` sur `first-2d-frame` (aujourd'hui : au montage, en idle).
- **P1-3** — `backgroundColor: "#07040d"` dans `tauri.conf.json` (anti-flash blanc).
- **P1-4** — Réécrire la chaîne de statut « 60 FPS Déterministe » → « Boucle fixe 60 Hz ».
- **P2-1** — Exposer les erreurs de `EngineState::lock()` en `Result` (fin des `unwrap()` paniquants).
