# Rapport de version — v0.3.8 « Le jeu exporté a une interface »

| Champ | Valeur |
|---|---|
| Version | `v0.3.8` |
| Phase | P1 (export) |
| Date | 13 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.7.md`](RAPPORT_v0.3.7.md) |

> Dette §5.1 du rapport v0.3.7 soldée : les `ui_screens` voyagent dans le paquet et le
> menu est réellement dessiné à l'import. Un défaut visuel majeur découvert au passage.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| `ui_screens` embarqués dans le paquet | ✅ | 4 écrans (Hud, Menu, Pause, Victory) |
| Menu **dessiné** après import | ✅ | titre + entrées, sans chrome du Studio |
| Indicateur « NIVEAU X/N » | ✅ | en haut à droite, sous le radar, seulement si multi-niveaux |
| Smoke `--expect-screens` | ✅ | `écrans=4/4` |
| Qualité visuelle du menu moteur | ❌ | §5 : fonctionnel mais pauvre |

## 2. Le défaut visuel trouvé en route

`import_package` et les reconstructions perdaient `editor.embedded_mode`. Conséquence :
le **chrome du Studio** (barres, panneaux) réapparaissait autour du jeu autonome dans
le navigateur. Corrigé à l'import et dans `start_game`/`next_level`/`reload`.
Le Studio natif (`embedded_mode = false`) n'est pas affecté.

## 3. Preuves

```text
cargo test --workspace        → 516 passed, 0 failed
cargo check --workspace       → 0 warning
npm run check:versions        → 0.3.8 partout

export_game → 3 niveau(x), 24 entités, 4 graphes KVL, 4983 Ko
export-smoke --expect-runtime --expect-input --expect-levels 3 --expect-screens 4
→ OK — runtime=running, niveaux=3/3, écrans=4/4, entrée=6092 pixels modifiés
deux exports → même sha256
```

Tests ajoutés : paquet embarque les 4 écrans dans le même ordre que le projet ; import →
écrans non vides + `screen == Menu` + conservés après `start_game`/`next_level` ;
**rendu après import dessine le menu** (canvas 1200×800, > 8 couleurs, voile > 100 000 px) ;
`level_counter()` (`None` en mono-scène, `1/3`, `2/3`, `3/3`, bornage) ; paquet sans
`ui_screens` importable ; HUD multi-niveaux sans panique.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Transport des écrans | recopie à l'import (pas de champ mémoire) | `levels`/`project_id`/`ui_screens` sont déjà recopiés aux reconstructions |
| Import legacy | écrans installés **seulement** si le paquet en transporte | ne pas effacer les écrans de l'instance cible |
| Indicateur de niveau | widget séparé du HUD | ne touche ni la pilule procédurale ni le HUD data-driven |

## 5. Limites inscrites

1. **Le menu moteur est pauvre visuellement** : texte petit, terne, troisième ligne à
   la limite du cadre. Voir capture dans le rapport de version suivant. À polir.
2. `window.__karacti_level` n'est pas rafraîchi après un changement de niveau (aucun
   accesseur de niveau exposé par le runtime) ; le smoke ne vérifie donc le niveau
   qu'après import.
3. SVG et assets 3D ne voyagent toujours pas.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Dette v0.3.7 §5.1 soldée
- [x] Défaut visuel découvert et corrigé (chrome du Studio)
- [x] Limites inscrites (§5)
- [ ] Tag `v0.3.8` créé par un humain
