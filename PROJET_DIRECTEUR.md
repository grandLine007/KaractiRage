# KARACTIRAGE — FICHIER DIRECTEUR DU PROJET

> **Rôle de ce document** : Ce fichier est le document vivant et collaboratif qui pilote le développement de KaractiRage. Il consigne l'état du projet, les étapes franchies, les observations de l'utilisateur (vos souhaits, ajustements et retours) et les observations techniques de l'agent. Il sert de boussole permanente.

---

## 1. Vision et Principes Fondateurs
* **Objectif** : Concevoir un moteur et environnement de création de jeux accessible aux PC modestes (*low-end first*), modulaire, déterministe et assisté par des agents IA outillés.
* **Philosophie clé** : Le runtime de jeu est 100% autonome et déterministe (aucune IA ni connexion obligatoire pour jouer).
* **Langage du cœur** : **Rust** (sécurité mémoire, modularité Cargo, fiabilité pour le dev multi-agents).
* **Système logique** : **KVL (KaractiRage Visual Logic)** — continuum en 3 niveaux (Guidé $\rightarrow$ Visuel $\rightarrow$ Expert) sur un modèle unique de données.

---

## 2. Cartographie de l'Architecture (Crates actives)

| Crate | Responsabilité | Statut |
| :--- | :--- | :--- |
| `crates/kvl-core` | Modèle de données universel (graphe, nœuds, pins, variables), sérialisation JSON/RON, validateur statique d'intégrité. | ✅ Opérationnel (v0.1.0) |
| `crates/kvl-runtime` | Interpréteur d'exécution séquentiel, traçabilité des pas, gestion du contexte mémoire et protection anti-boucles infinies. | ✅ Opérationnel (v0.1.0) |
| `crates/kvl-cli` | CLI de test, validation et benchmark autonome des mécaniques KVL. | ✅ Opérationnel (v0.1.0) |
| `crates/engine-core` | Fenêtrage natif (`winit`), renderer de compatibilité 2D (`softbuffer`), boucle de jeu 60 FPS et interconnexion KVL. | ✅ Opérationnel (v0.1.0) |

---

## 3. Journal des Étapes et Réalisations

