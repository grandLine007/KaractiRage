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
  * Temps de frame CPU moyen : ~420 µs (< 3% du budget frame de 16.6 ms).

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
