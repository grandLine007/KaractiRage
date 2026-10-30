# Harnais d'audit UX — `npm run audit:ui`

> WS0 / UX-001 · Mesure reproductible de l'interface KaractiRage Studio (React +
> Tauri) : header, canvas, contrastes et accessibilité, via Playwright (Chrome
> headless) + axe-core, avec un **mock IPC** qui remplace le backend natif.

## Lancer l'audit

```bash
cd apps/studio
npm run audit:ui               # build + capture de tous les écrans (mode mesure)
npm run audit:ui -- --strict   # idem, échoue (code 1) si violations critical/serious
npm run audit:ui:update        # régénère la baseline visuelle (audit/baseline/)
```

Le script effectue :

1. `npm run build` (production) ;
2. `vite preview` sur `http://127.0.0.1:4173` ;
3. ouverture de **Chrome headless** (`playwright-core`, `channel: 'chrome'`) ;
4. injection du **mock IPC** (`scripts/mock-ipc.js`) via `addInitScript` : il
   définit `window.__TAURI_INTERNALS__.invoke`, basculant le frontend en mode
   Tauri sans backend natif (frame 1200×800, 12 entités, catalogue de nœuds,
   4 projets de démo) ;
5. capture de **7 écrans** × **2 tailles** (1440×900, 1024×720) = **14 captures**
   + **3 scénarios de pannes** (1440×900) = **17 captures** ;
6. pour chaque écran : mesure du **header** (hauteur, plus petite cible), du
   **canvas** (dimensions/ratio) et des **violations axe-core** (WCAG A/AA).

## Lire les résultats

- `audit/report.json` — métriques + violations axe par scénario ;
- `audit/<scenario>-<width>.png` — captures d'écran.

Chaque violation axe expose `targets` (sélecteurs CSS, ≤ 10 nœuds), `html`
(extrait tronqué) et `failureSummary` : le rapport est **directement
actionnable** — on localise et corrige sans rejouer l'audit. Le terminal
imprime aussi une ligne `↳ <règle> ×<n> → <sélecteurs>` par scénario.

Résumé imprimé en console, ex. :

```text
[audit] hub @1440x900 header=56px cible_min=32px canvas_ratio=1.5 axe_crit/serious=0
[audit]   ↳ target-size ×4 → button[title="Échelle 1x"] ‖ …        # sélecteurs des violations
```

## Cibles suivies (scorecard §4 du plan)

| KPI | Mesuré par | Cible |
|---|---|---|
| K2 | axe-core (`critical`/`serious`) | 0 |
| K5 | `header_height`, `header_target_min` | 56-64 px, cibles ≥ 24 px |
| K6 | `canvas.ratio` | 1,5 ± 0,01 |

Les autres KPIs (K3 contrastes détaillés, K9 clavier, K10 labels) sont couverts
partiellement par axe-core ; les scripts dédiés viendront dans les lots WS4/WS12.

## Mettre à jour la baseline (UX-003)

- `npm run audit:ui:update` (ou `--update-baseline`) copie les captures courantes
  dans `apps/studio/audit/baseline/` (versionnées par git, `.gitignore` négocié).
- En mode normal, chaque capture est comparée pixel à pixel à la baseline ; l'écart
  est reporté dans `report.json` (`baseline_diff_pct`) et un avertissement est émis
  au-delà de **0,5 %**.
- Toute différence justifiée doit être documentée dans la PR.
- **Règle** : la baseline se régénère **uniquement en fin de lot** (refonte visuelle
  ou correctif a11y terminé), après revue des captures — jamais en cours de lot, pour
  ne pas masquer une régression.

## Scénarios de pannes (UX-005)

Le mock lit `?fault=<valeur>` :

| Valeur | Effet | Écran attendu |
|---|---|---|
| `null` | `engine_status` → `failed`, `engine_canvas_width`/`frame` rejetés | panneau « Erreur du moteur » + Réessayer (pas d'écran noir) |
| `timeout` | `engine_canvas_width` ne résout jamais | spinner « Préparation du moteur… » |
| `slow` | `engine_canvas_width` résout après 2,5 s | spinner puis frame |
| `empty` | `engine_plugin_catalog` → catalogue vide | onglet Nœuds vide |

3 captures sont produites : `fault-null`, `fault-timeout`, `fault-empty`.

## data-testid stables (UX-004)

| `data-testid` | Composant |
|---|---|
| `header` / `header-modes` | `HeaderBar` (barre + sélecteur de mode) |
| `canvas-viewport` | canvas 2D (`GameViewport`) |
| `inspector` | `EntityInspector` |
| `hub` | `ProjectHubModal` |
| `kv-palette` / `kv-canvas` | `KvlGraphEditor` (palette + canevas React Flow) |

`audit-node-<id>` (nœuds KVL) est différé au lot WS6 (éditeur KVL) — le graphe de
démo est vide, aucune cible à instrumenter aujourd'hui.

## Dépendances

- `playwright-core`, `axe-core`, `pngjs` (devDependencies uniquement, aucune dépendance runtime).
- Nécessite un **Chrome** système (aucun navigateur téléchargé par Playwright).
