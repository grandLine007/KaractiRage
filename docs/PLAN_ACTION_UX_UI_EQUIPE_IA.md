# Plan d'action UX/UI — KaractiRage Studio

> **Version** : 1.0 — 27 octobre 2026
> **Source** : audit [rapport_51_critique_ux_studio.md](../rapports/rapport_51_critique_ux_studio.md)
> **Destinataires** : équipe d'agents IA (design, frontend, accessibilité, moteur, QA)
> **Périmètre** : `apps/studio/src`, `apps/studio/src-tauri`, HUD de `crates/engine-core`
> **Statut** : backlog prêt à exécuter — décisions ouvertes §13 à trancher avant la vague 2

---

## 0. Comment utiliser ce document

1. **Un agent = un lot (WS)**. Ne pas mélanger deux lots dans une même branche.
2. Lire les fichiers concernés **avant** toute modification ; respecter la matrice de propriété §8.
3. Chaque tâche est cochable et possède un **critère d'acceptation mesurable** ; une tâche sans preuve n'est pas terminée.
4. Exécuter les vagues dans l'ordre §7 ; les dépendances sont indiquées par tâche.
5. Toute PR doit contenir : problème, changements, captures avant/après (1440×900 **et** 1024×720), résultats des scripts, risques.
6. En cas de blocage produit (choix de mode, typographie, 1024), **ne pas inventer** : ouvrir une décision §13 et s'arrêter sur la tâche concernée.
7. État d'une tâche : `[ ]` TODO, `[x]` DONE. Pour les états intermédiaires, préfixer le titre par `DOING:` ou `BLOCKED:`. Ne jamais cocher sans preuve.

**Top 10 par impact (à faire en premier)** : UX-001, UX-201, UX-301, UX-101, UX-102, UX-401, UX-403, UX-404, UX-1001, UX-501.

---

## 1. Mission et définition du succès

> **Mission** : faire passer le Studio de « cockpit impressionnant pour initié » à « atelier qui met un débutant en état de créer, tester et comprendre en moins de 10 minutes », sans perdre l'identité Dark Atelier ni le déterminisme 60 FPS.

**Succès =**

- un débutant lance une démo jouable **en ≤ 60 s** sans documentation ;
- il crée une règle KVL connectée (3 nœuds) **en ≤ 2 min** ;
- il comprend toujours **où il est**, **ce qui est sélectionné**, **ce qu'il peut faire ensuite** ;
- l'interface reste **lisible et complète à 1024×720** ;
- aucune régression des **60 FPS**, du déterminisme ou des 271 tests Rust.

---

## 2. Non-négociables (garde-fous)

- [ ] **Identité** : conserver la palette Dark Atelier, les accents violet/cyan/émeraude/ambre/rouge et la typographie industrielle. Interdiction d'importer un kit UI générique qui écrase l'identité.
- [ ] **Français** : toute copie visible est en français ; les termes techniques sont définis dans un glossaire (§9.6). Les identifiants bruts (`SetVar:score:0`) ne sont jamais l'étiquette principale.
- [ ] **Accessibilité AA** : contraste ≥ 4,5:1, cibles ≥ 24×24 px, navigation clavier complète, labels associés, `aria-*` à jour, `lang="fr"`.
- [ ] **Moteur** : aucune modification du chemin chaud sans bench ; 60 FPS et pas fixe 120 Hz préservés ; `cargo test --workspace` vert.
- [ ] **Source de vérité** : le catalogue de nœuds reste `NodeSchema` / `PluginHost` ; pas de liste de nœuds codée en dur dans le Studio.
- [ ] **Pas de mock en production** : les mocks IPC ne vivent que dans le harnais d'audit.
- [ ] **Preuve** : chaque tâche livre captures + métriques + commandes exécutées.
- [ ] **Diff minimal** : pas de refactor opportuniste hors périmètre ; un lot = une intention.

---

## 3. Baseline mesurée (26 octobre 2026)

| Mesure | Baseline | Où |
|---|---|---|
| Score Nielsen | **15/40** | rapport 51 §2 |
| Contrastes axe (serious) | **21** édition, **31** KVL, **6** hub | audit 51, à rejouer via UX-001 |
| Contraste `#64748b` | **3,94:1** (carte), **4,14:1** (panneau) | calcul WCAG |
| Textes < 12 px | **248** en KVL, **73** en édition | DOM |
| Hauteur header | **38 px** @1440, **72 px** @1024 | DOM |
| Cibles header | **16 px** de haut | DOM |
| Canvas affiché | **896×803** (ratio 1,116) @1440 ; **480×574** (0,836) @1024 | DOM |
| Échelle canvas | **0,747** et **0,4** (fractionnaire) | DOM |
| Champs sans label | **4** (X, Y, Larg., Haut) | axe critical |
| Tab | **détourné** (bascule de mode) | `App.tsx:172` |
| Écran noir sur IPC nul | **oui** (Pixel) | test de panne |
| HUD | objectif coupé, hotbar chevauchée, 802,6 vs 60 FPS | frame réelle |

> **Fiabilité** : ces chiffres viennent d'un audit en contexte unique, avec IPC mocké (rapport 51, dégradé). **UX-001 doit les reproduire** avant de les utiliser comme référence contractuelle ; un écart > 10 % doit être expliqué dans la PR.

---

## 4. Résultats à atteindre (scorecard bloquante)

> Une vague n'est validée que si **toutes** les lignes de son périmètre atteignent la cible.