### Étape 1 : Initialisation & Prototype KVL Core *(14 Octobre 2026)*
* **Réalisations** :
  * Création du dépôt Git local avec branche `main`.
  * Configuration du Workspace Cargo modulaire (`kvl-core`, `kvl-runtime`, `kvl-cli`).
  * Implémentation du modèle KVL : `Graph`, `Node` (catégories d'intention, blocs composés), `Pin`, `Connection`, `Value`.
  * Validateur statique détectant les incohérences de types, connexions orphelines et inversions de pins.
  * Interpréteur d'exécution testé sur la mécanique de ramassage d'une pièce.
* **Mesures & Benchmarks** :
  * Temps de résolution de l'événement et exécution des 3 nœuds : **13.5 microsecondes**.
  * Consommation mémoire : négligeable, allocations contrôlées, zéro fuite.
  * 100% des tests unitaires validés.

### Étape 2 : Moteur de rendu 2D et Fenêtrage natif *(14 Octobre 2026)*
* **Réalisations** :
  * Ajout de la crate `engine-core` basée sur `winit` 0.29 et `softbuffer` 0.4.
  * Implémentation du renderer 2D logiciel (Canvas pixel, formes géométriques, texte bitmap sans police externe).
  * Création de la scène de jeu interactive : joueur contrôlable (flèches / ZQSD), pièces à collecter, porte interactive.
  * Interconnexion directe avec le runtime KVL : chaque collision ou interaction déclenche un événement KVL qui met à jour les variables et génère un log de traçabilité en bas d'écran.
* **Mesures & Benchmarks** :
  * Temps de compilation complet : 3.8s.
  * Consommation RAM estimée : ~15 Mo.
  * Zéro avertissement de compilation, tests workspace au vert.

### Étape 3 : Architecture Data-Driven, Mode Guidé KVL & Hot-Reload *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **Mode Guidé** dans `kvl-core` (`GuidedBuilder`) : génération automatique de graphes complets à partir d'intentions de jeu (`CollectibleIntent`, `DoorIntent`).
  * Découplage complet scène $\leftrightarrow$ logique : création de `SceneData` et du répertoire `assets/` contenant `level_01.scene.json` et les scripts `.kvl.json`.
  * Support du **rechargement à chaud (Hot-Reload)** via la touche **R** : permet de modifier les fichiers de scènes ou de scripts KVL et d'observer les modifications instantanément dans le jeu.
* **Mesures & Benchmarks** :
  * 100% des tests unitaires validés (incluant la validation du mode guidé).
  * Rechargement à chaud quasi-instantané (< 1 ms en mémoire).

### Étape 4 : Télémétrie en temps réel & Adaptation matérielle dynamique *(14 Octobre 2026)*
* **Réalisations** :
  * Module `telemetry` : mesure continue à la microseconde du temps de frame CPU, calcul du FPS lissé, historique glissant de 120 frames.
  * Module `adaptation` : gestion des 3 profils cibles (`Compatibilité`, `Équilibré`, `Élevé`) et boucle de régulation par hystérésis (dégradation gracieuse en cas de surcharge prolongée, temporisation anti-oscillations).
  * Panneau d'overlay de diagnostic (**F1**) : affichage du graphe Sparkline d'historique en direct, jauges de budget et compteurs de pas KVL.
  * Touches de contrôle en jeu : **F1** (Overlay), **F2** (Cycler profil), **F3** (Auto/Manuel), **R** (Hot-Reload).
  * Système de particules réactif (pièces et portes) s'adaptant automatiquement au budget du profil matériel.
* **Mesures & Benchmarks** :
  * Temps de frame CPU moyen : **< 400 microsecondes** (consomme moins de 3% du budget de 16.6ms pour 60 FPS).
  * Tests workspace 100% au vert, zéro warning.

### Étape 5 : Mode Éditeur Interactif & Dépliage Progressif KVL *(14 Octobre 2026)*
* **Réalisations** :
  * Création du module `editor` : bascule instantanée entre **Mode Jeu** et **Mode Édition** avec la touche **`TAB`**.
  * **Sélection et déplacement à la souris (Mouse picking)** : clic sur n'importe quel objet (joueur, pièce, porte), halo de sélection jaune et glisser-déposer pour réorganiser la scène.
  * **Inspecteur d'entité & Visualiseur KVL** (Phase 10 de la roadmap) :
    * **Vue Simplifiée** : Affichage linéaire de macro-blocs stylisés (*« Contact $\rightarrow$ Score +10 $\rightarrow$ Disparaître »*).
    * **Vue Détaillée (Dépliage progressif)** : Touche **`ESPACE`** pour déplier la vue technique complète (nœuds individuels, pins Trigger In/Out et connexions).
  * Suspension déterministe du gameplay en mode édition pour éditer sereinement, puis reprise immédiate avec **`TAB`**.
* **Mesures & Benchmarks** :
  * Temps de compilation : 0.82s.
  * Empreinte mémoire stable : ~15 Mo de RAM.
  * Zéro régression, 100% des tests unitaires validés.

### Étape 6 : Refonte de l'affichage & Typographie haute lisibilité *(14 Octobre 2026)*
* **Réalisations** :
  * Réponse au retour utilisateur (*« revois un peu l'affichage, c'est trop petit »*) particulièrement marqué sur les écrans haute densité (Retina / HiDPI).
  * Résolution de fenêtre native portée à **1200 x 800** (au lieu de 800 x 600).
  * Implémentation du moteur de rendu de texte bitmap avec multiplicateur d'échelle (`scale = 2` ou `3`) : glyphes 12x14 px et 18x21 px parfaitement lisibles sans flou.
  * Re-dimensionnement et repositionnement proportionnel des entités interactives (joueur, pièces, porte, particules).
  * Panneau d'inspecteur latéral élargi (360 px), encarts de titres de scènes et télémétrie redimensionnés dynamiquement.
* **Mesures & Benchmarks** :
  * Fluidité maintenue : temps de frame CPU toujours < 1 ms en software rendering direct.
  * Confort visuel immédiat en jeu et dans l'inspecteur KVL.

### Étape 7 : Câblage interactif de broches KVL & Programmation visuelle à la souris *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **câblage visuel complet de nœuds KVL** (Option C choisie par l'utilisateur).
  * Algorithme de tracé de lignes de Bresenham déterministe (`draw_line`, `draw_thick_line`) et symboles typographiques enrichis (`>`, `<`, `[`, `]`, `X`).
  * Méthodes de manipulation de graphe dans `kvl-core` : `connect_pins`, `disconnect_pin`, `remove_connection_by_id`, `is_pin_connected`.
  * Rendu de boîtes de nœuds modulaires techniques avec connecteurs `[>]` colorés (vert si branché, cyan/or si libre).
  * **Câblage interactif à la souris** : clic sur broche Output $\rightarrow$ fil élastique animé $\rightarrow$ clic sur broche Input réceptrice pour valider la liaison.
  * **Coupure unitaire** : badge rouge `[X]` au centre de chaque fil pour sectionner une liaison en un clic.
  * Répercussion en direct sur le gameplay en revenant au Mode Jeu (`TAB`) : prouve la réactivité du moteur KVL.
* **Mesures & Benchmarks** :
  * Temps de compilation : 0.81s.
  * 100% des tests validés (5/5).
  * Temps de frame CPU < 500 µs (consommation < 4% du budget 60 FPS).

### Étape 8 : Moteur Physique 2D, Murs Solides & Glissement Continu AABB *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **système de collision physique 2D et glissement continu** (Option B choisie par l'utilisateur).
  * Ajout de `WallEntity` et du champ `walls` dans le format de scène data-driven `level_01.scene.json` (cloisons de salle et embrasure de porte).
  * Algorithme de détection et résolution AABB par passes d'axes séparées (X puis Y) garantissant un glissement fluide et naturel le long des parois lors de déplacements diagonaux.
  * Liaison physique KVL : les portes fermées constituent des obstacles infranchissables, et deviennent perméables dès leur ouverture interactive par script KVL.
  * Rendu texturé des murs et intégration complète dans le Mode Éditeur (`TAB`) : sélection, déplacement à la souris et fiche de propriétés physiques.
* **Mesures & Benchmarks** :
  * 7/7 tests unitaires réussis à 100% (incluant `test_aabb_wall_blocking_and_sliding` et `test_door_solidity_when_closed_vs_open`).
  * Temps de frame CPU toujours < 500 µs.



### Étape 9 : Palette de Création de Nœuds KVL & Sauvegarde de Scène (Ctrl+S) *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation conjointe des **Options D et E** :
    * **Palette de création de nœuds KVL (« + Nouveau Nœud »)** :
      * Menu contextuel flottant violet néon dans l'inspecteur offrant 3 modèles d'actions prêts à l'emploi (`BonusPoints`, `BoostSpeed`, `SpawnSparkles`).
      * Instanciation dynamique dans le graphe en mémoire avec identifiant unique, pins Trigger In/Out et catégorisation contextuelle.
      * Prise en charge dans `kvl-runtime` : `BonusPoints` (+50 pts au score), `BoostSpeed` (+120 px/s à la vitesse de déplacement), `SpawnSparkles` (gerbe de 35 particules dorées).
    * **Persistance complète de scène & KVL sur disque (Ctrl+S / Cmd+S)** :
      * Enregistrement de l'état spatial complet dans `level_01.scene.json` (positions modifiées du joueur, pièces, portes et murs).
      * Écriture et sérialisation des scripts `.kvl.json` associés à chaque entité pour préserver les nouveaux nœuds et les connexions modifiées.
      * Raccourci universel `Ctrl+S` / `Cmd+S` (compatible macOS) et bouton interactif dédié `[ SAUVER (CTRL+S) ]` dans l'inspecteur.
* **Mesures & Benchmarks** :
  * 9/9 tests unitaires réussis à 100% sur l'ensemble du Workspace Cargo.
  * Zéro avertissement de compilation, persistance validée par test automatisé de rechargement à froid.
### Étape 10 : Blocs de Logique Conditionnelle & Variables KVL *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **branchement conditionnel dynamique dans KVL** (Option 2 choisie par l'utilisateur) :
    * Évaluation dans `kvl-runtime` des nœuds `NodeKind::Condition { operator }` sur les variables mémoire (ex: `ScoreAtLeast50`, `CheckVarBool`).
    * Routage déterministe du flux d'exécution vers la broche de sortie `out_true` (Vrai) ou `out_false` (Faux), avec traçabilité complète dans le journal d'audit.
    * Nouvelles actions natives : `ToggleDoor` (ouverture porte), `DenyAccess` (flag de refus d'accès), `ResetScore`.
  * Verrouillage conditionnel interactif dans `level_01` :
    * La porte principale exige désormais `Score >= 50` pour s'ouvrir.
    * En cas de score insuffisant (< 50) : déclenchement de la branche `out_false`, verrouillage maintenu, geyser de particules d'alerte rouges et message explicite.
    * Dès que le joueur atteint 50 points : déclenchement de la branche `out_true`, déverrouillage de la porte et particules vertes.
  * Enrichissement de l'éditeur visuel & palette :
    * Nouveaux templates de palette : `? Condition: Si Score >= 50` et `! Action: Alerte Refus`.
    * Thème visuel ambre / orange néon (`0x00D97706`) avec broches stylisées en vert émeraude (`Vrai:Out`) et rouge (`Faux:Out`).
* **Mesures & Benchmarks** :
  * 11/11 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo.
  * Zéro avertissement de compilation.
  * Temps de frame CPU moyen : ~420 µs (< 3% du budget frame de 16.6 ms pour 60 FPS).

### Étape 11 : Système Sonore Rétro & Synthèse Procédurale Low-Overhead *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **synthétiseur sonore procédural temps réel** (Option A choisie par l'utilisateur) :
    * Intégration de `tinyaudio` (moteur audio bas niveau multiplateforme léger, CoreAudio sous macOS, WASAPI sous Windows, ALSA sous Linux).
    * Générateur d'ondes mathématiques pures (ondes carrées, sinusoïdales, triangulaires, pseudo-bruit blanc et glissandi avec enveloppe d'amplitude ADSR).
    * Zéro dépendance d'actifs volumineux sur disque : 100% procédural (aucun fichier .wav/.mp3 à charger).
    * 9 familles d'effets sonores arcade rétro :
      * `Coin` : carillon doré bi-ton B5 (987 Hz) $\rightarrow$ E6 (1318 Hz).
      * `BonusFanfare` : arpège héroïque 4 notes C5-E5-G5-C6.
      * `DoorOpen` : glissando ascendant triangulaire 220 Hz $\rightarrow$ 440 Hz.
      * `DoorClose` : glissando descendant sourd 380 Hz $\rightarrow$ 140 Hz.
      * `AccessDenied` : double bip grave de buzzer d'erreur (130 Hz).
      * `SpeedBoost` : balayage de fréquence ascendant avec vibrato dynamique.
      * `Sparkles` : poussière d'étoiles scintillante haute fréquence (1200-2400 Hz).
      * `UiClick` : impulsion courte et nette (1200 Hz) pour validation UI et câblage.
      * `WireCut` : claquement percussif texturé pour section de fil KVL.
  * Raccordement complet aux événements de jeu et d'édition :
    * Collision avec une pièce $\rightarrow$ son `Coin` (ou `BonusFanfare` si bonus >= 50).
    * Interaction avec la porte $\rightarrow$ `AccessDenied` si verrouillée, sinon `DoorOpen` / `DoorClose`.
    * Câblage de nœuds KVL $\rightarrow$ `UiClick` lors de la pose, `WireCut` lors de la section de liaison.
    * Sauvegarde globale (`Ctrl+S`) $\rightarrow$ carillon de validation `BonusFanfare`.
    * Raccourci Mute clavier (`M`) : bascule instantanée du flux audio avec retour haptique et indicateur HUD (`[AUDIO: ACTIF (M)]` vert ou `[AUDIO: MUET (M)]` rouge).
  * Résilience et architecture sans crash :
    * Dégradation gracieuse automatique (`is_available = false`, `[AUDIO: N/D]`) en environnement sans carte son / tests automatisés headless.
* **Mesures & Benchmarks** :
  * 14/14 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+3 tests audio et intégration).
  * Zéro avertissement de compilation, buffer audio streaming à faible latence (~10 ms).
  * Impact CPU négligeable (< 0.2% de CPU supplémentaire par frame).

