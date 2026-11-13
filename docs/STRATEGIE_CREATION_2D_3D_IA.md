# Stratégie — création et modification de jeu 2D / 2.5D / 3D avec IA

> **Statut** : proposition de cadrage — 10 novembre 2026. À trancher avant toute
> nouvelle fonctionnalité.
> **Contexte** : le socle IA inline (plan → ops → validation → preview → undo) et la barre
> d'édition ancrée existent et sont verts. Le risque n'est plus technique, il est de
> **dispersion** : vouloir un modeleur 3D, un générateur d'images, un moteur audio et un
> studio complet en même temps.

---

## 1. Diagnostic honnête

Ce qui marche déjà, et qu'il faut cesser de rouvrir :

| Brique | État |
|---|---|
| Moteur 2D logiciel déterministe (60 FPS low-end) | stable, 406 tests |
| KVL (graphe, exécution, validation catalogue) | stable |
| Boucle IA contrôlée (plan → op → clone → rollback) | stable, 31 ops |
| Édition IA ciblée (contexte, cible, proposition, transaction) | stable |
| Studio 2D : viewport, inspecteur, hiérarchie, HUD, export HTML5 | stable |
| Prévisualisation 3D (three.js dans le Studio) | existe, mais **sans outils d'édition 3D** |

Ce qui manque, par ordre d'impact réel :

1. **La chaîne d'assets.** Aucun pipeline image/son/3D : pas de manifeste, pas
   d'import, pas de budget mémoire, pas de provenance.
2. **Le niveau « jeu complet ».** On sait faire une scène ; pas un jeu avec menus,
   progression, sauvegarde, audio, export desktop.
3. **L'outillage 3D.** Il n'y a pas de caméra d'édition, pas de gizmo, pas de
   placement assisté, pas d'import glTF réel.
4. **L'évaluation du petit modèle.** On ne mesure pas la qualité des ops produites
   par un 7B branché sur Ollama/LM Studio.

**Conclusion du diagnostic** : le prochain chantier n'est pas « plus d'IA », c'est
**la chaîne d'assets et le niveau de complexité**. Sans assets, l'IA n'a rien à
assembler ; sans niveau « jeu complet », l'export n'a rien à prouver.

---

## 2. Le principe directeur : le Studio n'est pas un modeleur 3D

C'est la décision structurante, et elle rejoint ton intuition.

> **KaractiRage est un assembleur de scène et un éditeur de logique. Le contenu 3D
> naît ailleurs (Blender, bibliothèques CC0, générateurs), et entre par un pipeline
> d'assets versionné. L'IA orchestre l'assemblage, jamais la modélisation.**

Trois conséquences :

- on ne code **aucun** outil de sculpture, de retopologie, de rigging, de shader ;
- Blender reste l'atelier 3D, piloté par MCP pour les tâches répétitives et
  l'export automatisé ;
- le moteur ne consomme que du **glTF validé + un manifeste**, ce qui rend le
  runtime indépendant de l'outil qui a produit l'asset.

C'est aussi la seule façon de tenir la contrainte low-end : un asset non validé
(200 Mo de textures 4K) doit être refusé à l'import, pas découvert en jeu.

---

## 3. La chaîne de valeur, et ce que l'IA doit y faire

```text
ASSET ──────► SCÈNE ──────► LOGIQUE ──────► EXPORT
(images, 3D,   (placement,    (KVL,          (HTML5,
 sons, anims)   transforms,    conditions,    desktop,
                lumières)      variables)     wasm)
```

| Étape | Sans IA | Avec IA utile | Ce que l'IA ne doit pas faire |
|---|---|---|---|
| ASSET | import, manifeste, budget | décrire ce qu'on veut, choisir un archétype, générer un sprite pxl, lancer un job Blender | inventer un asset « à la volée » sans provenance |
| SCÈNE | placement, gizmo | « place 5 pièges le long de ce mur », « aligne ces 3 caisses » | créer du code de rendu |
| LOGIQUE | KVL visuel | « si le joueur a 50 pièces, ouvre la porte » → 3 nœuds + 2 liens | produire des nœuds inconnus du catalogue |
| EXPORT | bouton | pré-vol d'export (assets manquants, budget dépassé), rapport | modifier le jeu à l'export |

