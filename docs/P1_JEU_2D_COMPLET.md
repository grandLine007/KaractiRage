# P1 — Gel de la boucle « jeu 2D complet »

> **Statut** : **figé le 10 novembre 2026**. Tout ajout hors de ce périmètre passe
> après v0.3.0. Les critères d'acceptation ci-dessous sont la définition du succès.
> **Version cible** : v0.3.0 · **Rapport attendu** : `docs/RAPPORTS/RAPPORT_v0.3.0.md`

---

## 1. Objectif de P1

Démontrer la promesse du projet sur son niveau le plus exigeant et le plus utile :
**un créateur produit un jeu 2D complet, jouable et exportable, sans écrire de code,
sur une machine modeste.**

P1 ne cherche pas à ajouter des fonctions. Il cherche à **fermer la chaîne** :
créer → mettre en scène → donner de la logique → habiller (HUD, écrans, audio) →
progression → exporter → rejouer.

## 2. Ce que « jeu 2D complet » signifie (niveau L3)

Un jeu qui possède **les six briques suivantes**, chacune vérifiable :

1. **Un flux d'écrans** : menu principal → jeu → pause → défaite/victoire, avec retour
   au menu. Aucun écran mort, aucun cul-de-sac.
2. **Plusieurs scènes enchaînées** (au moins 3), avec passage de niveau et progression
   conservée.
3. **Une boucle de gameplay complète** : objectif, échec possible, victoire possible.
4. **Un HUD** qui reflète l'état réel (score, vie, objectif) sans mentir.
5. **Un son et des réglages** : effets sonores, volume, mute persistant.
6. **Une sortie** : un fichier HTML5 autonome, jouable hors ligne, sans console
   d'erreur, et un package de projet réimportable.

## 3. Périmètre IN (engagé)

### P1.1 — Écrans et navigation

> **Avancement (10/11/2026)** : machine à états `GameScreen` livrée
> ([`screens.rs`](../crates/engine-core/src/screens.rs), 13 tests) — transitions,
> gel de la simulation, miroir des variables KVL, compatibilité totale quand aucun
> écran n'est déclaré. Reste à faire dans ce lot : le **contenu** des écrans
> (boutons/labels) porté par le jeu de référence (P1.3), et la validation du focus.

- [x] Écrans `menu`, `pause`, `game_over`, `victory` consommés par le runtime (état
      `GameScreen` + rendu overlay ; la résolution par variables KVL est remplacée).
- [x] Transitions : `Entrée`/clic pour valider, `Échap` pour pause, reprise de la partie.
- [ ] Focus clavier fiable : aucune touche « avalée » entre écrans (à valider sur le jeu de référence).
- [x] Aucune régression : le HUD actuel reste tel quel (419 tests verts).

### P1.2 — Progression et sauvegarde

> **Avancement (11/11/2026)** : livré côté moteur et commandes Tauri.
> `SaveState` (projet + **niveau courant** + variables KVL) remplace le simple projet ;
> les anciens slots restent lisibles (niveau 1). `resume_state` reprend la sauvegarde la
> plus récente. Les réglages audio vivent dans `~/.karactirage/settings.json`, séparés.
> `project_id` est désormais **stable** (il dérivait du nom de scène, donc changeait à
> chaque niveau : les sauvegardes partaient dans un dossier différent).
> Limite : aucun menu de slots dans l'interface — les commandes existent
> (`engine_save_game`, `engine_load_game`, `engine_resume_game`), l'écran viendra avec P1.6.

- [x] Sauvegarde de la progression (niveau courant, variables, score) via `save_config`.
- [x] Reprise : rechargement du projet puis `resume` restaure niveau et score (testé ;
      le « quitter/relancer » est simulé par un rechargement complet, pas par un vrai redémarrage).
- [x] Réglages persistants (volume, mute) dans un fichier **hors** sauvegarde de partie.

### P1.3 — Jeu de référence L3

> **Avancement (10/11/2026)** : enchaînement de niveaux livré et **format tranché —
> option A retenue**. `KaractiProject::scenes: Vec<SceneData>` embarque tous les niveaux ;
> `scene` reste le niveau 1 (projets mono-scène normalisés à la lecture). Côté moteur,
> `GameState.levels` / `level_index` / `next_level()` instancient une scène **en mémoire**
> (`GameState::from_scene_data`), sans fichier externe : l'export reste auto-porteur.
> 11 tests couvrent écrans, passage de niveau, score conservé, victoire au dernier niveau,
> normalisation des anciens JSON, export multi-niveaux et redémarrage au niveau 1.
> **Reste : écrire le contenu** (2 niveaux puis un 3ᵉ avec boss) et le prouver de bout en bout.

- [x] Un jeu de démonstration **livré avec le Studio** : `assets/projects/jeu_reference.karacti`
      (3 niveaux embarqués, 4 écrans, `menu_active` au démarrage), visible dans le Hub.
- [x] Un **objectif par scène** (`COLLECTEZ LES 3 PIÈCES…`, `ATTEIGNEZ LA PORTE…`,
      `TRAVERSEZ L'ARÈNE DU TITAN…`).