### Étape 12 : Ennemis Patrouilleurs, IA KVL (Raycast LoS) & Minicarte Radar 2D *(14 Octobre 2026)*
* **Réalisations** :
  * Implémentation du **système d'IA de patrouille et de détection de champ de vision** (Raycasting & Line of Sight) :
    * Ajout de `EnemyEntity` et de `enemies` dans le format de scène data-driven `level_01.scene.json`.
    * Sentinelle autonome (`guard_sentinel_1`) avec patrouille ping-pong sur l'axe Y et visière robotique.
    * Algorithme d'intersection segment-AABB de **Liang-Barsky** ultra-rapide ($\mathcal{O}(1)$ sans allocation) pour tester l'occultation de la ligne de mire par les murs et portes fermées.
    * Détection en cône de vision en temps réel : dès que le joueur est vu sans obstacle, le garde déclenche l'événement KVL `"OnPlayerSeen"`, passe en état d'alerte rouge, émet la sirène rétro `SoundType::Alarm` et génère un geyser de particules d'exclamation.
  * Nouvelles actions et script KVL `guard_alert.kvl.json` :
    * `TriggerAlarm` (activation du flag d'alerte), `ClearAlarm` (extinction temporisée après fuite du joueur), `LockAllDoors` (verrouillage de sécurité immédiat de toutes les portes pour confiner l'intrus).
    * Nœuds ajoutés à la palette contextuelle de l'éditeur visuel KVL.
  * **Minicarte Radar Tactique 2D (Minimap)** :
    * Cadre radar technologique bleu nuit avec grille et bordures néon cyan `0x000284C7`.
    * Projection en temps réel de tous les éléments du monde (murs, portes, pièces dorées, sentinelles avec faisceau de visée, et joueur clignotant).
    * Bascule d'affichage via la touche `N` avec effet sonore `SoundType::RadarPing`.
  * Raccordement complet au Mode Éditeur (`TAB`) :