| # | KPI | Baseline | Cible | Mesure |
|---|---|---|---|---|
| K1 | Score Nielsen | 15/40 | **≥ 32/40**, aucune heuristique ≤ 1 | revue design |
| K2 | axe critical/serious | 21 / 31 / 6 | **0** sur Hub, Édition, KVL, 1440 et 1024 | `npm run audit:ui` |
| K3 | Contraste texte | 3,94:1 | **≥ 4,5:1** (texte normal), ≥ 3:1 (grand) | script |
| K4 | Texte essentiel < 12 px | 248 / 73 | **0** ; corps ≥ 13 px | DOM |
| K5 | Header | 38/72 px, 16 px cibles | **56-64 px stable**, 0 élément coupé, cibles ≥ 24 px | DOM 1024-1920 |
| K6 | Canvas | ratio 1,116 / 0,836 | **1,5 ± 0,01** à toutes les tailles | DOM |
| K7 | Échelle canvas | 0,747 / 0,4 | **entière** (1×/2×) ou zoom explicite | DOM |
| K8 | HUD | 2 chevauchements | **0** chevauchement, 0 texte coupé | inspection frame |
| K9 | Clavier | Tab détourné | **Tab navigue**, focus visible 100 %, 0 changement de mode | script clavier |
| K10 | Labels/ARIA | 4 inputs sans label, 0 aria-pressed | **0** input sans label, **100 %** des modes/onglets avec état ARIA | axe + manuel |
| K11 | Hub → démo | formulaire | **≤ 60 s** pour lancer une démo | test 5 utilisateurs |
| K12 | KVL → règle connectée | impossible | **≤ 120 s** pour 3 nœuds connectés + undo | test |
| K13 | Panne IPC | écran noir | **0** écran blanc ; 100 % des erreurs ont un retry | injection de faute |
| K14 | Copie | ≥ 5 labels EN | **0** label EN non justifié ; glossaire appliqué | revue copie |
| K15 | Perf | 60 FPS | **≥ 58 FPS** mesuré, CPU ≤ +5 % | bench + télémétrie |
| K16 | Build/tests | vert | **100 %** `npm run build`, `npm run lint`, `cargo test --workspace` | CI |
| K17 | Bundle | — | **≤ +10 %** sans justification | `vite build` |
| K18 | Documentation | partielle | tokens + composants + captures + décisions à jour | revue docs |

### 4 bis. Taxonomie des interventions

| Nature | Définition | Exemples de tâches |
|---|---|---|
| **Corriger** | Défaut objectif (bug visuel, fonctionnel, a11y) | header comprimé, canvas étiré, Tab détourné, labels absents, écran noir |
| **Renforcer** | Consolider l'existant sans changer le concept | tokens, primitives, harnais d'audit, error boundaries, tests |
| **Améliorer** | Élever la qualité d'usage ou la lisibilité | filtres KVL, inspecteur de nœud, libellés, états vides, zoom |
| **Refondre** | Changer la structure ou le parcours | header deux zones, hub d'accueil, KVL éditeur, HUD anti-collision |

Chaque tâche du §6 indique sa nature ; à défaut, la nature du lot s'applique.

---

## 4 ter. Hors périmètre (ne pas s'y perdre)

- Refonte de l'art du jeu (sprites, décors, effets) : c'est un autre chantier.
- Équilibrage gameplay, level design, narration.
- Multijoueur, netcode, modding wasm.
- Refonte du moteur de rendu ou du renderer 3D.
- Internationalisation complète FR/EN (décision §13.6) : si retenue, lot séparé.
- Migration vers un autre framework UI.

---

## 5. Lots de travail (workstreams)

| Lot | Nom | Nature | Responsable type | Priorité | Vague | Effort |
|---|---|---|---|---|---|---|
| WS0 | Harnais d'audit & baseline | Renforcer | Agent QA | P0 | 0 | 2 j |
| WS1 | Design system & typographie | Refondre + Renforcer | Agent design system | P0 | 1 | 3 j |
| WS2 | Shell & header | Refondre | Agent frontend shell | P0 | 1 | 2 j |
| WS3 | Viewport & canvas | Corriger + Améliorer | Agent frontend moteur | P0 | 1-2 | 1,5 j |
| WS4 | Accessibilité & clavier | Corriger + Renforcer | Agent a11y | P0 | 1-3 | 3 j |
| WS5 | Hub & onboarding | Refondre | Agent produit/frontend | P1 | 2 | 5 j |
| WS6 | Éditeur KVL | Améliorer + Refondre | Agent frontend KVL | P1 | 2-3 | 8 j |
| WS7 | Édition, inspecteur, responsive | Améliorer | Agent frontend studio | P1 | 2-3 | 4 j |
| WS8 | HUD moteur | Refondre + Corriger | Agent moteur | P1 | 2-3 | 5 j |
| WS9 | Copie & terminologie | Améliorer | Agent contenu | P1 | 2-3 | 2 j |
| WS10 | Résilience & erreurs | Corriger + Renforcer | Agent fiabilité | P1 | 1-2 | 3 j |
| WS11 | Finitions & mouvement | Améliorer | Agent polish | P2 | 3 | 3 j |
| WS12 | QA & non-régression | Renforcer | Agent QA | P0 | 0-3 | 4 j |

**Budget indicatif total** : ≈ 45 jours-agent, parallélisable par lot. Chemin critique : WS0 → WS1 → WS2 → WS5 → WS6 → WS12.

---

## 6. Backlog détaillé

### WS0 — Harnais d'audit (prérequis absolu)

#### [UX-001] Créer `apps/studio/scripts/audit-ui.mjs`
- **Type** : Renforcer · **P0** · **Effort** : M · **Dépend de** : —
- **Constat** : aucune mesure reproductible ; l'audit 51 a été fait avec un script jetable dans `/tmp`.
- **Actions** :
  - [ ] Playwright (`playwright-core`) + `axe-core` en `devDependencies`.
  - [ ] Mock IPC isolé dans `scripts/mock-ipc.mjs` ; données réelles générées par un binaire Rust (`examples/dump_ui_data.rs`) : frame 1200×800, entités, catalogue, graphes.
  - [ ] Scénarios : Hub, Play, Édition, KVL/Recettes, KVL/Nœuds, Pixel, Vector, 1440×900 et 1024×720.
  - [ ] Sorties : `audit/report.json` (métriques, violations axe) + captures `audit/<scenario>-<width>.png`.
  - [ ] Échec code 1 si une cible K2/K3/K5/K6/K9/K10 est violée.
- **Critères** :
  - [ ] `npm run audit:ui` produit 8 captures et un JSON en < 90 s.
  - [ ] Le script mesure : header, canvas, contrastes, tailles de cibles, labels, focus, débordements.
  - [ ] Aucune dépendance runtime ajoutée.
- **Vérification** : `npm run audit:ui && node -e "..."` (lire le JSON).

#### [UX-002] Ajouter les scripts npm et la doc
- [ ] `audit:ui`, `audit:ui:update` (baseline), `test:ui` (scénarios fonctionnels).
- [ ] `docs/AUDIT_UI.md` : comment lancer, lire, mettre à jour une baseline.

#### [UX-003] Baseline visuelle versionnée
- [ ] Captures de référence dans `apps/studio/audit/baseline/` (hors build).
- [ ] Seuil de diff ≤ 0,5 % ; toute différence justifiée dans la PR.