- [x] Un **3ᵉ niveau avec boss** : arène du Titan Gardien (composant `BossComponent`).
      Limite assumée : pas de système de combat en P1 — le boss est un gardien, le niveau
      se conclut par la porte.
- [x] Une **condition de défaite** : mort du joueur → écran `game_over` (testé).
- [x] Une **condition de victoire** : dernier niveau terminé → écran `victory` (testé).
- [x] Construit **uniquement** avec les outils : scènes, graphes KVL et écrans déclaratifs
      (aucun contenu codé dans le moteur ; le générateur d'exemple ne sert qu'à assembler
      le `.karacti` à partir des scènes).
- [x] Le jeu se termine par une victoire **reproductible de bout en bout** :
      menu → niveau 1 → **interaction de porte réelle (touche E)** → niveau 2 → porte →
      niveau 3 (boss présent) → porte → **victoire**. Le graphe KVL, la variable et la
      machine à états sont traversés pour de vrai.
- [x] **Défaite** couverte (mort → `game_over`), au niveau de la machine à états.

### P1.4 — Audio et réglages

> **Avancement (11/11/2026)** : livré. Les effets couvraient déjà collecte (hausse de
> score), dégâts, porte, mort et victoire ; `PlaySound:<id>` côté KVL était déjà consommé
> par le moteur. Ajouté : son de **passage de niveau**, préservation du périphérique audio
> et des réglages à travers `next_level`/`start_game`/`reload` (auparavant chaque niveau
> recréait l'`AudioEngine` et **perdait volume et mute**), et contrôle volume/mute dans le
> pied de page.
> Le gate d'audit a attrapé une régression de contraste de ma part (`#64748b`, token
> décoratif, utilisé pour le pourcentage) : corrigé avant commit.

- [x] Effets sonores branchés sur les événements réels (dégâts, collecte, porte,
      passage de niveau, victoire, défaite).
- [x] Volume et mute réglables dans l'interface, persistants (`engine_audio_settings`).
- [x] *Hors P1* : fichiers audio, musique externe, bus, spatialisation → **P2 (Kira)**.

### P1.5 — Export et pré-vol minimal

> **Avancement (11/11/2026)** — livré, avec une **découverte majeure** sur AC3.
>
> Ce qui est livré : pré-vol bloquant (`StandaloneExporter::preflight`, 5 tests),
> `export_html_checked`, empreinte stable (`export_fingerprint`), CLI
> `export_game`, test de fumée navigateur (`apps/studio/scripts/export-smoke.mjs`),
> commandes Tauri `engine_export_preflight` / `engine_export_standalone_html_checked`.
>
> **Ce que l'audit a révélé** : l'export HTML5 n'était **pas jouable**. Le template
> importait `./pkg/engine_wasm.js` (interdit en `file://` : CORS) et retombait sur une
> carte statique, tout en affichant « Prêt à jouer ». Deux corrections : plus aucune
> police externe (Google Fonts supprimés), import du runtime conditionné au `http(s)`,
> message honnête. Le test de fumée sur l'export réel du jeu de référence passe
> désormais : **9 entités, 0 erreur console, 0 requête réseau**.
>
> **Reste bloqué** : le **runtime jouable embarqué**. Le WASM se charge (glue + binaire
> inlinés par blobs testés avec succès) mais `NativeEngine::init` panique sur
> `std::env::temp_dir` : il n'existe pas d'initialisation wasm sans système de fichiers.
> AC3 est donc **partiellement atteint** : auto-porteur et hors ligne, mais le jeu
> affiche une carte d'identification. Le runtime embarqué est reporté en **P6**.

- [x] Export HTML5 autonome : un seul fichier, **0 requête réseau, 0 erreur console**
      (vérifié par `export-smoke.mjs`) — mais **non jouable** tant que le runtime wasm
      n'a pas d'init sans disque (P6).