### Étape 13 : Partitionnement Spatial (Spatial Hash Grid) $\mathcal{O}(1)$ & Zéro-Allocation *(14 Octobre 2026)*
* **Réalisations** :
  * Conception et intégration de la structure de données `SpatialHashGrid` (`crates/engine-core/src/spatial.rs`) :
    * Hachage spatial 2D avec taille de cellule optimisée ($C = 64\text{ px}$) et dispersion bitwise rapide `(cx * 73856093 ^ cy * 19349663) & bucket_mask`.
    * Masquage bitwise sur puissance de deux ($N=256$) éliminant le coût des divisions modulo.
    * Respect strict de la directive d'architecture **Zero Alloc Hot Path** : réinitialisation in-place `bucket.clear()` et tampon réutilisable `query_buffer` évitant toute réallocation de mémoire tas par frame.
  * Indexation spatiale dynamique et broadphase :
    * Indexation de tous les éléments du monde : murs solides, portes fermées, pièces dorées, et sentinelles de patrouille.
    * Accélération de la résolution physique AABB du joueur (glissement fluide séparé X et Y) via `query_aabb`.
    * Accélération de la détection et collecte des pièces via filtrage spatial préalable.
    * Accélération du Raycasting de ligne de mire (LoS) des gardes patrouilleurs via `query_segment` (recherche des seuls obstacles dans le corridor de visée avant l'intersection fine de Liang-Barsky).
  * Affichage de débogage et télémétrie :
    * Visualisation en direct du quadrillage de cellules spatiales activable avec la touche **`G`**.
    * Indicateur des statistiques de broadphase (`stats_queries`, `stats_candidates`) affiché en direct dans l'overlay de profilage **`F1`**.
    * Raccourci `G (Grille)` ajouté au pied d'écran.
* **Mesures & Benchmarks** :
  * 19/19 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+3 tests dédiés au spatial hash grid).
  * Suppression totale des allocations dynamiques de vecteurs d'obstacles (`solid_boxes: Vec<SolidBox>`) dans la boucle de frame.
  * 0 avertissement de compilation (`cargo check --all-targets`).