#### [UX-004] `data-testid` stables
- [ ] `header`, `header-modes`, `canvas-viewport`, `kv-palette`, `kv-canvas`, `inspector`, `hub`, `audit-node-<id>`.
- [ ] Aucun `data-testid` utilisé pour le style.

#### [UX-005] Injection de pannes
- [ ] Mode `AUDIT_FAULTS=null|timeout|slow` dans le mock ; vérifier les états d'erreur (UX-1001/1002).
- [ ] 3 captures : moteur absent, catalogue vide, commande lente.

---

### WS1 — Design system & typographie

#### [UX-101] Charger la vraie typographie
- **Type** : Refondre · **P0** · **Fichiers** : `index.html`, `src/index.css`, `public/fonts/*`
- **Constat** : le Studio utilise `-apple-system/Segoe UI` ; Space Grotesk/Outfit/JetBrains annoncés dans le design doc ne sont **pas** chargés.
- **Actions** :
  - [ ] Auto-héberger Outfit (corps), Space Grotesk (titres), JetBrains Mono (IDs/code) en woff2.
  - [ ] `font-display: swap`, préchargement des 2 graisses utiles.
  - [ ] Échelle : 12 / 13 / 14 / 16 / 20 / 24 / 32 ; line-height 1,4 (corps), 1,2 (titres).
  - [ ] Interdire `< 12 px` pour un texte porteur de sens.
- **Critères** :
  - [ ] Aucun FOUT visible > 100 ms ; aucune requête réseau externe.
  - [ ] Les 3 familles sont utilisées aux rôles définis.
  - [ ] K4 = 0.

#### [UX-102] Tokens de couleur accessibles
- **Type** : Renforcer · **P0** · **Fichiers** : `src/index.css`
- **Constat** : `--color-faint #64748b` = 3,94:1, utilisé 81 fois.
- **Actions** :
  - [ ] `--text-muted` = `#94a3b8` (7,68:1) pour le texte ; `--text-faint` réservé aux éléments non informatifs (icônes décoratives).
  - [ ] Tokens sémantiques : `--accent-primary`, `--success`, `--warning`, `--danger`, `--info` + une règle d'usage (1 accent par rôle).
  - [ ] Vérifier les états hover/active/disabled/placeholder.
- **Critères** : K3 = 0 échec ; axe K2 = 0 ; aucune couleur codée en dur dans les nouveaux composants.

#### [UX-103] Échelle d'espacement, rayons, bordures
- [ ] Espacements 2/4/6/8/12/16/24/32 ; rayons 4/6/8/12 ; bordures 1 px uniquement pour séparer, pas pour décorer chaque carte.
- [ ] Réduire les glows/ombres à 3 niveaux d'élévation.
- [ ] Documenter dans `docs/DESIGN_TOKENS.md`.

#### [UX-104] Primitives de composants
- [ ] `Button` (primary/secondary/ghost/danger, tailles 28/32/36), `IconButton` (≥ 32), `Tabs`, `Panel`, `Field`, `Badge`, `Tooltip`.
- [ ] États : hover, focus-visible, active, disabled, loading.
- [ ] Migrer header, hub, KVL, inspecteur en priorité.
- **Critères** : 0 bouton ad hoc dans les écrans migrés ; cibles K5.

