# Rapport de version — v0.6.10 « Le viewport 3D est exécuté par un harnais »

| Champ | Valeur |
|---|---|
| Version | `v0.6.10` |
| Phase | 3D.13 — couverture d'interface du chemin 3D |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.9.md`](RAPPORTS/RAPPORT_v0.6.9.md) |

## 1. Ce qui manquait

Le mock IPC renvoyait un **paquet de rendu vide** : le viewport 3D, le picking, le badge
de sélection et le gizmo n'étaient jamais exécutés par un test — d'où les limites
répétées « preuve moteur, pas visuelle ».

## 2. Ce qui change

| Élément | Détail |
|---|---|
| Mock | émet un **vrai paquet binaire** (1 cube, entité 1, caméra 0,5,10 → 0,0,0) — miroir exact du format `RenderPacket` |
| Hameçon | `window.__scene3dDebug` expose `gizmoVisible()` et `selection()` |
| Harness | `scripts/check-3d.mjs` : ouvre le Studio, bascule en 3D, clique, vérifie |

```text
✓ canvas three.js présent
✓ clic centre → entité 1 sélectionnée
✓ badge Sélection #1
✓ poignées du gizmo visibles
```

## 3. Le bug réel que le harnais a attrapé

La scène 3D était **recréée à chaque re-render de l'App** : les callbacks passés en props
changeaient d'identité, donc l'effet qui monte la scène se démontait/remontait sans
cesse. Conséquence : la sélection pouvait tomber **au moment exact du clic** — un état
perdu, pas un plantage.

Correctif : `useEffectEvent` (React 19) lit toujours la dernière version des callbacks
sans entrer dans les dépendances de l'effet ; la scène ne dépend plus que de `view3d`.

## 4. Incident — disque plein

Le dossier `target/` avait atteint **132,2 Gio** d'artefacts : `cargo test` échouait avec
`No space left on device` (le quota de la session, pas le disque physique). `cargo clean`
a libéré la place, la suite complète a été **reconstruite** et repassée verte, et le
binaire du Studio a été reconstruit (19 Mo).

## 5. Preuves

```text
cargo test --offline --workspace → 618 passed, 0 failed (après reconstruction complète)
cargo check --offline --workspace --tests → 0 warning
tsc → 0 erreur · build OK · lint → 14/0
audit:ui --strict → 39 captures, 0 violation, 0 dérive
check-3d.mjs → 4/4 vert
npm run check:versions → 0.6.10 partout
```

## 6. Limites inscrites

1. `check-3d.mjs` n'est **pas branché à la CI** (il exige le serveur Vite) : à intégrer
   dans le job Studio quand la CI le permettra.
2. Le harnais couvre sélection + gizmo, pas encore le glisser (la projection axe/plan
   n'est pas simulée).
3. Le paquet du mock est un cube seul : pas de maillage glTF, donc le chargeur reste
   hors harnais.

## 7. Validation

- [x] Gates vertes (§5)
- [x] Bug réel corrigé, prouvé par le harnais
- [x] Limites inscrites (§6)
- [ ] Tag `v0.6.10` créé par un humain