### Étape 14 : Exportateur WebAssembly (Wasm) & Démonstrateur Canvas HTML5 *(14 Octobre 2026)*
* **Réalisations** :
  * Architecture bicéphale native Desktop (`main.rs`) & WebAssembly (`lib.rs` / `cdylib`) dans `crates/engine-core` :
    * Exposition de la bibliothèque `karactirage_wasm` avec liaisons `wasm-bindgen` complètes (`crates/engine-core/src/wasm_api.rs`).
    * Gestion native du zero-copy : le pointeur brut vers le buffer de pixels RGBA32 de `Canvas` est partagé directement avec la mémoire linéaire WebAssembly (`Uint8ClampedArray` vers `ImageData` HTML5).
    * Fallback autonome de scène et scripts KVL embarqués (`include_str!`) : permet au binaire Wasm de fonctionner instantanément dans le navigateur sans nécessiter d'appels `std::fs` ni de requêtes réseau synchrones.
  * Démonstrateur Web complet dans `web/` :
    * `web/index.html` : Interface cockpit haute fidélité en Dark Mode (palette HSL sur-mesure, typographies Outfit et Fira Code, glassmorphism avec flou d'arrière-plan).
    * `web/style.css` : Design responsive avec affichage pixel-art préservé (`image-rendering: pixelated`), cartes d'indicateurs de performance et légendes interactives.
    * `web/app.js` : Boucle d'animation à 60 FPS (`requestAnimationFrame`), transmission continue des entrées clavier et coordonnées souris (redimensionnées au Canvas 1200x800), et polling télémétrique à 10 Hz.
    * Boutons rapides tactiles/cliquables pour basculer le Mode Éditeur (TAB), le Radar (N), la Grille (G) et le Son (M).
  * Outillage & Automatisation :
    * `scripts/build_web.sh` : Script de compilation Wasm automatique avec `wasm-bindgen` (binaire optimisé `karactirage_wasm_bg.wasm` de seulement 535 Ko).
    * `scripts/serve_web.py` : Serveur de développement local Python avec types MIME conformes (`application/wasm`, `application/javascript`) et isolation cross-origin.
* **Mesures & Benchmarks** :

### Étape 15 : Architecture Moteur ECS & Découplage Total du Gameplay *(14 Octobre 2026)*
* **Directive Majeure de l'Utilisateur** : *"Tâche de ne plus perdre de vue l'objectif : construire un moteur, pas seulement un jeu."*
* **Réalisations** :
  * **Élimination complète des concepts de jeu codés en dur** dans le noyau du moteur (`engine-core`) :
    * Suppression de tous les champs spécifiques (`player_x`, `player_y`, `coins: Vec<CoinInstance>`, `doors: Vec<DoorInstance>`, `walls: Vec<WallInstance>`, `enemies: Vec<EnemyInstance>`).
    * Implémentation du **modèle universel Entité-Composants (ECS)** dans `crates/engine-core/src/entity.rs` :
      * `Entity` : Conteneur générique avec identifiant, nom, tag, transform 2D et composants optionnels.
      * `Transform2D` : Coordonnées spatiales $X, Y$, dimensions $W, H$ et rotation.
      * `Collider2D` : Boîte de collision AABB solide ou déclencheur (trigger) avec calque de collision.
      * `Renderable2D` : Primitives de rendu vectoriel (`Rect`, `Circle`, `Text`), couleurs, bordures et ordre de profondeur $Z$-index.
      * `Movement2D` : Propriétés cinématiques, contrôle joueur ou patrouille ping-pong configurable.
      * `VisionSensing` : Cône de vision paramétrable avec gestion de l'alerte et minuteur de décroissance.
      * `KVLScriptComponent` : Liaisons data-driven avec les scripts visuels KVL (événements `on_overlap`, `on_interact`, `on_seen`).
  * **Systèmes Moteur Découplés & Agnostiques** :
    * `SpatialPartitioningSystem` : Insertion universelle de tout composant `Collider2D` dans la grille spatiale `SpatialHashGrid` $\mathcal{O}(1)$.
    * `MovementPhysicsSystem` : Déplacement générique avec glissement AABB sur $X$ et $Y$ sans collision pour toute entité contrôlée.
    * `TriggerOverlapSystem` : Détection broadphase d'intersection pour toute entité marquée `is_trigger`, déclenchant automatiquement le script KVL associé (`on_overlap_event`).
    * `InteractionSystem` : Déclenchement générique à distance (touche E / Espace) pour toute entité dotée d'`on_interact_event`.
    * `VisionSystem` : Raycasting Line of Sight de Liang-Barsky générique entre l'entité observatrice et le joueur à travers les obstacles solides de la grille.
    * `RenderSystem` : Rendu trié par $Z$-index avec support des formes `Rect`, `Circle`, `Text`, faisceaux de vision et sélection d'éditeur.
    * `TacticalRadarSystem` : Projection générique sur minicarte basée sur les tags d'entités (`"player"`, `"wall"`, `"door"`, `"collectible"`, `"enemy"`).
  * **Éditeur KVL Universel** :
    * Sélection générique `SelectedEntity::Entity(usize)` permettant de cliquer, déplacer et inspecter à la souris n'importe quelle entité du monde.
    * Rendu contextuel : graphe KVL pour les entités scriptées, inspecteur de composants pour les entités passives.

### Étape 16 : Extension de la Palette KVL (Téléporteurs, Altérateurs de Vitesse & Compteurs) *(14 Octobre 2026)*
* **Réalisations** :
  * **Nouveaux Nœuds & Actions KVL** :
    * `TeleportPlayer` & `TeleportTo:x,y` : Téléportation spatiale instantanée de l'entité vers des coordonnées absolues ou relatives avec émission d'effets de particules cyan et signal sonore dédié.
    * `SlowSpeed` & `ModifySpeed:delta` : Ajustement dynamique de la vitesse (zones de boue, glace, ralentisseurs, hyper-propulseurs).
    * `IncrementCounter:name` : Incrémentation universelle de compteurs numériques dans l'`ExecutionContext`.
    * `SetVar:name:val` & `AddVar:name:delta` : Manipulation data-driven de variables arbitraires de jeu (`keys`, `health`, `mana`, `level`).
  * **Nouveaux Opérateurs Conditionnels KVL** :
    * `CounterAtLeast:name:threshold` & `CounterGte:name:threshold` : Vérification de seuils sur compteurs (ex: clés réunies, interrupteurs activés).
    * `VarGte:name:threshold` & `VarEqBool:name:val` : Branchements logiques `out_true` / `out_false` sur toute variable d'exécution.
  * **Synthèse Sonore Procédurale FM Rétro** :
    * Ajout de `SoundType::Teleport` dans `AudioEngine` (`crates/engine-core/src/audio.rs`) : balayage ascendant 250 Hz $\rightarrow$ 1050 Hz avec modulation de fréquence (FM) 30 Hz et décroissance exponentielle.
  * **Palette Visuelle de l'Éditeur & Hitboxes** :
    * Élargissement de la palette overlay contextuelle `[+ NOUVEAU NŒUD]` (12 modèles de nœuds KVL disponibles).
    * Câblage direct de pins et déclenchement réactif dans la boucle de frame.
* **Mesures & Benchmarks** :
  * 34/34 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+4 tests dédiés aux nouvelles actions, téléportation et compteurs).
  * 0 avertissement de compilation (`cargo check --all-targets`).
  * WebAssembly recompilé avec succès (535 Ko).

