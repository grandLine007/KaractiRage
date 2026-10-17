# ⚡ CRITIQUE DESIGN — MI-PARCOURS
### Interface de la desktop app KaractiRage (édition native Dark)

> **Statut** : revue de mi-parcours — état du code au 16 octobre 2026
> **Sources** : `KARACTIRAGE_DESIGN_DOCUMENT_DARK.md`, `MOTEUR-DE-JEU-PRINCIPES.md`, code source (`main.rs`, `game.rs` [8 203 l.], `editor.rs`, `renderer.rs`, `dialogue.rs`, `inventory.rs`), `web/`
> **Objectif** : rendre l'app **facile à prendre en main pour un débutant**, **agréable et conforme à la vision Dark Atelier**, et **prête à accueillir des modifications pilotées par une IA**.

---

## 1. Verdict en 30 secondes

Le moteur est impressionnant pour un stade mi-parcours : 5 genres jouables, Studio KVL, guide pédagogique, palette déjà très conforme au design doc. **Mais l'app se comporte comme un outil d'ingénieur, pas comme un produit qu'un débutant ouvre pour la première fois.** Trois écarts majeurs :

1. **Aucun état de survol, aucune infobulle, nulle part dans l'UI** — les 20+ raccourcis clavier sont ensevelis dans une modale-texte (H), et le premier lancement ressemble à… rien (les messages d'accueil partent dans `println!` du terminal).
2. **Le bandeau supérieur est une liste à puces de 11–13 badges** sans hiérarchie ni regroupement, en layout fixe non responsive (fenêtre mini non contrainte → collisions en dessous de ~1 000 px).
3. **La place de l'IA est structurellement vide** — zéro occurrence d'IA/LLM dans le code — alors que les fondations (undo/redo, validateur KVL statique, wizard guidé, halos violets de patch) sont **exactement** ce qu'il faut pour l'accueillir proprement.

---

## 2. Ce qui est déjà juste (ne pas casser)

| Élément du design doc | État dans le code | Verdict |
|---|---|---|
| Palette Dark Atelier (`#38BDF8` cyan, `#A855F7` violette, `#F59E0B` ambrée, `#10B981` émeraude, `#EF4444` rouge, `#334155` arêtes, `#0F172A` panneaux) | Utilisée quasi à l'identique (`0x38BDF8`, `0xA855F7`, `0x334155`, `0x0F172A`…) | ✅ 95 % conforme |
| Cornières technologiques `draw_corner_brackets` | Présent, utilisé (viewport, panneaux) | ✅ |
| Badges/pilules d'action `draw_badge` | Présent, centré pixel-perfect | ✅ |
| Console KVL réactive en bandeau bas | Drawer compact toujours-visible | ✅ |
| Arêtes de câbles KVL colorées (signal clair / ambré / effacement rouge / **patch violet**) | Couleurs codées dans l'éditeur | ✅ (base parfaite pour l'IA) |
| Undo/redo (Ctrl+Z / Ctrl+Y) avec messages « Annulé : … / Rétabli : … » | Stack opérationnel | ✅ |
| Guide pédagogique H + Assistant Guidé G (3 étapes) | Modale 760×560 + wizard 6 catégories | ✅ Fonctionnel, ⚠️ présentation (§4) |
| Police de titre characterful (Space Grotesk Bold) + mono (JetBrains Mono) | Bundlées via `include_bytes!` | ✅ (⚠️ un seul graisse, §4.4) |

---

## 3. Le parcours du débutant, aujourd'hui (jour 1, honnête)

```
1. L'app s'ouvre sur une scène top-down déjà pleine d'entités.
2. Le joueur voit : bandeau de 11 badges, HUD, bandeau bas avec une
   ligne de statut… et aucun indice de quoi faire.
3. Les messages d'accueil ("Première fois ? Appuyez sur H")
   sont dans le TERMINAL (println! de main.rs) → invisibles si
   lancée depuis le Finder/double-clic.
4. Le débutant clique au hasard. TAB bascule en Studio. L'inspecteur
   KVL (nœuds, pins, pins d'entrée) s'affiche en pleine puissance,
   comme pour un expert.
5. F1 = aide technique de debug. H = mur de texte 760×560 en MAJUSCULES,
   non scrollable, sans progression.
6. Au bout de 10 minutes : le débutant sait que le moteur est puissant,
   il ne sait toujours pas ce qu'il peut faire en 2 clics.
```

Le problème n'est pas la puissance de l'app, c'est **l'absence de progression du regard** : tout est visible, tout est au même niveau visuel, rien ne se met en avant.

---

## 4. Problèmes d'ergonomie, classés par gravité