#### [UX-105] Politique de casse et de densité
- [ ] Capitales réservées aux badges ≤ 3 mots ; labels en casse de phrase.
- [ ] Letter-spacing uniquement sur capitales.
- [ ] Supprimer les libellés redondants (ex. « ACTION » sur chaque ligne si le groupe l'indique déjà).

#### [UX-106] Contrôles natifs stylés
- [ ] `input[type=range]`, `select`, scrollbars, contrôles React Flow (`Controls`, `MiniMap`) au thème Dark Atelier.
- [ ] Aucun fond blanc dans l'UI.

#### [UX-107] Mouvement et `prefers-reduced-motion`
- [ ] Tokens durée 120/160/200 ms, easing standard.
- [ ] `@media (prefers-reduced-motion: reduce)` : désactiver arêtes animées, pulses, transitions non essentielles.
- [ ] Test manuel + capture.

---

### WS2 — Shell & header

#### [UX-201] Refonte du header en deux zones
- **Type** : Refondre · **P0** · **Fichiers** : `components/HeaderBar.tsx`, `App.tsx`, `index.css`
- **Constat** : 38 px @1440 (boutons 16 px), 72 px @1024, labels collés, `Export` coupé.
- **Actions** :
  - [ ] `shrink-0` + `min-height: 56px` (cible 60) ; ne jamais comprimer sous 56.
  - [ ] Zone gauche : logo + nom. Zone centre : modes. Zone droite : statut + « … » (Hub, IA, Export, À propos).
  - [ ] Déplacer FPS/CPU/Score dans la barre de statut ou un popover « Performances ».
  - [ ] Retirer le badge « Tauri v2 • React » du header (→ À propos).
  - [ ] 1 seul niveau de lecture : marque discrète, modes lisibles, actions secondaires dans le menu.
- **Critères** :
  - [ ] K5 : hauteur 56-64 stable de 1024 à 1920, 0 élément coupé, 0 wrap.
  - [ ] Les 5 modes restent accessibles au clavier et à la souris.
  - [ ] `Export` accessible en 1 clic via le menu.

#### [UX-202] Libellés de modes courts + état ARIA
- [ ] `Jouer`, `Créer`, `Logique`, `Pixel`, `Vectoriel` (le terme complet en tooltip/description).
- [ ] `aria-pressed` (ou `role=tab` + `aria-selected`) + indicateur visuel non fondé sur la seule couleur (icône/underline).
- [ ] Raccourcis `Ctrl+1..5` / `F6` pour changer de mode.

#### [UX-203] Barre de statut utile
- [ ] Statut contextuel + version + bouton « Aide clavier ».
- [ ] Supprimer la liste de raccourcis en pied de page ; la déplacer dans un popover `?`.

#### [UX-204] Navigation de mode sans perte
- [ ] Si une édition est en cours, demander confirmation ou autosauver avant de quitter.
- [ ] Mémoriser le dernier mode et le dernier graphe.

#### [UX-205] Responsive header
- [ ] Points de rupture 1024/1280/1440 ; à 1024, menu « … » et modes en icônes + tooltip si nécessaire.
- [ ] Test 1920×1080 et 1024×720.

#### [UX-206] Tests de non-régression header
- [ ] Capture 4 largeurs ; vérifier 0 overflow horizontal, 0 texte coupé, cibles ≥ 24.

---

### WS3 — Viewport & canvas

#### [UX-301] Ratio et letterbox
- **Type** : Corriger · **P0** · **Fichiers** : `components/GameViewport.tsx`, `index.css`
- **Constat** : 896×803 @1440 (ratio 1,116) et 480×574 @1024 (0,836) au lieu de 1,5.
- **Actions** :
  - [ ] Conteneur `display:grid; place-items:center; min-height:0`.
  - [ ] Canvas : `aspect-ratio: 1200 / 800; max-width:100%; max-height:100%; width:auto; height:auto`.
  - [ ] Letterbox sombre autour ; aucune déformation.
  - [ ] `ResizeObserver` pour recalculer.
- **Critères** : K6 = 1,5 ± 0,01 à 1024/1280/1440/1920 ; aucune ligne/colonne étirée.

#### [UX-302] Échelle entière et zoom
- [ ] Modes `Fit`, `1×`, `2×`, `3×` ; affichage du facteur.
- [ ] Préférence persistée (`localStorage`).
- [ ] En `Fit`, utiliser un ratio entier si possible, sinon letterbox.
- **Critères** : K7 ; aucune échelle fractionnaire silencieuse.

#### [UX-303] Netteté pixel art
- [ ] `image-rendering: pixelated` uniquement en échelle entière ; `auto` en Fit fractionnaire (ou downscale propre).
- [ ] Vérifier `devicePixelRatio` (1 et 2).
- [ ] Test visuel : grille, texte bitmap, pièces rondes.

#### [UX-304] Focus et sémantique du canvas
- [ ] `role="img"`/`aria-label="Viewport du jeu — 1200×800"` ou `application` justifié.
- [ ] `focus-visible` ring 2 px accent + offset ; ne pas supprimer l'outline sans remplacement.
- [ ] `tabIndex=0` conservé uniquement si des raccourcis y sont actifs.

#### [UX-305] Overlays et HUD dans le viewport
- [ ] Vérifier que les boutons flottants (pause, radar, grille, 3D) font ≥ 32×32 et ne chevauchent pas le HUD.
- [ ] Masquer la télémétrie par défaut (UX-803).

#### [UX-306] Matrice de tailles
- [ ] Captures 1024/1280/1440/1920 ; documenter le comportement attendu.

---

### WS4 — Accessibilité & clavier

#### [UX-401] Rendre Tab au navigateur
- **Type** : Corriger · **P0** · **Fichiers** : `App.tsx:172`
- **Constat** : `preventDefault()` sur Tab + bascule de mode ; focus bloqué.
- **Actions** :
  - [ ] Supprimer le détournement ; utiliser `F6` ou `Ctrl+Tab` pour changer de mode.
  - [ ] Mettre à jour le footer/l'aide et les tests.
- **Critères** : K9 ; 100 % des contrôles atteignables au clavier dans l'ordre visuel.

#### [UX-402] Focus visible global
- [ ] Token `:focus-visible` (2 px accent, offset 2) sur boutons, liens, inputs, canvas, cartes cliquables.
- [ ] Test : parcours clavier complet, capture de 5 focus.

#### [UX-403] Labels et noms accessibles
- [ ] `htmlFor`/`id` sur X, Y, Larg., Haut ; `aria-label` sur les icônes seules.
- [ ] `aria-describedby` pour les aides/erreurs.
- **Critères** : K10 ; axe `label` = 0.

#### [UX-404] Sémantique des composants
- [ ] Modes : `aria-pressed` ; KVL/Hub : `role=tablist/tab/tabpanel` + `aria-selected`.
- [ ] Modales : `role="dialog"`, `aria-modal`, focus trap, restitution du focus, `Échap` ferme.
- [ ] `lang="fr"`, `<title>KaractiRage Studio</title>`.

#### [UX-405] Cibles tactiles/souris
- [ ] Minimum 24×24 (WCAG 2.5.8), cible 32×32 sur desktop.
- [ ] Header, onglets KVL, palette, contrôles React Flow, boutons d'icônes.

#### [UX-406] Contrastes et états
- [ ] axe 0 serious ; vérifier hover/focus/disabled/placeholder.
- [ ] Ne pas coder une information par la couleur seule (badge + icône/texte).

#### [UX-407] Mouvement réduit
- [ ] Test `prefers-reduced-motion` ; arêtes KVL statiques ; aucun pulse.

#### [UX-408] Aide clavier accessible
- [ ] `?` ouvre un dialogue « Raccourcis » avec tableau, fermable, focus trap.
- [ ] Lien visible dans le header/statut.

#### [UX-409] Lecteur d'écran
- [ ] Landmarks `header/main/aside/footer`, titres hiérarchisés, canvas décrit.
- [ ] Test VoiceOver sur Hub, Édition, KVL.

#### [UX-410] Audit final
- [ ] `npm run audit:ui` + parcours clavier + VoiceOver ; rapport signé.

---

### WS5 — Hub & onboarding

#### [UX-501] Refonte du hub en écran d'accueil
- **Type** : Refondre · **P1** · **Fichiers** : `ProjectHubModal.tsx`, `App.tsx`
- **Constat** : modale 1024×532, onglets `1.1/1.2/1.3`, 3 CTA concurrents, formulaire avant résultat.
- **Actions** :
  - [ ] Plein écran (ou modale 90 %) avec 3 choix visuels : **Jouer une démo** / **Créer mon premier jeu** / **Comprendre le Studio**.
  - [ ] Démo : 5 cartes d'archétypes avec aperçu, 1 clic.
  - [ ] Création : 3 étapes (nom → genre → modèle) avec aperçu ; dossier de destination en « Avancé ».
  - [ ] Comprendre : visite de 3 écrans (Créer / Logique / Tester) + raccourcis.
- **Critères** : K11 ; un utilisateur lance une démo en ≤ 3 clics et ≤ 60 s.

#### [UX-502] Supprimer la numérotation de spécification
- [ ] `1.1 NOUVEAU PROJET` → `Nouveau projet`, etc.
- [ ] Un seul CTA primaire par écran.

#### [UX-503] États vides et récents
- [ ] « Aucun projet » avec explication + CTA.
- [ ] Récents avec vignette, genre, date ; action « Ouvrir » claire.
- [ ] « Ne plus afficher » / reprise du dernier choix.

#### [UX-504] Onboarding progressif
- [ ] Après création : 3 étapes contextuelles (placer un objet, ajouter une règle, tester), avec progression persistée.
- [ ] Bouton « Passer » et réouverture via `?`.

#### [UX-505] Test utilisateur
- [ ] 5 personnes non techniques ; mesurer temps jusqu'à démo + compréhension des 3 modes.
- [ ] Seuil : 4/5 réussissent en ≤ 60 s.

---

### WS6 — Éditeur KVL

#### [UX-601] Palette filtrable
- **Type** : Améliorer · **P1** · **Fichiers** : `KvlGraphEditor.tsx`
- **Constat** : 75 nœuds à plat, sans filtre.
- **Actions** :
  - [ ] Filtres `Tout / Actions / Conditions / Événements` avec compteurs.
  - [ ] Catégories repliables, mémorisées.
  - [ ] Tri : suggérés d'abord, puis alphabétique.
- **Critères** : 0 nœud non trouvé en ≤ 10 s pour 5 requêtes types.

#### [UX-602] Cartes de nœuds lisibles
- [ ] Libellé français d'abord, ID technique en second (9-11 px autorisé pour l'ID seulement).
- [ ] Icône de type + couleur + tooltip ; description courte.
- [ ] Badge de paramètre requis.