- [x] Export du package de projet réimportable (inchangé, couvert par les tests existants).
- [x] Pré-vol : refus d'export si un script KVL ou un asset référencé manque.
- [x] Empreinte d'export stable (même projet → même contenu).
- [x] Les scripts référencés par les entités sont chargés à l'ouverture d'un projet
      (auparavant `kvl_graphs` restait vide : l'export perdait toute la logique).

### P1.6 — Preuves et documentation

> **Avancement (11/11/2026)** : livré. Le parcours de bout en bout passe par une
> **interaction réelle** (touche E sur la porte) ; le test de fumée navigateur valide
> l'export (0 erreur, 0 requête) ; le guide et le rapport v0.3.0 existent.
> Écart consigné : ce n'est pas un **replay** hashé (AC2 partiel), et l'export n'est
> pas encore jouable (AC3 partiel).

- [x] Test de bout en bout : menu → 3 niveaux → victoire **par interaction de porte**
      (déterministe, mais pas via `ReplayManager` : AC2 partiel).
- [x] Test d'export : le HTML produit **démarre** dans Chrome headless, sans erreur ni
      requête réseau (il affiche la carte d'identification, pas le jeu animé).
- [x] Audit UI `--strict` : 0 violation, 0 overflow, baseline à jour.
- [x] Guide « créer ce jeu en 10 minutes » avec captures :
      [`GUIDE_CREER_UN_JEU_10_MIN.md`](GUIDE_CREER_UN_JEU_10_MIN.md).
- [x] [`RAPPORT_v0.3.0.md`](RAPPORTS/RAPPORT_v0.3.0.md) rempli avec les preuves.

### Statut de P1

| Lot | État |
|---|---|
| P1.1 Écrans + navigation | ✅ |
| P1.2 Progression + sauvegarde | ✅ (UI de slots manquante) |
| P1.3 Jeu de référence | ✅ 3 niveaux, boss gardien (sans combat) |
| P1.4 Audio + réglages | ✅ |
| P1.5 Export + pré-vol | ✅ pré-vol ; **export jouable ❌ (P6)** |
| P1.6 Preuves + rapport | ✅ |

**Décision de version** : v0.3.0 peut être taguée **en connaissance de cause** — AC3
partiel et AC2 partiel sont inscrits au rapport. Le runtime embarqué est le premier
chantier de P6.

## 4. Périmètre OUT (explicitement exclu de P1)

| Exclu | Pourquoi | Où c'est traité |
|---|---|---|
| Import Tiled `.tmx` | confort de création, pas nécessaire pour fermer la boucle | P2 |
| Fichiers audio, musique, Kira | le son procédural suffit à habiller P1 | P2 |
| Manifeste d'assets complet | aucun asset externe en P1 (sprites procéduraux / pxl) | P2 |
| glTF, 3D, 2.5D, Blender MCP | autre palier de complexité | P4 |
| Génération d'images/audio par IA | coût et dépendance inutiles pour P1 | P3 |
| Export desktop et WASM | HTML5 remplit la promesse « publier » | P6 |
| Nouvelles ops IA (au-delà des 31) | la barre inline est **gelée** | après v0.3.0 |
| Multijoueur, rollback netcode | hors produit v1 | jamais en v1 |
| Physique 3D | hors scope assumé | L6 |

**Gel explicite** : la barre d'édition IA (`InlineAiBar`, `AiSparkle`) et les 31 ops sont
**figées**. On corrige les bugs, on n'ajoute pas de surface. Aucun ghost, aucune barre
KVL, aucun batch tant que v0.3.0 n'est pas publié.

## 5. Critères d'acceptation (mesurables)

| # | Critère | Mesure |
|---|---|---|
| AC1 | Un nouveau venu obtient un jeu jouable | « Nouveau jeu » → jouable en **< 2 min**, chronométré, sans documentation |
| AC2 | Le jeu de référence se termine | replay déterministe : menu → victoire, **même hash de replay à chaque exécution** |
| AC3 | L'export est autonome | HTML unique, **0 requête réseau**, **0 erreur console** au démarrage |
| AC4 | La progression survit | quitter → relancer → scène et score restaurés |
| AC5 | Aucune régression | `cargo test --workspace` ≥ 406 verts, `audit:ui --strict` 0 violation, lint 0 erreur / ≤ 14 warnings |
| AC6 | Le jeu de référence n'utilise que les outils | 0 entité/écran modifié par du code de moteur |

**AC2 est le critère qui vaut signature** : si le replay ne reproduit pas la partie, la
déterminisme revendiqué est une affirmation sans preuve.

## 6. Découpage et ordre

| Lot | Contenu | Sortie vérifiable |
|---|---|---|
| P1.1 | Écrans + navigation | on passe du menu au jeu et on revient |
| P1.3 | Jeu de référence (2 scènes) | on joue, on perd, on gagne |
| P1.2 | Progression + sauvegarde | on ferme, on rouvre, on reprend |
| P1.3b | 3ᵉ scène + boss | la boucle complète existe |
| P1.4 | Audio branché | les événements sonnent |
| P1.5 | Export + pré-vol | le fichier HTML se partage |
| P1.6 | Preuves + rapport | le dossier de v0.3.0 est complet |

Ordre non négociable : **on n'exporte pas avant que le jeu se termine** (AC2 avant AC3),
et **on ne branche pas l'audio avant d'avoir un jeu qui se termine** (éviter de sonoriser
une boucle cassée).

## 7. Risques de P1

| Risque | Signal d'alerte | Mitigation |
|---|---|---|
| La navigation d'écrans empiète sur le HUD | régressions visuelles | un seul point d'entrée `Mode`, tests d'écrans |
| Le replay n'est pas assez déterministe | hash instable | figer le pas fixe et l'ordre d'itération avant d'écrire le test |
| Le jeu de référence dérive en « démo tech » | 2 semaines sans fin | 3 scènes maximum, contenu borné |
| La tentation d'ajouter des ops IA | PR hors AC1-AC6 | le gel est écrit ici |

## 8. Definition of Done de v0.3.0

- [ ] AC1 à AC6 satisfaits et **prouvés** dans `RAPPORT_v0.3.0.md`
- [ ] Le jeu de référence est livré dans le Studio (accessible depuis le hub)
- [ ] Toutes les gates vertes (tests, build, lint, audit strict)
- [ ] Aucun fichier généré par les tests commité
- [ ] Le rapport de version est écrit **avant** le tag