### 🔴 P1 — Aucun feedback de survol / infobulle / focus (grep `hover|tooltip` → 1 résultat, un commentaire)
- `draw_badge` ne reçoit jamais la position souris : impossible de savoir qu'un bouton est cliquable.
- Aucun état `:hover`, aucun `:focus`, aucun changement de curseur.
- Le design doc promet §11.3 « feedback au survol immédiat » — non implémenté.
- **Impact débutant** : l'app entière donne l'impression d'une image, pas d'un outil. C'est le défaut n°1.

### 🔴 P1 — 20+ raccourcis clavier, discoverables uniquement via une modale-texte
TAB, H, F1–F4, G, N, P, C+D, Suppr, V, B, X, T, E, ~, Espace, E/F clic, Ctrl+Z/Y… Tous ensevelis dans le guide H. Les badges du header mélangent parfois la touche (`B: PINCEAU`), parfois non (`SAVE`, `+ ENTITÉ`) — **incohérence de nommage**.
- **Impact débutant** : l'efficacité « power user » existe mais est invisible.

### 🔴 P1 — Bandeau supérieur surchargé et non hiérarchisé
Jeu : ~11 badges en ligne (save, undo, redo, guider, entités, radar, qualité, profil, télémétrie, reset, aide, son). Studio : ~13.
- Tout a le **même poids visuel** : « Sauvegarder » et « Télémétrie CPU » sont des badges jumeaux.
- Layout en positions X figées, **aucune contrainte de taille de fenêtre** (`with_min_inner_size` absent ; le clamp logique va jusqu'à 400×300 alors que la modale d'aide fait 760×560 fixe) → sous ~1 000 px de large, le header entre en collision et la modale H déborde de l'écran.

### 🟠 P2 — Aucun onboarding premier lancement
- L'accueil n'existe que dans le `println!` du terminal (`main.rs:36-41`).
- La carte « BIENVENUE DANS KARACTIRAGE » (inspecteur, selection vide) n'apparaît **qu'en mode Studio sans sélection**.
- En mode Jeu : rien. Objectif présent (bonne idée de pilule cyan « OBJECTIF »), mais aucun chemin « Apprendre / Créer / Jouer ».

### 🟠 P2 — `status_message` persiste éternellement
Les ~40 assignations (création, dup, suppression, attach composant…) ne sont **jamais effacées ni estampées**. Le bandeau bas garde indéfiniment « Entité 'X' créée avec succès ! » — il devient du bruit au lieu d'un signal.

### 🟠 P2 — Typographie mono-graisse, tout en capitales
- Une seule graisse chargée : `SpaceGrotesk-Bold` pour **tout** le texte UI (corps 11→17 px), `JetBrainsMono-Bold` pour la console.
- ~100 % des libellés en MAJUSCULES (« GUIDE RAPIDE », « COMMANDES RAPIDES », « SAUVEGARDER… ») → les phrases en capitales sont 10–15 % plus lentes à lire, surtout en français accentué.
- Corps de texte à 11–12 px : sous la lisibilité confortable sur écran non-Retina (la cible « PC modestes » justement).
- Le design doc prévoit une **fonte UI de corps** (type Outfit) : elle n'est pas bundlée ; seule la graisse Bold existe.
- Aucune gestion de retour à la ligne dans `draw_text` → chaque appelant tronque à la main (découpage par bytes à risque : `speaker_str[..3]` dans `entity.rs:1043` peut panic sur une initiale multibyte type `Éé`).

### 🟡 P3 — Le continuum « guidé → expert » n'est pas modulé
Le design doc et le rapport 30 promettent un mode guide ; en pratique **l'inspecteur complet (nœuds KVL, pins, composants, script binding) est exposé dès le premier jour**, au même niveau que le pinceau. Un débutant n'a pas besoin de voir les pins d'un nœud avant d'avoir placé 3 entités.

### 🟡 P3 — Guide H = mur de texte non paginé
Une seule modale, tout en capitales, sans onglets, sans état « lu/lu pas », sans lien cliquer-monde. Le wizard G est bien paginé (3 questions) — c'est le bon modèle, à étendre.

### 🟡 P3 — Pas d'état vide « scénario sans entité » / pas de toasts
Les états vides existent pour l'inspecteur et la console, pas pour la scène. Les erreurs vont dans la ligne de statut au lieu de toasts contextuels.

---

## 5. Recommandations

### Lot 1 — « Prendre en main » (le parcours jour 1) — **priorité haute, effort moyen**

1. **Écran d'accueil au premier lancement** (drapeau persistant) : 3 cartes
   `▶ JOUER la démo` · `🛠 CRÉER une scène` · `🎓 APPRENDRE (5 min)` — avec l'esthétique Dark Atelier (cornières, cartes `#0F172A`, accent cyan). Au retour ensuite : plus jamais visible (ou accessible via `?`).