**Règle** : l'IA est un **raccourcisseur de gestes**, pas un générateur de contenu
opaque. Chaque proposition reste un delta d'ops typées, validé, annulable.

---

## 4. Trois niveaux d'assistance, un seul contrat

| Niveau | Déclencheur | Coût modèle | Exemple | Aujourd'hui |
|---|---|---|---|---|
| **N0 — Template** | `⌘K`, ✦ | **0 token** | « rouge », « plus rapide », « supprime » | fait (résolveur, 18 tests) |
| **N1 — Micro-op** | `⌘K` + LLM | 1 appel, 1-3 ops, 1,5-3 k tokens | « rends le boss plus lent mais avec plus de vie » | prêt (prompt micro), **non branché** |
| **N2 — Plan** | `⌘L` | 1 plan, N ops | « crée un niveau 2D avec un boss et 3 pièges » | existe (AiPlan/executor) |
| **N3 — Agent + outils** | tâche longue | boucle, MCP | « importe ce GLB, réduis-le, place-le sur une grille » | **à concevoir** |

Ordre de valeur : N0 couvre 60-80 % des gestes et **ne coûte rien** ; N1 est le
vrai différenciateur pour un petit modèle ; N3 est le plus fragile et doit rester
optionnel.

### 4.1 Ce qui rend un 7B utilisable (et un 4B inutile)

Le petit modèle échoue pour trois raisons, toutes évitables :

1. **Trop de contexte** → il ne reçoit que la cible et 3 voisins (déjà fait).
2. **Trop de liberté de sortie** → schéma JSON imposé par le fournisseur
   (`format` chez Ollama, `response_format` chez LM Studio), pas de texte libre.
3. **Trop d'arithmétique** → il ne calcule jamais « 110 × 1,5 » : le moteur le
   fait, le modèle propose « plus rapide ».

Plancher réaliste : **7-8B, contexte ≥ 8 k**. En dessous, on garde le modèle pour
*classer* une intention (« couleur / vitesse / suppression ») et on laisse le
template exécuter.

### 4.2 Le tableau de bord qu'il faut mesurer

Corpus du plan : 40 instructions × 10 cibles. Métriques : cible correcte ≥ 95 %,
première op valide ≥ 95 %, no-op silencieux = 0, latence médiane < 15 s en local.
**Sans ce harnais, on ne saura pas si un changement de prompt améliore ou dégrade.**

---

## 5. Architecture MCP : un bus d'outils d'authoring, pas un runtime

```text
                    ┌──────────────── Studio (Tauri) ────────────────┐
  Plan / ops  ──►   │  ai_edit  →  ToolBroker  →  MCP client (rmcp)  │
  (validées)        │                 │                             │
                    │                 ├── pack blender   (glTF out)   │
                    │                 ├── pack image    (PNG/atlas)  │
                    │                 ├── pack audio    (OGG/WAV)    │
                    │                 ├── pack video    (clips → img)│
                    │                 └── pack pixel    (pxl/Aseprite)│
                    └───────────────────────────┬───────────────────┘
                                                ▼
                                    assets/manifest.json (hashé)
                                                ▼
                                  Moteur + export souverain
```

### 5.1 Les invariants à ne pas perdre

1. **MCP = authoring seulement.** Aucun outil MCP n'est requis à l'exécution du jeu
   exporté. Pas de cloud, pas de modèle, pas de Blender chez le joueur.
2. **Le LLM ne parle pas à MCP directement.** Il propose une op ; le moteur décide
   si elle déclenche un outil, avec des paramètres typés. Même boucle de confiance
   que `ai_edit`.
3. **Chaque pack est un processus isolé** avec : nom, version, outils, capacités
   déclarées, timeout, et un mode « indisponible » propre. Blender fermé ne doit
   jamais bloquer le Studio.
4. **Tout asset produit entre par le manifeste** : hash, taille, licence, modèle
   ou outil d'origine, paramètres. C'est la condition de la reproductibilité.
5. **Un pack peut être refusé.** Un utilisateur qui n'a pas de GPU doit pouvoir
   désactiver image/vidéo et garder Blender + pixel art.