#### [UX-603] Inspecteur de nœud
- [ ] Clic sur un nœud → panneau : paramètres issus du `NodeSchema`, édition, validation.
- [ ] `Appliquer` / `Annuler` ; feedback.
- [ ] Prévisualisation du type concret (`SetVar:score:0`).

#### [UX-604] Câblage et édition du graphe
- [ ] Tirer d'un pin de sortie vers un pin d'entrée ; supprimer une arête ; supprimer un nœud.
- [ ] Undo/redo (Ctrl+Z/Y) sur toutes les opérations, y compris l'insertion.
- [ ] Sélection multiple (P2).

#### [UX-605] « Insérer & câbler »
- [ ] Assistant : choisir un nœud → choisir le nœud source/prise → insérer + connecter.
- [ ] Sinon, insertion simple + zoom/sélection sur le nouveau nœud.

#### [UX-606] Gestion des graphes
- [ ] Nouveau graphe, renommer, dupliquer, supprimer (confirmation).
- [ ] Indicateur « enregistré / non enregistré ».
- [ ] État vide « Créer votre première règle » + recette conseillée.

#### [UX-607] Lisibilité du canevas
- [ ] Flèches de direction, légende des couleurs (Événement/Condition/Action), fit view.
- [ ] Contrôles de zoom stylés, 0 fond blanc.
- [ ] Grille discrète, contraste suffisant.

#### [UX-608] Recettes guidées
- [ ] Accordéon : une seule recette ouverte ; résultat attendu affiché.
- [ ] Bouton désactivé + explication si moteur non prêt.
- [ ] Après génération : focus sur le graphe + « Tester ».

#### [UX-609] États de chargement/erreur
- [ ] Skeleton pendant le catalogue ; message + retry si échec.
- [ ] Jamais « Aucun graphe » quand le moteur boote.

#### [UX-610] Tests scénarisés
- [ ] Créer 3 nœuds, les connecter, undo, redo, save, recharger → 0 perte.
- [ ] Test au clavier complet.

---

### WS7 — Édition, inspecteur, responsive

#### [UX-701] Panneaux redimensionnables/repliables
- [ ] Largeurs persistées ; poignées accessibles ; bouton replier.
- [ ] Canvas prioritaire : largeur min 50 % à 1280.

#### [UX-702] Inspecteur responsive
- [ ] ≥ 1280 : colonne ; < 1280 : drawer superposé.
- [ ] Champs pleine largeur, unités, pas de valeur coupée.
- [ ] Labels associés (UX-403).

#### [UX-703] Barre d'outils
- [ ] « Ajouter un objet » toujours visible ; groupes d'outils ; overflow menu.
- [ ] Tooltips avec raccourci ; icônes ≥ 16 px, cibles ≥ 32.

#### [UX-704] Conseils/onboarding contextuel
- [ ] Déplacer en widget flottant ou panneau haut-droit.
- [ ] 3 étapes max, progression, dismiss, lien vers l'action.
- [ ] Visible à 1024 sans scroll.

#### [UX-705] Hiérarchie
- [ ] Recherche, filtre par tag, compteur ; sélection claire.
- [ ] État vide avec CTA.

#### [UX-706] Responsive global
- [ ] 1024×720 : 0 élément coupé, 0 overflow horizontal, header stable, canvas ratio.
- [ ] 1280×800 et 1920×1080 validés.

---

### WS8 — HUD moteur

> **Attention** : `game.rs` est gros et chaud. Isoler le layout HUD dans un module dédié, avec tests, sans allocation par frame.

#### [UX-801] Layout HUD anti-collision
- **Type** : Refondre · **P1** · **Fichiers** : `crates/engine-core/src/game.rs` (rendu HUD), éventuellement `renderer.rs`
- **Constat** : objectif coupé par le radar ; hotbar chevauchée.
- **Actions** :
  - [ ] Mesurer le texte avant dessin ; retour à la ligne / ellipse.
  - [ ] Zones réservées : objectif (haut-gauche), radar (haut-droite), télémétrie (masquée), hotbar (bas centre).
  - [ ] Anti-collision : si une zone dépasse, réduire la priorité ou déplacer.
- **Critères** : K8 = 0 chevauchement à 1200×800 et 1024×720.

#### [UX-802] Hotbar lisible
- [ ] Libellés sous les slots, troncature + tooltip.
- [ ] Slot sélectionné non ambigu ; quantités lisibles.
- [ ] Test 5 objets.

#### [UX-803] Télémétrie masquée par défaut
- [ ] Mode « Performances » opt-in (F1) ; badge compact FPS.
- [ ] En mode débutant : aucun panneau technique.

#### [UX-804] Source FPS unique
- [ ] Une seule valeur, libellée « FPS mesuré » ; la cible 60 est un texte séparé.
- [ ] Pas de 802,6 face à 60 dans le même écran.

#### [UX-805] Zones sûres et échelles
- [ ] Tester 1200×800, 1024×720, 1440×900 ; HUD indépendant du scale.
- [ ] Titre/objectif lisibles à la taille affichée.