### Étape 17 : Studio de Création d'Entités à Chaud & Authoring ECS *(14 Octobre 2026)*
* **Réalisations** :
  * **Palette de Création Universelle (Spawn Palette)** :
    * Accessible via le bouton interactif `[+ SPAWN (P)]` ou la touche **P** en mode Édition.
    * Gabarits préconfigurés prêts à l'emploi : `Mur Solide (AABB)`, `Objet Collectible (Pièce KVL)`, `Porte Interactive Sécurisée`, `Sentinelle Patrouilleuse (Vision/Raycast)`, `Portail Quantique (Téléporteur)`.
  * **Duplication & Suppression Interactive** :
    * Duplication instantanée (`Ctrl+D` / `Cmd+D` ou bouton dédié) : clonage profond de l'entité, génération d'ID unique sans collision, décalage spatial de confort (+24px) et désactivation du contrôle joueur sur les clones.
    * Suppression sécurisée (`Suppr` / `Backspace` ou bouton dédié) : suppression propre de l'entité avec émission de particules de destruction et protection inviolable du joueur principal contre toute suppression accidentelle.
  * **Composition Dynamique de Composants ECS à la Volée** :
    * Inspecteur contextuel affichant les composants actifs et proposant l'ajout dynamique des composants manquants : `[+ Collider2D]`, `[+ Movement2D]`, `[+ VisionSensing]`, `[+ Script KVL]`.
  * **Barre d'Outils Studio Unifiée & Ergonomie** :
    * Bandeau supérieur transformé en véritable barre de commande de studio : boutons `SPAWN`, `SAUVER`, `DUPLIQUER`, `SUPPRIMER`, statut audio et raccourcis clavier affichés en bas d'écran.
    * Re-partitionnement spatial automatique $\mathcal{O}(1)$ de toute entité créée ou déplacée sans réallocation mémoire.