### 5.2 Le contrat d'un pack (à écrire dans le code, pas dans un doc)

```rust
pub struct ToolPackManifest {
    pub id: String,            // "blender", "comfyui", "aseprite"
    pub version: String,
    pub kind: PackKind,        // Asset3D | Image | Audio | Video | Pixel
    pub tools: Vec<ToolDecl>,  // nom, description, schéma JSON d'entrée/sortie
    pub requires: Vec<String>, // logiciel externe, GPU, port local…
    pub writes_to: AssetScope, // assets/models, assets/sprites, assets/audio
    pub timeout_secs: u64,
}
```

Une op IA ne référence **jamais** un outil par nom libre : elle référence un
`ToolRequest` validé, avec des chemins confinés à `writes_to`.

---

## 6. L'échelle de complexité : 2D → 2.5D → 3D

Chaque niveau doit être **fini et exportable** avant d'ouvrir le suivant.

| Niveau | Contenu | Ce qu'il faut ajouter | Ce qu'on refuse |
|---|---|---|---|
| **L1 — 1 écran, 1 mécanique** | plateforme, collecte | templates, HUD | — |
| **L2 — mini-jeu** | 3-5 écrans, 2-3 mécaniques, audio | transitions, sons, sauvegarde | — |
| **L3 — jeu 2D complet** | menus, progression, boss, inventaire, dialogues | écrans data-driven, export desktop | — |
| **L4 — 2.5D** | visuel 3D, logique 2D | caméra, sprites billboard, hauteur, profondeur | physique 3D |
| **L5 — 3D déclarative** | scènes 3D, lumières, matériaux, glTF | caméra d'édition, gizmo, import/validation glTF | modeleur, rigging |
| **L6 — 3D riche** | animations squelettées, physique 3D | — | **hors scope** (Blender ou moteur externe) |

### 6.1 2.5D : le meilleur rapport valeur/risque

Le moteur possède déjà `transform3d`, `renderable3d`, `camera3d`, `lights` et un
`RenderPacket` consommé par three.js. Le 2.5D consiste à :

- garder la collision, la logique et la physique **2D** (elles sont fiables) ;
- ajouter une hauteur et une profondeur aux entités ;
- rendre le sprite en billboard, trier par profondeur ;
- exposer la caméra (position, cible, FOV) comme un objet éditable au même titre
  qu'une entité.

C'est peu de code, un rendu spectaculaire, et zéro physique 3D à écrire.

### 6.2 3D déclarative : ce qu'on accepte d'écrire

- import glTF (`gltf` 1.4) → validation (taille, textures, matériaux) ;
- placement : position, rotation Euler, échelle — déjà dans `AiOp` ;
- lumières et caméra — déjà dans le modèle ;
- matériaux : couleur/rugosité/métal, puis textures — pas de shader custom ;
- preview three.js dans le Studio, rendu natif simple pour l'export.

Ce qu'on n'écrit pas : shadow maps avancées, PBR complet, animation de squelette,
physique 3D (sauf besoin démontré par un jeu).

---

## 7. Export et souveraineté

| Cible | État | Ce qui manque |
|---|---|---|
| HTML5 autonome | existe | budget d'assets, pré-vol |
| Package `.karacti.json` | existe | version de manifeste |
| Desktop (bundle) | **non supporté** (`export_for_target` renvoie une erreur) | script Tauri/zip |
| WASM | **non supporté** | chaîne `wasm32` (crate `engine-wasm` existe) |

Règles :

1. **Le pré-vol d'export est un gate**, pas un avertissement : textures > budget,
   asset absent du manifeste, script KVL cassé → export refusé avec la liste.
2. **Le binaire exporté ne contient ni IA ni MCP.** Le jeu est le même pour tous.
3. **L'export est reproductible** : même projet + même manifeste → même sortie
   (hors date), vérifié par un test de hash.

---

## 8. Écosystème vérifié (novembre 2026)

### 8.1 Crates Rust à réutiliser