2. **États de survol partout** : passer la position souris à `draw_badge` (elle est déjà dans `InputState`) → variante `hover` (bord cyan + fond éclairci +2 niveaux), pressed (scale interne), et **tooltip sous le badge** affichant `Libellé — Touche` après 400 ms. C'est le lot le plus rentable de toute la liste.
3. **Barre d'outils restructurée** :
   - Rail vertical gauche (comme les éditeurs modernes) pour les outils Studio : Sélection / Entité / Pinceau / Gomme / Tuile / Dup / Suppr.
   - Header gardé pour : logo · sélecteur de scène · bascule JEU/STUDIO · `↶ ↷` · **CTA principal unique** (en mode Jeu : `▶ JOUER`, en Studio : `＋ CRÉER` ou `✨ DEMANDER À L'IA`).
   - Secondaire (radar, qualité, profil, télémétrie, reset, son) → **menu « ⚙ » déroulant**. Le header passe de 13 badges à ~6.
4. **Taille de fenêtre mini** : `with_min_inner_size(1024, 640)` + garde-fous : si largeur < 1 000, masquer les badges secondaires (déjà prévus par la logique de droite vers la gauche) et agrandir la modale H en `min(760, w-80)`.
5. **Toasts temporaires** : `status_message` avec horodatage, fondu après 4 s ; les messages persistants (erreurs) restent en rouge dans la barre basse.
6. **Guide H refondu en 3 onglets** : `DÉCOUVRIR` (parcours en 6 étapes cochables, avec le monde réel derrière chaque étape) · `RACCOURCIS` (grille par catégorie, pas un pavé) · `CONCEPTS` (ECS, KVL, genres). Fermeture par Échap, réouverture à l'étape suivante.

### Lot 2 — Conformité Dark Atelier (lisibilité & identité) — **priorité moyenne, effort faible**

7. **Typographie à 3 niveaux** : charger aussi `SpaceGrotesk.ttf` (déjà dans `assets/fonts/` !) comme graisse *regular* → Bold réservé aux titres/badges, Regular pour le corps. Corps de texte min **14 px**.
8. **Sentence case pour les phrases, capitales réservées aux badges/courts libellés** : « Guide rapide » au lieu de « GUIDE RAPIDE » dans les paragraphes ; capitales OK pour `SAVE`, `UNDO`… — mais uniformiser : soit `B: PINCEAU`, soit `PINCEAU (B)`, partout le même schéma (libellé + touche entre parenthèses).
9. **Fond écran** : aligner `canvas.clear` sur `#020617` (actuellement `0x00080B14`) — détail, mais le design doc est immuable sur la palette.
10. **Chip de profil permanent** comme prévu au design doc : `PROFIL: Low-End / 60 FPS [CPU: —] [Mem: —]` dans le header au lieu du badge F1 isolé (utile aussi pour la promesse « PC modestes »).

### Lot 3 — La place de l'IA (le vrai gap) — **priorité haute pour la vision**

État des lieux : **aucune ligne d'IA dans le projet**. Le rapport 30 a démontré le *pattern* (intent → graphe KVL), mais il est déterministe et fermé (6 catégories × 3 questions).

**Phasage recommandé, sans casser l'architecture :**