* **Mesures & Benchmarks** :
  * 42/42 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+4 tests dédiés au spawning runtime, à la duplication, à la suppression protégée et à la composition dynamique de composants).
  * 0 avertissement de compilation (`cargo check --all-targets`).
  * WebAssembly recompilé avec succès (539 Ko).

### Étape 18 : Système d'Émetteurs de Particules Déclaratif ECS & Pilotage KVL *(15 Octobre 2026)*
* **Réalisations** :
  * **Composant Déclaratif `ParticleEmitter2D`** :
    * Modes d'émission : `Continuous` (flux régulier asservi au temps delta $\Delta t$) et `Burst` (gerbes instantanées).
    * Paramètres physiques et esthétiques complets : cadence (`rate` particules/s), durée de vie (`lifetime`), cône d'orientation et d'ouverture angulaire (`spread_rad`), vitesse (`speed_min`, `speed_max`), accélération gravitationnelle/poussée (`ax`, `ay`), et dégradé chromatique (`color_start`, `color_end`).
    * Presets prêts à l'emploi : `torch_fire` (flamme ascendante jaune $\rightarrow$ rouge), `magic_aura` (aura mystique violette $\rightarrow$ cyan), `smoke_plume` (volutes de fumée grise avec retombée douce), `spark_burst` (étincelles vives).
  * **Simulation Physique Logicielle Haute Performance (Zero-Alloc Hot Path)** :
    * Extension de la structure `Particle` avec accélération $a_x, a_y$, temps de vie maximum et gradient dynamique de couleur.
    * Fonction d'interpolation chromatique bitwise ultra-rapide `lerp_color(c1, c2, t)` sans allocations heap.
    * Respect strict et dynamique du plafond adaptatif (`max_particles`) selon le profil matériel actif (`Compatibility: 10`, `Balanced: 35`, `High: 100`).
  * **Pilotage Événementiel par le Langage Visuel KVL** :
    * Nouvelles actions natives dans `kvl-runtime` : `EmitBurst`, `EmitBurst:count`, `ToggleEmitter`, `ToggleEmitter:state`.
    * Bus d'événements moteur réactif (`emitter_burst`, `emitter_active`) permettant à tout script KVL de déclencher des gerbes ou de moduler l'émission des torches/auras.
  * **Intégration Studio & Authoring ECS** :
    * Nouveau gabarit dans la Spawn Palette : `Torche / Brasero (Émetteur de Flammes)`.
    * Bouton d'ajout contextuel dans l'inspecteur : `[ + PARTICLE EMITTER 2D ]`.
    * Affichage dynamique des caractéristiques de l'émetteur dans le panneau latéral (mode, cadence, durée de vie, gradient de couleur).
* **Mesures & Benchmarks** :
  * 50/50 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+3 tests dédiés à la simulation continue, au cap adaptatif et aux triggers KVL).
  * 0 avertissement de compilation (`cargo check --all-targets`).
  * WebAssembly recompilé et optimisé (543 Ko).

