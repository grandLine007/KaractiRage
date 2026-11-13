# Plan d'intégration des nouveaux outils (P2 → P6)

> **Statut** : proposition — 10 novembre 2026. **Flexible par construction**, borné par
> les invariants du §7. Chaque écart réel sera consigné dans le rapport de la version
> concernée.
> **Complète** : [STRATEGIE_CREATION_2D_3D_IA.md](STRATEGIE_CREATION_2D_3D_IA.md) (arbitrage
> des crates) et [P1_JEU_2D_COMPLET.md](P1_JEU_2D_COMPLET.md) (gel de la v0.3.0).

---

## 0. Cadre

L'objectif final ne change pas : **tout créateur, sur une machine modeste, conçoit,
prototype et publie un jeu de haute tenue, avec une IA contrôlée et un runtime
souverain.** Les nouveaux outils (Tiled, glTF, Kira, packs MCP, Blender, générateurs)
ne sont que des **moyens d'alimenter la chaîne** asset → scène → logique → export.

| Version | Phase | Thème | Sortie démontrable |
|---|---|---|---|
| v0.2.0 | — | Édition IA inline | livrée |
| **v0.3.0** | **P1** | Boucle jeu 2D complète | un jeu L3 exporté |
| v0.4.0 | P2 | Pipeline d'assets, Tiled, audio | niveau Tiled + musique |
| v0.5.0 | P3 | Packs image/audio locaux | sprite généré → scène |
| v0.6.0 | P4 | glTF, 3D déclarative, Blender | scène 3D exportée |
| v0.7.0 | P5 | Qualité IA mesurée | rapport 40×10 |
| v1.0.0 | P6 | Exports desktop/WASM | 3 cibles vérifiées |

---

## P2 — Pipeline d'assets (v0.4.0)

### P2.1 — Manifeste d'assets

**Objectif spécifique** : rendre tout contenu externe **traçable, borné et
reproductible**, avant d'en accepter un seul octet.