- **Phase A — « Intent bar » locale (aucun réseau)** : un champ `✨ Décrire ce que je veux…` dans le header/Studio qui n'est autre qu'une **extension du wizard G** : l'intention est mappée (mots-clés → `WizardCategory` + choix de condition/action/son) vers le wizard **pré-rempli**, que l'utilisateur valide ensuite normalement. Zéro dépendance externe, et ça fait de l'app une app « IA-ready » dès aujourd'hui.
- **Phase B — Assistant connecté** : même champ, mais derrière un connecteur LLM optionnel (clé saisie par l'utilisateur dans les réglages, jamais bundlée). L'IA propose :
  - la création/modification d'**entités** (composants déjà typés : Collider2D, Movement2D…),
  - un **graphe KVL** (mêmes nœuds que l'éditeur visuel),
  - un **patch de scène** JSON.
- **Phase C — Boucle de confiance** : chaque proposition s'affiche comme **patch violement haloïté** (la couleur violette `#A855F7` est DÉJÀ codée dans le câblage pour « modification par un patch d'IA ») avec trois boutons : **Valider** · **Modifier** · **Annuler** — l'annulation réutilise **la pile undo/redo existante** (elle gère déjà « Annulé : … / Rétabli : … »). Plus : un **journal des interventions IA** dans le drawer console (qui a déjà l'UI d'historique).

**Garde-fous déjà présents et à exploiter** : le validateur statique KVL (un LLM ne peut pas produire un graphe invalide qui s'exécute), la sandbox d'exécution (halos, pas d'exécution directe), et l'undo global. Il ne manque que **l'affichage du diff**.

### Lot 4 — Continuum débutant → expert — **priorité moyenne**

11. **Mode « Débutant » par défaut au premier lancement** : rail d'outils réduit (Sélection, Entité, Pinceau), inspecteur KVL masqué (bouton `⌃ Expert` pour l'ouvrir), guide accessible en un clic. Bascule explicite `Débutant ⇄ Expert` dans le menu ⚙ — pas de masquage magique.
12. **États vides + progression** : scène vide → cartes « Place une entité » / « Lance le wizard G » ; les 6 étapes du guide cochables avec persistance.

---

## 6. Parcours cible (après correctifs)

```
Jour 1 — Carte d'accueil : [▶ JOUER] [🛠 CRÉER] [🎓 APPRENDRE]
   → « APPRENDRE » = guide en 6 étapes, chacune montre le raccourci
     ET laisse le doigt dessus (badge en surbrillance le temps de l'étape).
Jour 1 — Survole un bouton : il s'éclaircit, une infobulle dit
   « Dupliquer — Ctrl+D ». L'app répond.
Jour 2 — Mode Débutant : rail de 5 outils, inspecteur replié,
   bouton ✨ « Décrire ce que je veux » qui pré-remplit le wizard.
Semaine 1 — Expert : rail complet, KVL node editor, télémétrie,
   guide H en onglets consulté comme une doc, pas comme un mur.
```

---

## 7. Dette structurelle à considérer avant d'aller plus loin

- **`game.rs` fait 8 203 lignes et contient TOUTE l'UI** (header, HUD, inspecteur, wizard, guide, console, radar, dialogues). Refondre le layout (Lots 1–2) dans ce fichier aggravera la dette. **Extraire un module `ui/`** (`ui/header.rs`, `ui/hud.rs`, `ui/inspector.rs`, `ui/guide.rs`, `ui/toast.rs`) est le prérequis confortable des Lots 1 et 3.
- **Toolkit UI immédiat sans couche état** : `draw_badge` + hitboxes manuelles. Une micro-couche `Button { rect, label, key, hover_state }` avec hit-test centralisé supprimera la duplication (le même badge est dessiné + testé 3× dans les chemins JEU/Studio).
- **Le test rouge `test_multi_genre_scene_switching`** (73/74) et l'absence de CI : à réparer avant toute refonte UI, sinon impossible de distinguer une régression de la refonte.

---

## 8. Plan d'action priorisé

| # | Action | Lot | Effort | Impact débutant |
|---|---|---|---|---|
| 1 | États de survol + tooltips (`draw_badge` + souris) | 1 | Faible | 🔥🔥🔥 |
| 2 | Écran d'accueil premier lancement (3 cartes) | 1 | Moyen | 🔥🔥🔥 |
| 3 | Toasts temporaires pour `status_message` | 1 | Faible | 🔥🔥 |
| 4 | Taille fenêtre mini + garde-fous header/modale | 1 | Faible | 🔥🔥 |
| 5 | Guide H → 3 onglets + progression | 1 | Moyen | 🔥🔥🔥 |
| 6 | Graisse Regular + sentence case + corps 14 px | 2 | Faible | 🔥🔥 |
| 7 | Rail d'outils + menu ⚙ (header 13 → 6 badges) | 1 | Élevé | 🔥🔥🔥 |
| 8 | Chip profil permanent `Low-End / 60 FPS / CPU / Mem` | 2 | Faible | 🔥 |
| 9 | Intent bar → wizard pré-rempli (Phase A IA) | 3 | Moyen | 🔥🔥 |
| 10 | Diff violet + Valider/Modifier/Annuler + journal IA (Phase C) | 3 | Élevé | 🔥🔥🔥 |
| 11 | Mode Débutant/Expert + inspecteur replié | 4 | Moyen | 🔥🔥🔥 |
| 12 | Extraire `ui/` de `game.rs` (prérequis confort) | dette | Élevé | indirect |

**Ordre conseillé** : 12 (miroir : extraire d'abord ce qu'on touche) → 1 → 3 → 4 → 2 → 5 → 6 → 7 → 11 → 9 → 10 → 8.
Les items 1, 3, 4, 6 sont réalisables en quelques heures chacun et changent radicalement la *présence* perçue de l'app.

---

*Critique rédigée en revue de mi-parcours — elle ne modifie aucun code. Chaque point est référencé dans le source pour être implémenté tel quel.*