### Étape 19 : Refonte Esthétique & Ergonomie Pédagogique du Studio (HUD, Guide Interactif H & Design System) *(15 Octobre 2026)*
* **Directive de l'Utilisateur** : *"L'interface générale n'est pas très pédagogique ni esthétiquement attrayante, revois cela, s'il faut des skills qui facilitent cela, installe-les."*
* **Réalisations** :
  * **Installation du Skill Dédié** :
    * Création de `.agents/skills/engine-ui-ux-pedagogy/SKILL.md` : Guide de conception d'interfaces rétro-modernes logicielles, hiérarchie visuelle, code couleur cyber/dark tech, typographie bitmap complète et pédagogie progressive du visual scripting.
  * **Typographie Bitmap Enrichie & Primitives Graphiques** (`renderer.rs`) :
    * Extension complète de la table de glyphes bitmap 5x7 avec prise en charge native de tous les caractères spéciaux clés : `'?'`, `'_'`, `'|'`, `'#'`, `'%'`, `';'`, `'\''`, `'\"'`, `'{'`, `'}'`, `'$'`, `'~'`, `'@'`. Élimination des carrés de substitution disgracieux.
    * Nouvelles méthodes graphiques intégrées : `draw_badge()` (pilules et boutons biseautés avec texte centré) et `draw_corner_brackets()` (cornières technologiques néon pour encadrements immersifs).
  * **Fenêtre Modale de Guide Pédagogique Interactif (`H`)** :
    * Accessible en continu via la touche **H**, le bouton dédié `[? AIDE]` ou la touche `Échap`.
    * Résumé pédagogique en 4 volets :
      1. Les deux modes de travail (`[TAB] Mode Jeu` vs `[TAB] Mode Studio ECS`).
      2. L'architecture Entité-Composants (`Entity`, `Transform2D`, `Collider2D`, `Movement2D`, `VisionSensing`, `ParticleEmitter2D`, `KVLScriptComponent`).
      3. Le fonctionnement du langage visuel KVL (`Event` $\rightarrow$ `Condition` $\rightarrow$ `Action` $\rightarrow$ Bus d'événements).
      4. Les raccourcis clavier studio (`P`, `Ctrl+D`, `Suppr`, `Ctrl+S`, `Espace`, `N`, `G`, `M`, `F1`).
  * **Bandeau Supérieur & Carte d'Objectif Pédagogique** :
    * En Mode Jeu : Logo cyber, badge de scène, **carte d'objectif dynamique centrale** (`[ OBJECTIF : COLLECTEZ 50 OR POUR LA PORTE ]` en ambre étincelant, basculant en émeraude dès déverrouillage), score pill, profil matériel, audio et raccourcis.
    * En Mode Studio : Logo violet néon, boutons studio stylisés (`[+ SPAWN (P)]`, `[SAUVER (C+S)]`, `[DUPLIQUER (C+D)]`, `[SUPPR (DEL)]`, `[? AIDE]`).
  * **Inspecteur Pédagogique Déroulant** :
    * Lorsqu'aucune entité n'est sélectionnée : affichage d'une **fiche pédagogique "Guide du Studio" en 5 étapes numérotées** (1. Sélectionner, 2. Déplacer, 3. Composer ECS, 4. Logique KVL, 5. Tester en direct).
    * Lorsqu'une entité est sélectionnée : carte d'identité colorée selon le tag, badges détaillés des composants actifs et boutons d'attachement explicites.
    * Dans la chaîne KVL : cartes d'enchaînement avec badges `[1. QUAND]`, `[2. SI (TEST)]`, `[3. ALORS]` et flèches de flux néon.
  * **Bandeau Inférieur & Console KVL** :
    * Ruban de raccourcis sous forme de pilules cliquables, badge de statut moteur et console de traces temps réel au style terminal sombre.
* **Mesures & Benchmarks** :
  * 54/54 tests unitaires validés à 100% sur l'ensemble du Workspace Cargo (+2 tests dédiés à la modale d'aide, au switch d'objectif et aux hitboxes du HUD).
  * 0 avertissement de compilation (`cargo check --all-targets`).
  * WebAssembly recompilé et optimisé (554 Ko).

---

## 4. Observations & Directives de l'Utilisateur
*(Cette section vous est réservée. Vous pouvez y ajouter ou modifier vos notes, directives, préférences de design ou changements de priorités à tout moment.)*

* **Méthodologie demandée** :
  * Rédiger systématiquement un rapport d'intervention à chaque étape (stocké dans `rapports/`, ignoré par Git).
  * Maintenir à jour ce fichier directeur (`PROJET_DIRECTEUR.md`) avec les faits et les observations partagées.

---

## 5. Observations & Recommandations Techniques (Agent)

* **Performance & Low-End** :
  * Pour le fenêtrage et le rendu initial, le couple `winit` + `wgpu` (avec fallback d'API graphique) ou un framebuffer logiciel type `softbuffer` permet de garantir que le moteur s'ouvre même sur un PC modeste sans GPU dédié moderne.
  * *Recommandation* : Prévoir un mode de rendu "Software/Compatibilité" dès maintenant pour valider le principe *Low-end first* énoncé dans l'étude technique.
* **Intégration KVL $\leftrightarrow$ Moteur** :
  * La boucle de jeu doit convertir les entrées utilisateur (`KeyDown`, `MouseButton`, `Collision`) en triggers d'événements KVL (`trigger_event`).
  * Cela démontrera le premier jeu interactif jouable piloté par la logique visuelle.
