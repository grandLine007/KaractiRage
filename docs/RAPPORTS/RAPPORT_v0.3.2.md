# Rapport de version — v0.3.2 « Manifeste branché + assainissement CSS »

| Champ | Valeur |
|---|---|
| Version | `v0.3.2` |
| Phase | P2.1 (finalisation) + dette UI |
| Date | 11 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.1.md`](RAPPORT_v0.3.1.md) |

> Vague 3 d'orchestration : deux agents en périmètres disjoints (δ Rust, ζ CSS/TS),
> décision de baseline et alignement de version par le meneur.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| Manifeste branché sur le projet et le pré-vol (P2.1 réel) | ✅ | — |
| Correctif de la cascade CSS (`button`/`input` hors couche) | ✅ | — |
| Baseline d'audit régénérée | ✅ | 35 captures, décision du meneur, justifiée au §5 |
| Contournement inline du panneau de sauvegarde retiré | ✅ | — |
| Version alignée sur la série 0.3.x | ✅ | le code était resté à 0.2.0 alors que les rapports disaient 0.3.x |
| Import Tiled (P2.2) | ❌ | **crates.io répond 403** : aucune dépendance installable dans cette session |

## 2. Preuves

```text
cargo test --workspace        → 480 passed, 0 failed
cargo check --workspace       → 0 warning
cd apps/studio && npm run build → OK
npm run lint                  → 14 warnings, 0 erreur
npm run audit:ui -- --strict  → 35 captures, 0 violation, 0 dérive (après régénération)
npm run export:smoke          → OK (9 entités, 0 erreur, 0 réseau)
npm run check:versions        → 0.3.2 partout (Cargo, package.json, Tauri)
```

## 3. Ce qui n'est pas livré

| Élément | Raison | Échéance |
|---|---|---|
| Import Tiled / audio Kira | crates.io **403** ; aucun crate en cache | session avec réseau, ou `cargo vendor` fourni |
| Validation stricte de la version du manifeste **dans un projet** | le champ passe par le `Deserialize` dérivé | durcissement |
| Panneau de sauvegarde capturé par l'audit | le harnais ne l'ouvre pas | avec le 2ᵉ lot CSS |

## 4. Métriques

| Métrique | Valeur | Cible |
|---|---|---|
| Tests Rust | **480** | ≥ 469 |
| Warnings cargo | 0 | 0 |
| Warnings lint | 14 | ≤ 14 |
| Violations axe | 0 | 0 |
| Dérive baseline | 0 (après régénération) | 0 |
| Captures avec changement de repaint | 34 / 35 | justifié §5 |

## 5. Décision : baseline régénérée (meneur)

Le correctif CSS a rendu effectifs des utilitaires Tailwind qui étaient **silencieusement
écrasés** : 34 captures sur 35 changent. J'ai accepté la régénération après vérification
personnelle, sur ces critères :

1. **Métriques de mise en page identiques** sur 35/35 captures : header 56 px, cible
   minimale ≥ 24 px, ratio canvas, overflow 0, texte tronqué 0.
2. **0 violation axe** ; le premier passage remontait 16 violations de contraste, apparues
   précisément parce que les boutons deviennent visibles — corrigées par ζ avec des
   valeurs calculées (blanc sur `#047857` = 5,5:1, etc.).
3. **Inspection visuelle** (meneur) des écrans hub et édition-IA : les différences sont
   des surfaces de boutons/champs qui apparaissent, rien n'est masqué ni illisible.
4. Cascade vérifiée : ordre `properties → theme → base → utilities`, reset dans `base`.

## 6. Décisions prises

| Décision | Choix | Justification |
|---|---|---|
| Cause du CSS | reset déplacé dans `@layer base` (valeurs inchangées) | toute règle hors couche bat toute couche, indépendamment de la spécificité |
| Baseline | régénérée | changement visuel voulu, layout constant, 0 violation |
| Contournement inline du panneau | retiré | la cause est corrigée, le contournement deviendrait une dette |
| Version | passée à **0.3.2** partout | le code affichait encore 0.2.0 dans le pied de page |

## 7. Dettes connues

| Dette | Sévérité | Coût | Quand |
|---|---|---|---|
| `h1-h6`, `body`, `.badge-tag`, `.animate-*` restent hors couche | moyenne (piège futur) | 0,5 j + baseline | lot CSS 2 |
| `apps/studio/src/App.css` importé nulle part | faible | 5 min | lot CSS 2 |
| Panneau de sauvegarde absent du harnais d'audit | moyenne | 0,5 j | lot CSS 2 |
| Version de manifeste non validée dans un projet | faible | 2 h | P2.2 |
| AC1 non chronométré | moyenne | 0,5 j | lot UX |
| Tiled/Kira bloqués par le réseau | bloquant P2 | — | session réseau ou vendor |

## 8. Risques

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| Le lot CSS 2 change de nouveau 35 captures | élevée | revue lourde | le faire en une fois, avec preuves avant/après |
| P2 stagne sans accès réseau | moyenne | v0.4.0 glisse | travail sans dépendance en attendant (WASM, audit, UX) |

## 9. Gel et prochaine version

- **Gelé** : format `AssetsManifest` v1, pré-vol, cascade CSS, panneau de sauvegarde.
- **Prochaine version** : `v0.4.0` — P2.2 (Tiled) si le réseau revient ; sinon lot CSS 2 +
  **init WASM sans système de fichiers** (aucune dépendance requise, débloque AC3).

## 10. Validation

- [x] Gates vertes (§2)
- [x] Écarts consignés (§1, §3, §7)
- [x] Décision de baseline documentée (§5)
- [x] Périmètre gelé (§9)
- [ ] Tag `v0.3.2` créé par un humain