| Crate | Version | Usage | Décision |
|---|---|---|---|
| [`gltf`](https://crates.io/crates/gltf) | 1.4.1 | import/export glTF 2.0 | **prendre** |
| [`meshopt`](https://crates.io/crates/meshopt) | 0.7.0 | compression de maillages | prendre (export) |
| [`image`](https://crates.io/crates/image) | 0.25.10 | décodage PNG/JPEG, atlas | prendre |
| [`rmcp`](https://crates.io/crates/rmcp) | 3.9.0 | SDK MCP officiel en Rust | prendre côté client |
| [`rapier3d`](https://crates.io/crates/rapier3d) | 0.36.1 | physique 3D | **différer** (L6 seulement) |
| [`bevy`](https://crates.io/crates/bevy) | 0.20.1 | référence d'architecture, pas le moteur | ne pas intégrer |

### 8.2 Projets MCP existants (à étudier, pas à réécrire)

| Projet | Étoiles | Ce qu'il prouve | Usage |
|---|---|---|---|
| [mcp-for-blender](https://github.com/ahujasid/mcp-for-blender) | ~33,9 k | pilotage de Blender par LLM, addon + socket | pack `blender` |
| [godot-mcp](https://github.com/Coding-Solo/godot-mcp) / [godot-ai](https://github.com/hi-godot/godot-ai) | ~6,6 k / 3,5 k | un éditeur de jeu exposé en MCP | modèle du pack `scene` |
| [aseprite-mcp](https://github.com/diivi/aseprite-mcp) | ~961 | pixel art piloté par outils | pack `pixel` |
| [Pixelle-MCP](https://github.com/ATH-MaaS/Pixelle-MCP) | ~1,1 k | ComfyUI + MCP + LLM | pack `image`/`video` |
| [LocalAI](https://github.com/mudler/LocalAI) | ~50 k | LLM + image + voix + vidéo en local, API compatible OpenAI | **une** cible de fournisseur |

**Conséquence forte** : plutôt que d'intégrer chaque backend séparément, viser
**un seul contrat « endpoint compatible OpenAI »**. Ollama, LM Studio et LocalAI
deviennent interchangeables, et images/audio/vidéo passent par le même tuyau.

### 8.3 Ce que la recherche ne règle pas

- Aucun projet ne fournit un **pipeline d'assets de jeu** (manifeste + budget +
  provenance). C'est à nous de l'écrire, et c'est le vrai actif.
- Les MCP vidéo sont orientés « clip marketing », pas « boucle de sprite jouable ».
  Pour un jeu, la vidéo sert surtout à produire des **planches de frames**.
- Les licences des contenus générés varient selon le modèle : il faut les stocker
  dans le manifeste et les afficher à l'export.

---

### 8.4 Arbitrage de la liste élargie (vérifié sur crates.io, 10/11/2026)

Verdicts : **ADOPT** (on intègre), **DÉJÀ** (présent dans le moteur), **DIFFÉRÉ**
(utile mais pas maintenant), **REJET** (contraire à l'architecture ou mort).

| Crate | Version / date | Verdict | Raison |
|---|---|---|---|
| [`tiled`](https://github.com/mapeditor/rs-tiled) | 0.16.0 / 2026-07 | **ADOPT (P2)** | Importer une carte `.tmx` (Tiled) → entités + collisions + couches. **Le plus gros gain de « création facile »** de la liste : le créateur dessine dans un outil gratuit et répandu. |
| [`gltf`](https://crates.io/crates/gltf) | 1.4.1 / 2024-05 | **ADOPT (P4)** | Parseur standard, stable, 11,7 M téléchargements. Aucune alternative sérieuse. |
| [`kira`](https://github.com/tesselode/kira) | 0.12.6 / 2026-10 | **ADOPT (P2)** | Le moteur **synthétise** ses sons (`tinyaudio`, enum `SoundType`) et ne lit **aucun fichier audio**. Kira apporte lecture OGG/MP3, bus, effets, synchro musicale. |
| [`gilrs`](https://gitlab.com/gilrs-project/gilrs) | 0.11.2 / 2026-05 | **DIFFÉRÉ (L2/L3)** | Aucune manette aujourd'hui (`input.rs` = clavier seulement). Ajout peu coûteux quand le jeu « canapé » arrive. |
| [`image`](https://github.com/image-rs/image) | 0.25.10 / 2026-03 | **DIFFÉRÉ (P2)** | Le moteur ne décode que du PNG (`vector::decode_png`). JPEG/WebP utiles pour les assets importés, mais c'est une grosse dépendance : à mesurer en temps de build. |
| [`parry2d`](https://github.com/dimforge/parry) | 0.31.1 / 2026-09 | **DIFFÉRÉ** | Uniquement si les requêtes précises (raycast vs forme tournée) manquent au `spatial.rs` maison. |
| [`rapier2d`](https://crates.io/crates/rapier2d) / `rapier3d` | 0.36.1 / 2026-10 | **DIFFÉRÉ (L6)** | Le moteur a sa physique AABB déterministe + grille spatiale. Migrer maintenant = régressions et perte du déterminisme de replay. |
| [`wgpu`](https://github.com/gfx-rs/wgpu) | 30.0.1 / 2026-08 | **REJET (pour l'instant)** | Remplacerait le rendu logiciel, contre le budget low-end et le `RenderPacket`. La 3D est déjà rendue par three.js dans le Studio. |
| [`ash`](https://github.com/ash-rs/ash) | 0.38.0 / 2024-04 | **REJET** | Vulkan brut : encore plus loin de nos contraintes. |
| [`winit`](https://github.com/rust-windowing/winit) | 0.31.0-beta.4 | **DÉJÀ (0.29)** | Utilisé pour `KeyCode` seulement. Ne pas suivre la bêta : stabiliser d'abord. |
| [`glam`](https://github.com/bitshifter/glam-rs) | 0.34.3 / 2026-11 | **DÉJÀ** | Dépendance workspace. |
| [`rend3`](https://github.com/BVE-Reborn/rend3) | **0.3.0 / 2022-02** | **REJET** | Abandonné (aucune release depuis 2022, ~0 téléchargement). La liste le présente comme vivant : c'est faux. |
| [`pixels`](https://github.com/parasyte/pixels) | 0.17.2 / 2026-07 | **REJET** | Présentation GPU d'un buffer de pixels ; notre `Canvas` est déjà un `u32` en mémoire, sans dépendance GPU. |
| [`lyon`](https://github.com/nical/lyon) | 1.0.19 / 2026-03 | **REJET** | Tessellation pour GPU ; nous rastérisons en logiciel. |
| [`salva2d`](https://github.com/dimforge/salva) | 0.11.0 / 2026-11 | **REJET** | Fluides : hors périmètre jeu 2D low-end. |
| [`avian2d`](https://github.com/avianphysics/avian) | 0.8.0 / 2026-10 | **REJET** | Physique ECS (bevy) : incompatible avec notre modèle d'entités sérialisé. |
| [`hecs`](https://github.com/Ralith/hecs) / [`shipyard`](https://github.com/leudz/shipyard) / [`legion`](https://github.com/amethyst/legion) | 0.11.2 / 0.11.5 / **0.4.0 (2021)** | **REJET** | Le format de projet est un `Vec<Entity>` ordonné avec composants `Option` : c'est ce que sérialisent les sauvegardes **et** les ops IA. Passer en ECS casserait les deux. Legion est de surcroît abandonné (2021). |
| [`flecs`](https://github.com/jazzay/flecs-rs) | 0.1.5 / **2023-10** | **REJET** | FFI C/C++ (contredit « pur Rust »), 0.1.5, ~0 téléchargement. |
| [`nalgebra`](https://github.com/dimforge/nalgebra) | 0.35.0 / 2026-05 | **REJET** | Doublon de `glam` ; nous n'utilisons que `Vec3`/`Quat`/`Mat4`. |
| [`egui`](https://github.com/emilk/egui) | 0.37.0 / 2026-10 | **REJET (Studio)** | Le Studio est React/Tauri ; réécrire l'éditeur en egui serait un changement de produit, pas une dépendance. |
| [`cosmic-text`](https://github.com/pop-os/cosmic-text) | 0.19.0 / 2026-04 | **REJET** | Le moteur rend son texte par `fontdue` + police bitmap ; inutile ici. |
| [`pathfinding`](https://github.com/evenfurther/pathfinding) | 4.16.0 / 2026-09 | **REJET** | **Déjà implémenté** : A* sur grille optimisé dans `navigation.rs`. |
| [`renet`](https://github.com/lucaspoffo/renet) | 2.0.1 / 2026-10 | **REJET (v1)** | Multijoueur/rollback = autre produit. La promesse v1 est un jeu local déterministe exportable. |

**Candidats absents de la liste, plus utiles que la moitié de celle-ci :**

| Crate | Pourquoi |
|---|---|
| `notify` | Rechargement à chaud des assets et des scripts KVL quand un fichier change sur disque (confort d'authoring). |
| `symphonia` | Décodage OGG/MP3/FLAC pour Kira (déjà utilisé en amont par Kira via feature). |
| `zip` | Export desktop et packaging d'assets. |
| `meshopt` / `ktx2` | Compression des maillages et textures à l'export (budget low-end). |
| `png` (déjà là) | À conserver : décodage minimal, build rapide, suffisant pour le pixel art. |

---

## 9. Plan proposé (6 phases, chacune finie et démontrable)

> **Ce plan est décliné en deux documents opposables** :
> [`P1_JEU_2D_COMPLET.md`](P1_JEU_2D_COMPLET.md) (périmètre gelé, critères AC1-AC6) et
> [`PLAN_INTEGRATION_OUTILS.md`](PLAN_INTEGRATION_OUTILS.md) (objectifs, fonctionnalités,
> impacts, résultats attendus par phase). Le suivi des versions et les rapports obligatoires
> sont dans [`VERSIONS.md`](VERSIONS.md).

| Phase | Objectif | Livrable vérifiable | Durée indicative |
|---|---|---|---|
| **P0 — Cadrage** | figer les niveaux et les invariants | ce document validé | 0,5 j |
| **P1 — Boucle 2D complète** | créer → éditer → logique → exporter | un jeu L3 exporté, 4 captures d'audit, test de reproductibilité | 1 sem. |
| **P2 — Pipeline d'assets** | manifeste, import PNG/OGG, budget, pré-vol | `assets/manifest.json` + gate d'export | 1 sem. |
| **P3 — Packs image/audio** | MCP + endpoint compatible OpenAI | sprite généré → manifeste → scène, sans cloud | 1 sem. |
| **P4 — Blender + 3D déclarative** | import glTF validé, placement, lumières | scène 3D jouable en HTML5 | 1-2 sem. |
| **P5 — Petit modèle mesuré** | harnais 40×10, 7B via Ollama | rapport de précision/latence publié | 1 sem. |
| **P6 — Exports** | desktop + WASM, pré-vol | trois cibles qui passent le même scénario | 1-2 sem. |

**Ordre non négociable** : P1 avant P2 (sinon on génère des assets que rien ne
consomme), P2 avant P4 (sinon Blender produit des fichiers hors manifeste), P5
après P1 (on ne mesure pas un produit incomplet).

---

## 10. Anti-objectifs (écrits pour être opposables)

1. Pas de modeleur 3D, pas de rigging, pas de shader custom dans le Studio.
2. Pas d'IA ni de MCP dans le runtime exporté.
3. Pas de boucle agente en v1 : une intention = une proposition = une transaction.
4. Pas de physique 3D avant qu'un jeu L5 en ait besoin.
5. Pas de génération de vidéo « longue » : des frames, des boucles, des planches.
6. Pas de dépendance cloud obligatoire ; MCP local d'abord, cloud optionnel.
7. Pas de nouvel écran plein écran : l'édition reste inline/ancrée.

---

## 11. Décisions à trancher

1. **P1 (jeu 2D complet) avant tout le reste ?** C'est ma recommandation : c'est ce
   qui prouve la promesse « créer et publier », et ça révèle les manques réels.
2. **Le pipeline d'assets est-il le prochain chantier de code ?** Il conditionne
   image, audio, 3D et l'IA utile.
3. **Un seul backend compatible OpenAI (Ollama / LM Studio / LocalAI) ou
   intégrations séparées ?** Je recommande le contrat unique.
4. **Blender via MCP : dès P4 ou après un L3 solide ?** Je recommande après.
5. **Quel niveau de jeu est la cible de la v1 ?** L3 (jeu 2D complet) me paraît le
   bon engagement ; L5 en démonstration, L6 assumé hors scope.