#### [UX-806] Contraste et typographie HUD
- [ ] Taille min 10-12 px équivalent ; contraste ≥ 4,5:1.
- [ ] Pas de texte sur fond chargé sans plaque.

---

### WS9 — Copie & terminologie

#### [UX-901] Glossaire
- [ ] `docs/GLOSSAIRE_UI.md` : KVL, ECS, nœud, graphe, pin, recette, archétype, RenderPacket, profil.
- [ ] Première occurrence : terme + définition courte.

#### [UX-902] Traduire les labels
- [ ] « JOUER (PLAY) » → « Jouer » ; « Export » → « Exporter » ; « Hub » → « Accueil » ; « Intention IA » → « Assistant IA ».
- [ ] « GPU WEBGL2 », « FPS », « CPU » restent techniques mais dans un panneau Avancé.

#### [UX-903] Casse et concision
- [ ] Casse de phrase ; phrases ≤ 12 mots pour les labels.
- [ ] Supprimer les points d'exclamation et les majuscules criardes.

#### [UX-904] Messages d'erreur actionnables
- [ ] Format : ce qui s'est passé + pourquoi + action.
- [ ] Ex. « Le moteur n'est pas prêt (boot 128 ms). Réessayer. »

#### [UX-905] Revue de copie finale
- [ ] 0 label EN non justifié ; captures ; validation produit.

---

### WS10 — Résilience & erreurs

#### [UX-1001] Error boundaries
- **Type** : Renforcer · **P0** · **Fichiers** : `App.tsx`, `main.tsx`, composants critiques
- **Constat** : une réponse IPC nulle blanchit tout l'écran (Pixel).
- **Actions** :
  - [ ] Boundary racine + boundaries par panneau (Hub, Édition, KVL, Pixel, Vector).
  - [ ] Fallback : message, « Recharger le panneau », « Revenir à l'accueil », détails repliés.
- **Critères** : K13 ; 0 écran blanc en injection de faute.

#### [UX-1002] Client IPC robuste
- [ ] Wrapper typé : timeout, erreurs normalisées, retry, toast.
- [ ] Aucun composant ne suppose `null`/`undefined` pour un tableau.
- [ ] Journalisation avec contexte (commande, args, durée).

#### [UX-1003] États de chargement
- [ ] Skeletons Hub/KVL/Pixel/Vector ; messages « Préparation du moteur… ».
- [ ] Timeout 60 s avec action.

#### [UX-1004] Mode dégradé
- [ ] Moteur indisponible : l'UI reste navigable ; actions désactivées avec explication.
- [ ] « Réessayer » fonctionnel.

#### [UX-1005] Tests de panne
- [ ] `AUDIT_FAULTS=null|timeout|slow` ; 3 captures ; 0 crash.

---

### WS11 — Finitions

#### [UX-1101] Palette pixel-art
- [ ] Noms sémantiques + pastille + hex au survol.
- [ ] Groupes (peau, métal, nature…) ; recherche.

#### [UX-1102] Vectoriel
- [ ] Sliders stylés, inputs numériques avec unités, valeurs par défaut claires.
- [ ] Aperçu vide avec instruction.

#### [UX-1103] Iconographie
- [ ] Tailles 14/16/18, stroke 1,75 ; cohérence lucide.
- [ ] Aucune icône seule sans `aria-label`/tooltip.

#### [UX-1104] Micro-interactions
- [ ] Hover 120 ms, focus 120 ms, transitions 160 ms ; pas d'animation gratuite.
- [ ] Reduced motion respecté.

#### [UX-1105] À propos
- [ ] Versions, licences, crédits ; déplacer « Tauri v2 • React » ici.

---

### WS12 — QA & non-régression

#### [UX-1201] Matrice de tests
- [ ] 1024×720, 1280×800, 1440×900, 1920×1080 ; zoom 100/125/150 %.
- [ ] Hub, Play, Édition, KVL/Recettes, KVL/Nœuds, Pixel, Vector.
- [ ] Souris + clavier + reduced motion.

#### [UX-1202] Audit automatique
- [ ] `npm run audit:ui` en pré-commit/CI ; 0 critical/serious.
- [ ] Rapport JSON archivé par PR.

#### [UX-1203] Tests fonctionnels
- [ ] Scénario 1 : premier lancement → démo ≤ 60 s.
- [ ] Scénario 2 : créer un projet → placer un objet → règle KVL 3 nœuds → tester.
- [ ] Scénario 3 : panne IPC → fallback → retry.
- [ ] Scénario 4 : 1024×720 complet.

#### [UX-1204] Performance
- [ ] 60 FPS ± 5 % ; CPU frame ≤ baseline + 5 %.
- [ ] `cargo test --workspace` ; bench `engine_bench`.
- [ ] Bundle ≤ +10 %.

#### [UX-1205] Documentation finale
- [ ] `docs/DESIGN_TOKENS.md`, `docs/AUDIT_UI.md`, `docs/GLOSSAIRE_UI.md`.
- [ ] Captures de référence à jour ; ADR si décision structurante.

#### [UX-1206] Definition of Done global
- [ ] K1-K18 atteints sur le périmètre de la vague.
- [ ] Aucun `TODO` orphelin, aucun `console.log` de debug.
- [ ] Rapport de vague (avant/après, métriques, captures, risques).

---

## 7. Ordre d'exécution (vagues)

### Vague 0 — Mesurer (1-2 jours)
`UX-001` → `UX-002` → `UX-003` → `UX-004` → `UX-005`
**Sortie** : baseline JSON + 8 captures + décisions §13 tranchées.

### Vague 1 — Débloquer (3-5 jours)
`UX-101` → `UX-102` → `UX-103` → `UX-104` → `UX-201` → `UX-202` → `UX-203` → `UX-301` → `UX-302` → `UX-304` → `UX-401` → `UX-402` → `UX-403` → `UX-404` → `UX-1001` → `UX-1002`
**Sortie** : header stable, canvas 3:2, contraste AA, Tab rendu, 0 écran blanc.

### Vague 2 — Rendre utilisable (2 semaines)
`UX-204` → `UX-205` → `UX-303` → `UX-305` → `UX-501` → `UX-502` → `UX-503` → `UX-504` → `UX-601` → `UX-602` → `UX-603` → `UX-604` → `UX-605` → `UX-606` → `UX-701` → `UX-702` → `UX-703` → `UX-704` → `UX-801` → `UX-802` → `UX-803` → `UX-804` → `UX-901` → `UX-902` → `UX-903` → `UX-1003` → `UX-1004`
**Sortie** : parcours débutant complet, KVL connecté, HUD propre.