**Fonctionnalités**
- [ ] `assets/manifest.json` versionné : id, chemin, type, hash, taille, dimensions/durée,
      licence, **provenance** (outil ou modèle qui l'a produit, paramètres).
- [ ] Budget par type : pixels par texture, octets par son, octets par maillage, budget
      global du jeu.
- [ ] Lecture stricte : un asset non déclaré est refusé à l'import **et** au chargement.
- [ ] Commande de vérification (`cargo run -p engine-core --bin assets_check`) qui sort 0/1,
      utilisable en CI.
- [ ] Gate d'export : pré-vol bloquant (asset absent, budget dépassé, hash incohérent).

**Impacts**
- *Format* : le projet référence des assets par id, plus par chemin libre.
- *IA* : les ops `AddMesh`/`sprite_asset` passent par le manifeste (validation renforcée).
- *CI* : nouveau test de non-dérive `assets_check`, moins coûteux que le dictionnaire IA.
- *Export* : l'export ne part plus que du manifeste — condition de la reproductibilité.

**Résultats attendus**
- Un jeu qui référence un asset absent **ne s'exporte pas**, avec la liste exacte.
- Deux imports du même fichier donnent le même hash.
- Le budget refusé est expliqué en langage créateur (« 14 Mo de textures pour 8 Mo
  autorisés »).

**Flexibilité** : le format exact du manifeste peut évoluer (JSON → JSON + sidecar), mais
**la provenance et le budget ne sont pas négociables**.

---

### P2.2 — Import Tiled (`.tmx` / `.tsx`)

**Objectif spécifique** : permettre la conception de niveaux 2D dans un outil gratuit et
répandu, sans perdre la logique KaractiRage.

**Fonctionnalités**
- [ ] Import d'une carte `.tmx` : taille, tuiles, calques, objets.
- [ ] Tuiles → entités `wall`/`plate`/`decor` selon un **mapping déclaré** (fichier de
      correspondance, pas de heuristique cachée).
- [ ] Calque de collision → `Collider2D` (solide/déclencheur/one-way).
- [ ] Objets nommés → entités typées (joueur, porte, pièce, danger, boss).
- [ ] `properties` Tiled → `tag`, `script_path`, propriétés (vitesse, vie).
- [ ] Import idempotent : réimporter met à jour sans dupliquer.
- [ ] Rapport d'import : ce qui a été importé, ce qui a été ignoré et pourquoi.

**Impacts**
- *Dépendance* : `tiled` 0.16 (léger, 0,3 M téléchargements).
- *Format* : les tuiles importées deviennent des entités comme les autres (aucun statut
  spécial), donc éditables et annulables par l'IA.
- *UX* : nouvel écran/entrée « Importer une carte Tiled » dans le hub.
- *Tests* : un `.tmx` de référence dans `assets/test/` + test de non-régression.

**Résultats attendus**
- Une carte 20×20 s'importe en **< 1 s**, sans perte de collision.
- Le créateur peut modifier à la main ce que Tiled a produit, puis réimporter.
- Un niveau importé est jouable dans la seconde, sans script.

**Flexibilité** : on peut ne pas supporter les calques isométriques ni les animations de
tuiles en v1 de l'import — **à condition** que l'import le dise explicitement au lieu de
les ignorer silencieusement.

---

### P2.3 — Audio par fichiers (Kira)

**Objectif spécifique** : passer de bruitages procéduraux à une **bande-son de jeu**,
sans compromettre le déterminisme de la simulation.

**Fonctionnalités**
- [ ] Trait `AudioBackend` avec deux implémentations : `Procedural` (tinyaudio, existant)
      et `Assets` (Kira : OGG/WAV, bus, volume, boucle, fondu).
- [ ] Musique de fond par scène (boucle sans coupure), effets superposables.
- [ ] Volume musique / effets séparés, mute, persistés.
- [ ] Aucune influence sur la simulation : l'audio ne lit jamais l'état du jeu en avance,
      ne bloque jamais une frame.
- [ ] Repli silencieux si aucun périphérique audio (CI, serveur, machine muette).

**Impacts**
- *Dépendances* : `kira` 0.12 + `symphonia` (décodage). ALSA requis sur Linux (déjà traité
  en CI).
- *Perf* : budget < 5 % CPU, < 16 Mo de mémoire audio.
- *Déterminisme* : replay **inchangé** — l'audio est un observateur, jamais une entrée.

**Résultats attendus**
- 1 musique + 8 effets simultanés sans crackle ni coupure.
- Le replay de P1 reste identique avec et sans audio (preuve d'indépendance).
- Machine sans audio : le jeu tourne, silencieusement, sans erreur.

**Flexibilité** : `rodio` au lieu de `kira` si Kira s'avère trop lourd ; un seul bus au
lieu de plusieurs si le budget l'exige.

---

### P2.4 — Rechargement à chaud (notify)

**Objectif spécifique** : réduire la boucle d'itération du créateur à zéro action
manuelle.

**Fonctionnalités**
- [ ] Surveillance de `assets/` et `assets/scripts/` en développement uniquement.
- [ ] Rechargement d'un script KVL modifié, d'une texture remplacée, d'un manifeste.
- [ ] Rejet propre d'un fichier invalide : on garde l'ancienne version, on affiche l'erreur.
- [ ] Désactivable (CI, mode jeu, machine lente).

**Impacts**
- *Dépendance* : `notify`.
- *Sécurité* : chemins confinés au projet ; jamais d'exécution arbitraire.
- *UX* : toast « script rechargé » ; aucun rechargement pendant une partie.

**Résultats attendus** : modifier un `.kvl.json` dans un éditeur externe se voit en **< 1 s**
dans le Studio, sans redémarrage.

**Flexibilité** : peut se limiter aux scripts KVL si la surveillance des binaires pose
problème.

---

## P3 — Packs image/audio locaux (v0.5.0)

### P3.1 — Contrat de fournisseur unique (compatible OpenAI)

**Objectif spécifique** : ne pas dépendre d'un fournisseur, et rendre Ollama, LM Studio et
LocalAI interchangeables.

**Fonctionnalités**
- [ ] Un seul client HTTP « endpoint compatible OpenAI » : chat, JSON structuré, images.
- [ ] Profils déclaratifs (nom, endpoint, modèle, budget de tokens, support du schéma forcé).
- [ ] Détection de capacité : le Studio sait si le modèle accepte un JSON Schema ; sinon il
      retombe sur le parse + repair.
- [ ] Aucun secret dans le projet : configuration hors dépôt, clé au trousseau.

**Impacts** : remplace les intégrations par fournisseur ; réduit la surface de code ;
facilite P5 (mesure).

**Résultats attendus** : la même instruction, écrite une fois, fonctionne sur les trois
backends ; le rapport de latence les compare.

**Flexibilité** : additifs spécifiques (ex. paramètre `format` d'Ollama) autorisés s'ils
dégradent proprement.

### P3.2 — Pack image (sprite, tuile, planche)

**Objectif spécifique** : produire un sprite utilisable **sans cloud**, avec provenance.

**Fonctionnalités**
- [ ] Outil « générer un sprite » → écrit dans `assets/sprites/`, inscrit au manifeste.
- [ ] Chaîne pxl / Aseprite (existante pour pxl) et backend image (ComfyUI via MCP ou
      endpoint images) derrière le **même** outil.
- [ ] Prévisualisation avant validation ; refus si dimensions/budget hors cadre.
- [ ] Provenance enregistrée (modèle, prompt, seed) — condition de reproduction.

**Impacts** : premier vrai usage du manifeste en écriture ; la licence devient une donnée
du projet ; le pack peut être absent (repli sur pxl).

**Résultats attendus** : sprite généré → manifeste → entité dans la scène, **sans étape
manuelle**, en < 30 s en local sur GPU modeste.

**Flexibilité** : si ComfyUI est trop lourd, pxl + import manuel reste un chemin valide ;
le pack n'est jamais obligatoire.

### P3.3 — Pack audio (optionnel)

**Objectif spécifique** : générer un effet ou une boucle courte quand le créateur n'a pas
d'asset.

**Fonctionnalités** : génération d'un effet < 3 s, d'une boucle < 30 s ; écriture manifeste ;
licence ; refus si le format n'est pas supporté par le moteur.

**Résultats attendus** : 1 effet généré réutilisable dans KVL comme un asset normal.

**Flexibilité** : phase entièrement optionnelle (peut glisser après v0.5.0).

---

## P4 — glTF, 3D déclarative et Blender (v0.6.0)

### P4.1 — Import glTF validé

**Objectif spécifique** : accepter du contenu 3D sans casser le budget low-end.

**Fonctionnalités**
- [ ] Import `.glb`/`.gltf` via `gltf` 1.4 → maillages, matériaux, lumières, caméras.
- [ ] Validation : triangles, textures, octets ; refus au-delà du budget.
- [ ] Compression `meshopt` (maillage) et textures bornées (≤ 512 px en low-end).
- [ ] Asset inscrit au manifeste avec hash + licence.
- [ ] Prévisualisation three.js dans le Studio avant commit.

**Impacts** : premier asset binaire du projet ; le format d'export HTML doit embarquer les
maillages compressés ; le pré-vol doit estimer la taille.

**Résultats attendus** : un GLB de 5 Mo s'importe, se valide et s'affiche ; le même GLB en
version 60 Mo est **refusé** avec la raison.

**Flexibilité** : pas d'animations squelettiques en v1 de l'import (L6 hors scope).

### P4.2 — Édition 3D déclarative et 2.5D

**Objectif spécifique** : rendre l'assemblage 3D aussi simple que le placement 2D.

**Fonctionnalités**
- [ ] Caméra d'édition (orbite, cadre), gizmo de translation/rotation/échelle.
- [ ] Placement assisté : grille, aimantation, duplication en ligne.
- [ ] Ops IA 3D déjà présentes (`SetTransform3D`) exposées dans la barre inline.
- [ ] 2.5D : hauteur/profondeur, sprites billboard, tri par profondeur.
- [ ] Lumières et caméra éditables comme des objets.

**Impacts** : le viewport three.js devient un éditeur (pas seulement une prévisualisation) ;
la sélection doit être partagée entre React et le moteur.

**Résultats attendus** : une scène 2.5D de 100 entités tourne à 60 FPS en profil low-end ;
une scène 3D de 50 objets s'assemble en < 10 min par un non-informaticien.

**Flexibilité** : le gizmo peut être minimal (translation seule) en v1 ; la rotation par
champs numériques reste acceptable.

### P4.3 — Blender MCP (optionnel)

**Objectif spécifique** : automatiser la préparation d'assets répétitive sans intégrer
Blender au produit.

**Fonctionnalités**
- [ ] Pack MCP déclaré (`ToolPackManifest`) : outils `export_gltf`, `apply_material`,
      `decimate`, `bake_transform`.
- [ ] Chaque appel produit un asset **inscrit au manifeste**, jamais un fichier orphelin.
- [ ] Blender absent ou fermé → le pack est marqué indisponible, le Studio reste utilisable.
- [ ] Version de Blender et paramètres enregistrés en provenance.

**Impacts** : premier processus externe piloté ; gestion des timeouts et des échecs ;
documentation d'installation (addon + socket).

**Résultats attendus** : « importe ce dossier, réduis à 5 k triangles, exporte en GLB » en
une commande, avec 0 fichier hors manifeste.

**Flexibilité** : **peut être retiré entièrement** — l'import manuel d'un GLB suffit à
livrer la 3D déclarative. C'est un accélérateur, pas une fondation.

---

## P5 — Qualité IA mesurée (v0.7.0)

**Objectif spécifique** : remplacer les impressions par des chiffres sur les petits modèles.

**Fonctionnalités**
- [ ] Corpus 40 instructions × 10 cibles (2D, KVL, 3D) versionné.
- [ ] Runner Ollama / LM Studio / LocalAI ; mode « template seul » comme plancher.
- [ ] Métriques : cible correcte, première op valide, no-op, latence médiane, tokens.
- [ ] Rapport publié dans `docs/RAPPORTS/` ; seuils bloquants pour la release.
- [ ] Comparaison automatique avec la version précédente (régression = alerte).

**Impacts** : chaque changement de prompt devra être justifié par une mesure ; le coût de
CI augmente (nightly, pas à chaque push).

**Résultats attendus**
- Cible correcte ≥ 95 %, op valide ≥ 95 %, no-op silencieux = 0.
- Latence médiane locale < 15 s sur un 7-8B.
- Un rapport lisible par un non-spécialiste : « ce modèle fait X, l'ancien faisait Y ».

**Flexibilité** : les seuils peuvent être ajustés une fois, **avec justification écrite** ;
le corpus peut grandir, jamais rétrécir.

---

## P6 — Exports (v1.0.0)

**Objectif spécifique** : publier depuis le Studio, sur trois cibles, avec la même preuve.

**Fonctionnalités**
- [ ] HTML5 autonome (existant, durci par le pré-vol).
- [ ] Desktop : bundle (launcher + assets + manifeste) via `zip`.
- [ ] WASM : chaîne `engine-wasm` finalisée.
- [ ] Pré-vol commun : assets, budget, scripts, licences manquantes.
- [ ] Reproductibilité : hash d'export stable, documenté.
- [ ] Le jeu exporté **ne contient ni IA ni MCP**.

**Résultats attendus** : le jeu de référence s'exporte dans les 3 cibles et passe le même
scénario de test headless ; tailles documentées ; aucun réseau requis.

**Flexibilité** : l'ordre desktop/WASM peut s'inverser selon les blocages techniques ; les
cibles peuvent être ramenées à deux si la troisième coûte plus qu'elle ne prouve.

---

## 7. Invariants (ce qui ne peut pas changer)

Même en cas de contrainte technique, ces cinq points ne se négocient pas :

1. **Le runtime exporté est souverain** : aucune IA, aucun MCP, aucun cloud requis.
2. **Les données sont déclaratives** : aucune op ne génère de code exécutable.
3. **Toute modification est une op validée, annulable, traçable.**
4. **Les budgets low-end sont des gates**, pas des recommandations.
5. **L'objectif final prime** : un créateur sur machine modeste doit pouvoir finir un jeu.

## 8. Règles de flexibilité

- Un outil peut être **remplacé** (Kira ↔ rodio, ComfyUI ↔ endpoint images) sans toucher
  aux invariants.
- Un outil peut être **retiré** (Blender MCP, pack audio) : il est un accélérateur.
- Une phase peut **glisser** ou **se scinder**, à condition de tenir la version suivante.
- Tout écart est **consigné** dans le rapport de version : nature, raison, conséquence,
  décision de rattrapage.
- Ce qui ne peut pas changer : les critères AC1-AC6 de P1 et les cinq invariants.

## 9. Résultat final attendu (v1.0.0)

> Sur une machine modeste et sans connexion, un créateur ouvre KaractiRage Studio, choisit
> un modèle de jeu, dessine ses niveaux dans Tiled ou dans le Studio, génère ses sprites en
> local, assemble une scène 3D déclarative depuis des GLB validés, écrit sa logique en KVL
> avec l'aide d'un 7B contraint, règle son audio, puis exporte un jeu HTML5, desktop ou WASM
> qui se lance hors ligne, sans IA, et se comporte de façon identique à chaque exécution.

Concrètement, à v1.0.0 :

| Pilier de l'objectif final | Preuve |
|---|---|
| Accessibilité low-end | 60 FPS sur profil bas, budgets d'assets bloquants, tailles d'export publiées |
| Continuum KVL | guidé → visuel → expert sur un même modèle de données, sans rupture |
| IA contrôlée | plan → patch → test → rollback, mesuré sur 40 instructions, 0 no-op |
| Runtime souverain | 3 exports, 0 réseau, 0 IA embarquée, reproductibilité par hash |
