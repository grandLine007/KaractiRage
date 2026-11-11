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
- [ ] Effets sonores existants branchés sur les événements réels (dégâts, collecte, porte,
      victoire, défaite).
- [ ] Volume et mute réglables dans l'interface, persistants.
- [ ] *Hors P1* : fichiers audio, musique externe, bus, spatialisation → **P2 (Kira)**.

### P1.5 — Export et pré-vol minimal
- [ ] Export HTML5 autonome : un seul fichier, sans réseau, sans erreur console.
- [ ] Export du package de projet réimportable.
- [ ] Pré-vol : refus d'export si un script KVL ou un asset référencé manque.
- [ ] Empreinte d'export stable (même projet → même contenu, hors date).

### P1.6 — Preuves et documentation
- [ ] Test de bout en bout : un replay déterministe traverse menu → 3 scènes → victoire.
- [ ] Test d'export : le HTML produit démarre et affiche le menu (navigateur headless).
- [ ] Audit UI `--strict` : 0 violation, 0 overflow, baseline à jour.
- [ ] Guide « créer ce jeu en 10 minutes » avec captures.
- [ ] `RAPPORT_v0.3.0.md` rempli avec les preuves.

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