### Vague 3 — Finir (1 semaine)
`UX-105` → `UX-106` → `UX-107` → `UX-206` → `UX-306` → `UX-405` → `UX-406` → `UX-407` → `UX-408` → `UX-409` → `UX-410` → `UX-505` → `UX-607` → `UX-608` → `UX-609` → `UX-610` → `UX-705` → `UX-706` → `UX-805` → `UX-806` → `UX-904` → `UX-905` → `UX-1005` → `UX-1101` → `UX-1102` → `UX-1103` → `UX-1104` → `UX-1105` → `UX-1201` → `UX-1202` → `UX-1203` → `UX-1204` → `UX-1205` → `UX-1206`

---

## 8. Matrice de propriété des fichiers

| Zone | Fichiers | Agent propriétaire | Consommateurs |
|---|---|---|---|
| Tokens / global | `src/index.css`, `index.html`, `public/fonts/*`, futurs `src/ui/*` | WS1 | Tous |
| Shell | `App.tsx`, `components/HeaderBar.tsx`, footer | WS2 | WS5 |
| Viewport | `components/GameViewport.tsx`, `render3d/Scene3DView.tsx` | WS3 | WS8 |
| A11y transverse | tous les composants (attributs uniquement) | WS4 | Tous — coordonner |
| Hub | `ProjectHubModal.tsx`, `services/projectBridge.ts` | WS5 | WS2 |
| KVL | `KvlGraphEditor.tsx`, `services/engineBridge.ts` (KVL), `src-tauri/src/lib.rs` (commandes KVL) | WS6 | WS4 |
| Édition | `EntityInspector.tsx`, `StudioToolbar.tsx`, `EntityHierarchy.tsx`, `SuggestionPanel.tsx` | WS7 | WS4 |
| HUD moteur | `crates/engine-core/src/game.rs` (HUD), `renderer.rs`, tests | WS8 | Interdit aux autres sans coordination |
| Copie | tous les composants (texte) | WS9 | Tous — geler avant WS9 |
| Résilience | `services/*`, `App.tsx`, boundaries | WS10 | WS2/5/6 |
| Audit | `apps/studio/scripts/*`, `audit/*`, `package.json` (scripts) | WS0/12 | Tous |

**Règles de conflit** :
- Un seul agent écrit dans un fichier à un instant donné ; annoncer la prise dans le canal d'équipe.
- WS1 (tokens) et WS9 (copie) passent **avant** les autres sur leurs fichiers, puis gèlent.
- WS4 (a11y) intervient après les refontes de structure, pas avant, pour éviter de retravailler.
- WS8 (moteur) travaille sur une branche isolée et ne touche pas au Studio.

---

## 9. Protocole d'équipe IA

### 9.0 Rôles et passations

| Rôle | Mission | Compétences clés | Sortie attendue |
|---|---|---|---|
| **Orchestrateur** | Séquence, arbitre les conflits, valide les gates | Lecture du plan, git, revue | Vague validée + journal §14 |
| **Agent design system** | Tokens, polices, primitives, mouvement | Tailwind v4, CSS, typographie | `src/ui/*`, `index.css`, docs tokens |
| **Agent shell** | Header, statut, navigation de modes | React, layout flex/grid, responsive | Header stable 1024-1920 |
| **Agent viewport** | Canvas, ratio, zoom, HUD côté Studio | React, canvas, ResizeObserver | Canvas 3:2, zoom, focus |
| **Agent a11y** | Clavier, ARIA, contrastes, lecteurs d'écran | WCAG, axe, VoiceOver | 0 violation, parcours clavier |
| **Agent KVL** | Palette, inspecteur, câblage, undo | React Flow, Tauri IPC, KVL | Règle connectée ≤ 120 s |
| **Agent moteur** | HUD, anti-collision, FPS | Rust, renderer, bench | HUD 0 chevauchement, 60 FPS |
| **Agent contenu** | Copie FR, glossaire, erreurs | Rédaction, terminologie | 0 label EN, messages actionnables |
| **Agent QA** | Harnais, baseline, tests, pannes | Playwright, axe, CI | `audit:ui`, gates, rapport |

**Passation** : l'agent sortant laisse (1) la branche à jour, (2) les captures, (3) le journal §14, (4) les décisions ouvertes. L'orchestrateur fusionne ; aucun agent ne fusionne son propre lot sans revue.

### 9.1 Branches et commits
- Branche : `ux/<lot>-<id>` (ex. `ux/ws2-201-header`).
- Commit : `fix(studio/header): …`, `feat(studio/kvl): …`, `a11y(studio): …`, `docs(ux): …`.
- Un commit = une intention ; pas de mélange formatage + comportement.

### 9.2 Cycle par tâche
1. Lire la tâche + les fichiers + les captures de référence.
2. Écrire/ajuster le test ou le script d'audit **avant** si possible.
3. Implémenter au plus petit diff.
4. Exécuter : `npm run build`, `npm run lint`, `npm run audit:ui`, tests Rust si touchés.
5. Capturer 1440 et 1024 (avant/après).
6. Remplir le rapport de PR (template §9.4).
7. Marquer la checklist ; mettre à jour le journal §14.

### 9.3 Interdictions
- Modifier un fichier hors propriété sans accord.
- Introduire une dépendance UI lourde sans ADR.
- Remplacer la copie produit sans validation (WS9).
- Utiliser `any` pour masquer une erreur IPC.
- Laisser un mock dans le code de production.
- Toucher au chemin chaud du moteur sans bench.

### 9.4 Template de PR
```md
## Problème
## Changement
## Preuves
- Captures : 1440 / 1024 (avant/après)
- Audit : `npm run audit:ui` (K2/K3/K5/K6/K9/K10)
- Tests : build, lint, cargo (si applicable)
## Métriques
| KPI | avant | après |
## Risques / rollback
## Décisions prises (ou bloquées)
```

