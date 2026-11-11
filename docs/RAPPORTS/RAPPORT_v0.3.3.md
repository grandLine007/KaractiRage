# Rapport de version — v0.3.3 « Le jeu exporté joue »

| Champ | Valeur |
|---|---|
| Version | `v0.3.3` |
| Phase | clôture de **P1** (AC3) — dette technique |
| Date | 11 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.2.md`](RAPPORT_v0.3.2.md) |

> Vague 4 (wasm) et vague 5 (runtime embarqué + entrées) menées en orchestration :
> trois agents (η init wasm, λ runtime embarqué, μ entrées), intégration et preuves
> par le meneur.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| **AC3** — HTML unique, 0 réseau, 0 erreur console | ✅ | — |
| **AC3** — « jouable » | ✅ | le joueur se déplace au clavier dans le fichier exporté |
| Runtime WASM qui démarre sans système de fichiers | ✅ | — |
| Entrées clavier + souris (déplacement) dans la version web | ✅ | `mousemove` seulement, pas de clics |
| Export déterministe (même projet → même fichier) | ✅ | bug d'ordre de `HashMap` corrigé |
| Parité de l'export alternatif (`to_standalone_html`) | ✅ | — |
| Import Tiled / audio Kira | ❌ | **crates.io répond 403** (aucun crate installable) |

## 2. Preuves — AC3 fermé

```text
cargo run -p engine-core --bin export_game -- assets/projects/jeu_reference.karacti /tmp/ref_final.html
→ export OK — 9 entités, 2 graphes KVL, 4862 Ko, runtime embarqué, empreinte b6d60f74b1cddc11
  taille 4 980 000 o · deux exports → sha256 06f4f223a24082fb… identiques

node scripts/export-smoke.mjs /tmp/ref_final.html --expect-runtime --expect-input
→ OK — 0 erreur, 0 requête réseau, runtime=running,
  960000 pixels non noirs (65 couleurs), entrée=5778 pixels modifiés
```

**Jouabilité démontrée, pas supposée** : le smoke envoie `ArrowRight`, capture l'image
avant/après et exige un changement. Contrôle de l'agent : 708 pixels changent sans
entrée (animations de fond) contre **5 778** avec la flèche ; suivi du personnage :
centroïde x ≈ 152 → 503 (y inchangé), soit **+351 px**.

```text
cargo test --workspace      → 489 passed, 0 failed, 1 ignored
cargo check --tests         → 0 warning
cargo check wasm32          → 0 warning
npm run lint                → 14 warnings, 0 erreur
npm run audit:ui -- --strict→ 39 captures, 0 violation, 0 dérive
```

## 3. Ce qui n'est pas livré

| Élément | Raison | Échéance |
|---|---|---|
| Tiled (P2.2), Kira (P2.3) | crates.io 403, hors ligne | session réseau ou vendor |
| Clics souris transmis au jeu | périmètre μ | lot gameplay |
| Tests Firefox/Safari, CSP stricte | Chrome headless seulement | lot QA web |
| Mesure AC1 (< 2 min) | pas de test chronométré | lot UX |

## 4. Métriques

| Métrique | Valeur | Cible |
|---|---|---|
| Tests Rust | **489** | ≥ 480 |
| Warnings cargo | 0 | 0 |
| Warnings lint | 14 | ≤ 14 |
| Violations axe | 0 | 0 |
| Dérive baseline | 0 | 0 |
| Export jouable | 4,86 Mo, 1 fichier | < 10 Mo |
| Export données (sans runtime) | 26 Ko | — |
| Pixels modifiés par une flèche | 5 778 | > 2 000 |

## 5. Décisions prises

| Décision | Choix | Justification |
|---|---|---|
| Chargement du runtime | **inliné en base64** dans le HTML | `import('./pkg/...')` est interdit en `file://` (CORS) |
| Repli | **supprimé** | un repli silencieux produisait la carte « prétendu jouable » |
| États du template | `running` / `absent` / `error:<msg>` | un export de données ne doit pas mentir |
| Horloge wasm | compteur monotone dérivé des étapes KVL | `SystemTime::now()` panique sous wasm et bloquait toute interaction |
| Déterminisme | JSON embarqué à clés triées | deux exports du même projet différaient |
| Touches maintenues | relâchées sur `blur` / `visibilitychange` | éviter une touche « collée » après alt-tab |
| Empreinte | hash du **contenu de jeu**, pas du runtime | deux artefacts wasm différents gardent la même empreinte de projet — documenté |

## 6. Écarts et dérogations

| Écart | Cause | Impact | Rattrapage |
|---|---|---|---|
| Seuil d'entrée du smoke inclut les animations | mesure globale des pixels | un jeu très animé rendrait le seuil moins discriminant | baseline « sans entrée » |
| Pas de clics souris | périmètre | pas de jeu à la souris | lot gameplay |
| `dist/pkg` non versionné, à reconstruire | binaire de 3,5 Mo | un export sans `dist/pkg` retombe en données | `scripts/build_wasm.sh` documenté |
| Écarts AC1/AC2 antérieurs | cf. v0.3.0/v0.3.1 | AC2 désormais ✅, AC1 toujours non mesuré | lot UX |

## 7. Dettes connues

| Dette | Sévérité | Coût | Quand |
|---|---|---|---|
| AC1 non chronométré | moyenne | 0,5 j | lot UX |
| Passe typographique `h1-h6` (refusée avec preuve en v0.3.2) | faible | 0,5 j + baseline | lot UI |
| `Échap` ferme la barre IA avant le panneau de sauvegarde | faible | 30 min | lot UX |
| Pas de relâchement des touches sur perte de focus **du canvas seul** (window couvert) | faible | — | — |
| Tiled/Kira bloqués réseau | **bloquant P2** | — | réseau ou vendor |

## 8. Risques

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Rebuild wasm oublié après changement moteur → export périmé | moyenne | jeu figé au runtime précédent | le pré-vol signale la taille ; ajouter un hash attendu |
| Bundle de 4,86 Mo refusé par certains hébergeurs | faible | distribution | mode « données seules » conservé |

## 9. Gel et prochaine version

- **Gelé** : format d'export HTML (runtime inliné), protocole `__karacti_runtime`,
  `KEY_MAP` du template, format de replay, `AssetsManifest` v1.
- **Prochaine version** : `v0.4.0` — P2 (Tiled/Kira) **dès qu'un accès réseau ou un
  vendor est disponible** ; sinon lot UX (mesure AC1, typographie, ordre des Échap).

## 10. Validation

- [x] Gates vertes (§2)
- [x] AC3 prouvé par un test automatisé, pas par une déclaration
- [x] Écarts consignés (§3, §6)
- [x] Dette inscrite (§7)
- [ ] Tag `v0.3.3` créé par un humain
