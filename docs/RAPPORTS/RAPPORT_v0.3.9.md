# Rapport de version — v0.3.9 « L'écran de menu est présentable »

| Champ | Valeur |
|---|---|
| Version | `v0.3.9` |
| Phase | P1 (qualité d'interface du jeu exporté) |
| Date | 13 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.8.md`](RAPPORT_v0.3.8.md) |

> Dette §5.1 du rapport v0.3.8 soldée : la carte de menu est calculée sur son contenu,
> hiérarchisée, contrastée et sans débordement. Le HUD est prouvé **pixel-identique**.

## 1. Avant / après (mesuré)

| | Avant | Après |
|---|---|---|
| Carte | 446 × 85 px | **540 × 246 px**, largeur calculée |
| Marges internes | ~5 px (texte au bord) | **44 px** minimum |
| Titre | petit, terne | 30 pt, interlettrage, ombre |
| Entrées | 3 lignes de texte nu | pastilles de touche + action + chevron |
| Décor derrière | masqué | **visible** (voile blendé) |
| Contraste titre | — | **18,83:1** |
| Contraste secondaire | — | **7,32:1** |
| Contraste touche | — | **6,59:1** |

## 2. Preuves

```text
cargo test --workspace        → 521 passed, 0 failed
cargo check --workspace       → 0 warning
npm run check:versions        → 0.3.9 partout

cargo test -p engine-core ui_render::tests → 10 passed
  overlay_menu_card_layout_and_contrast : carte=676x250, gaps 264/264 et 277/277,
    contrastes 18.83 / 7.32 / 6.59, tous les pixels de texte à ≥ 4 px de la bordure
  render_hud_panel_is_pixel_identical : empreintes FNV-1a inchangées
    (0x8aa782c827578a56 en 320×200, 0x6eda033db52cf0d6 en 1200×800)

export-smoke --expect-runtime --expect-input --expect-levels 3 --expect-screens 4
→ OK — runtime=running, niveaux=3/3, écrans=4/4, entrée=6092 pixels modifiés
deux exports → même sha256
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Texte des touches | `#c084fc` au lieu de l'accent `#a855f7` | `#a855f7` sur `#1e1338` = **4,40:1**, sous le seuil AA ; `#c084fc` = 6,59:1 |
| Voile | blendé à 205/255, calibré sur le fond dominant | le décor reste perceptible **et** les deux tests legacy qui comptent le pixel `#060911` restent verts |
| Largeur de carte | calculée sur le contenu, bornée `[460, 760]` puis au viewport | un titre long ou 6 entrées ne débordent pas |
| HUD | chemin de rendu inchangé | preuve d'empreinte pixel-identique |

## 4. Limites inscrites

1. `UiScreen` n'a pas de champ sous-titre : le rôle est tenu par l'action après la touche.
2. Le titre 30 pt est rasterisé au **premier** affichage (cache préchauffé 10-20 pt) :
   coût ponctuel, hors périmètre.
3. Le voile est calibré sur `#0d1525` ; changer le fond dominant du niveau demandera de
   réajuster `MENU_VEIL` et les tests legacy.
4. Les chemins `bar`/`icon` de l'overlay sont implémentés mais non exercés par les écrans
   du projet de référence.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Dette v0.3.8 §5.1 soldée
- [x] Contraste mesuré, pas estimé
- [x] HUD prouvé non régressé (empreinte pixel)
- [ ] Tag `v0.3.9` créé par un humain