### 9.5 Definition of Done (par tâche)
- [ ] Critères d'acceptation cochés et **mesurés**.
- [ ] 0 régression sur les KPI du périmètre.
- [ ] Captures avant/après 1440 + 1024.
- [ ] `npm run build` et `npm run lint` verts.
- [ ] `npm run audit:ui` sans nouvelle violation.
- [ ] Code sans `console.log`, `TODO` orphelin, `any` non justifié.
- [ ] Documentation/glossaire mis à jour si la copie change.
- [ ] Journal §14 complété.

### 9.6 Glossaire minimal à appliquer
| Terme | Formulation UI |
|---|---|
| KVL | « Logique visuelle (KVL) » à la première occurrence, puis « Logique » |
| Nœud | « Bloc » ou « Nœud » (choisir et tenir) |
| Graphe | « Règle » ou « Graphe » (choisir et tenir) |
| Pin | « Prise » |
| Recette | « Modèle de mécanique » |
| Archétype | « Jeu de référence » |
| RenderPacket | interne, jamais affiché |
| ECS | « Entités et composants » |

### 9.7 Commandes de référence

```bash
# Studio
cd apps/studio
npm run dev              # Vite + Tauri via `cargo tauri dev` si nécessaire
npm run build            # tsc -b && vite build
npm run lint             # oxlint
npm run audit:ui         # créé par UX-001 — captures + axe + métriques
npm run audit:ui:update  # met à jour la baseline visuelle

# Moteur (si HUD/commandes touchés)
cargo test --workspace
cargo bench -p engine-core --bench engine_bench

# Vérification manuelle minimale
# 1. Hub → démo en ≤ 60 s
# 2. Créer → placer un objet → KVL 3 nœuds connectés → Tester
# 3. 1024×720 : aucun élément coupé
# 4. Panne IPC simulée : fallback + retry
```

---

## 10. Portes d'acceptation (gates)

| Gate | Contenu | Bloquant |
|---|---|---|
| G0 Build | `npm run build`, `npm run lint`, `cargo test --workspace` | oui |
| G1 A11y | axe 0 critical/serious ; labels 100 % ; Tab complet ; focus visible ; `lang=fr` | oui |
| G2 Layout | 1024/1280/1440/1920 ; header 56-64 ; canvas 1,5 ; 0 overflow | oui |
| G3 UX | Démo ≤ 60 s ; règle KVL ≤ 120 s ; panne IPC → retry | oui |
| G4 Engine | 60 FPS ; déterminisme ; 0 allocation nouvelle par frame | oui |
| G5 Docs | tokens, glossaire, audit, captures, journal | oui |

---

## 11. Matrice de tests

| Axe | Valeurs |
|---|---|
| Largeurs | 1024×720, 1280×800, 1440×900, 1920×1080 |
| Zoom | 100 %, 125 %, 150 % |
| États moteur | ready, booting, failed, IPC null/timeout |
| Données | 0 projet, 1 projet récent, 12 entités, 0 nœud, 75 nœuds |
| Entrées | souris, clavier seul, reduced motion, VoiceOver |
| Écrans | Hub, Play, Édition, KVL/Recettes, KVL/Nœuds, Pixel, Vector |

---

## 12. Risques et mitigations

| Risque | Impact | Mitigation |
|---|---|---|
| Header refondu casse la navigation | Élevé | Baseline visuelle + feature flag + tests clavier |
| Échelle entière réduit le viewport | Moyen | Mode Fit par défaut, zoom explicite |
| Câblage KVL nécessite de nouvelles commandes Tauri | Élevé | Coordonner WS6/WS8 ; fallback lecture seule |
| `game.rs` régresse les 60 FPS | Élevé | Module HUD isolé + bench + tests |
| Polices ajoutent du poids | Faible | woff2 subset, 2 graisses, preload |
| Refonte hub retarde la v1 | Moyen | Décision §13 ; hub minimal 3 choix d'abord |
| Dérive de périmètre | Élevé | Un lot = une intention ; revue PR |

---

## 13. Décisions ouvertes (à trancher avant la vague 2)

1. **1024×720 est-il contractuel ?** Si non, passer la fenêtre minimale à 1280×800 et simplifier le responsive.
2. **Faut-il consolider les modes ?** Ex. Vectorisation en « Avancé », Pixel en atelier secondaire ; garder 3 modes principaux.
3. **Typographie** : charger Outfit/Space Grotesk (identité) ou assumer la police système (performance) ?
4. **KVL v1** : éditeur complet (câblage/undo/params) ou générateur de recettes + insertion ? Le plan suppose l'éditeur complet ; à confirmer.
5. **HUD** : télémétrie masquée par défaut ? (recommandé oui)
6. **i18n** : FR seul ou FR/EN ? Si EN, budget d'extraction des chaînes.

---

## 14. Journal de validation

| Date | Vague | Tâches | KPI avant → après | Preuves | Agent |
|---|---|---|---|---|---|
| 2026-10-26 | Audit | — | Nielsen 15/40 ; axe 21/31/6 ; header 38/72 ; canvas 1,116/0,836 | rapport 51, captures | Audit |
| | | | | | |

---

## 15. Annexes

- Audit source : [rapport_51_critique_ux_studio.md](../rapports/rapport_51_critique_ux_studio.md)
- Captures : `rapports/assets/critique-*.png`
- Guide plugins : [PLUGINS.md](PLUGINS.md)
- ADR design : [ADR-0002](adr/ADR-0002-systeme-de-plugins.md)

---

## 16. Définition de fin de plan

Le plan est terminé quand **tous** les points suivants sont vrais :

- [ ] **K1-K18** atteints et mesurés sur le périmètre complet (Hub, Play, Édition, KVL, Pixel, Vector).
- [ ] **G0-G5** passés ; `npm run audit:ui` et `cargo test --workspace` verts.
- [ ] Un débutant lance une démo en **≤ 60 s** et crée une règle KVL connectée en **≤ 2 min** (test 5 personnes).
- [ ] **0** écran blanc en injection de panne ; toutes les erreurs ont un retry.
- [ ] **0** violation axe critical/serious ; parcours clavier complet ; `lang="fr"`.
- [ ] Header stable **56-64 px** de 1024 à 1920 ; canvas **1,5** ; HUD **0** chevauchement.
- [ ] **0** texte essentiel < 12 px ; contrastes ≥ 4,5:1.
- [ ] Aucune régression 60 FPS / déterminisme ; bench et tests moteur verts.
- [ ] Tokens, glossaire, audit et captures à jour ; baseline 2026-10-26 archivée.
- [ ] Journal §14 clôturé avec les métriques avant/après et les décisions prises.
