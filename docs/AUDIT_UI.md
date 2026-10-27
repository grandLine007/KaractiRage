# Harnais d'audit UX — `npm run audit:ui`

> WS0 / UX-001 · Mesure reproductible de l'interface KaractiRage Studio (React +
> Tauri) : header, canvas, contrastes et accessibilité, via Playwright (Chrome
> headless) + axe-core, avec un **mock IPC** qui remplace le backend natif.

## Lancer l'audit

```bash
cd apps/studio
npm run audit:ui            # build + capture de tous les écrans (mode mesure)
npm run audit:ui -- --strict  # idem, échoue (code 1) si violations critical/serious
```

Le script effectue :

1. `npm run build` (production) ;
2. `vite preview` sur `http://127.0.0.1:4173` ;
3. ouverture de **Chrome headless** (`playwright-core`, `channel: 'chrome'`) ;
4. injection du **mock IPC** (`scripts/mock-ipc.js`) via `addInitScript` : il
   définit `window.__TAURI_INTERNALS__.invoke`, basculant le frontend en mode
   Tauri sans backend natif (frame 1200×800, 12 entités, catalogue de nœuds,
   4 projets de démo) ;
5. capture de **7 écrans** × **2 tailles** (1440×900, 1024×720) = **14 captures** ;
6. pour chaque écran : mesure du **header** (hauteur, plus petite cible), du
   **canvas** (dimensions/ratio) et des **violations axe-core** (WCAG A/AA).

## Lire les résultats

- `audit/report.json` — métriques + violations axe par scénario ;
- `audit/<scenario>-<width>.png` — captures d'écran.

Résumé imprimé en console, ex. :

```text
[audit] hub @1440x900 header=38px cible_min=16px canvas_ratio=1.75 axe_crit/serious=18
```

## Cibles suivies (scorecard §4 du plan)

| KPI | Mesuré par | Cible |
|---|---|---|
| K2 | axe-core (`critical`/`serious`) | 0 |
| K5 | `header_height`, `header_target_min` | 56-64 px, cibles ≥ 24 px |
| K6 | `canvas.ratio` | 1,5 ± 0,01 |

Les autres KPIs (K3 contrastes détaillés, K9 clavier, K10 labels) sont couverts
partiellement par axe-core ; les scripts dédiés viendront dans les lots WS4/WS12.

## Mettre à jour la baseline (UX-003, à venir)

Le dossier `apps/studio/audit/` est **ignoré** par git (sorties régénérées).
Une baseline versionnée sera ajoutée dans `apps/studio/audit/baseline/` (avec
négation dans `.gitignore`) une fois la reproduction validée à ≤ 0,5 % d'écart.

## Dépendances

- `playwright-core` + `axe-core` (devDependencies uniquement, aucune dépendance runtime).
- Nécessite un **Chrome** système (aucun navigateur téléchargé par Playwright).
