# Rapport de version — v0.6.3 « Recette 3D et vérification de l'undo IA »

| Champ | Valeur |
|---|---|
| Version | `v0.6.3` |
| Phase | 3D.9 — vérification et recette |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.2.md`](RAPPORTS/RAPPORT_v0.6.2.md) |

> Deux vérifications : l'annulation IA existe bien et elle est testée ; la recette 3D
> est écrite pour être exécutée en trois minutes.

## 1. Annulation des propositions IA — état vérifié

| Élément | Emplacement | Preuve |
|---|---|---|
| Instantané avant application | `KaractiProject` sérialisé | `ai_edit_restore` |
| Commande Tauri | `engine_ai_edit_restore` | `lib.rs:503` |
| Raccourci | `⌘Z` quand une transaction IA est en cours | `App.tsx:564` |
| Test | `ai_edit_tests::restore_undoes_an_applied_edit` | **vert** |

Il y a donc **deux filets distincts** : `⌘Z` annule la dernière proposition IA tant que
sa transaction existe ; le bouton Annuler/Rétablir du Studio annule les mutations 3D
(transformée, matériau, lumières).

**Limite réelle** : les deux ne partagent pas la même pile. Un `⌘Z` après une édition
manuelle annule l'édition manuelle ; après une proposition IA, il annule la proposition.
L'ordre exact n'est pas garanti si les deux se mélangent dans la même seconde. Inscrit.

## 2. Recette 3D

[`RECETTE_3D.md`](RECETTE_3D.md) : six gestes, de l'import au matériau, avec ce qui doit
être vrai (modes 2D absents, palette réduite, annulation) et quoi noter en cas d'échec.

## 3. Preuves

```text
cargo test --offline --workspace → 616 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · lint → 14/0
export référence → 3 niveaux, moteur v0.6.3
npm run check:versions → 0.6.3 partout
```

## 4. Limites inscrites

1. Piles d'annulation IA et Studio **séparées** (décrit §1).
2. La recette n'a pas encore été exécutée par le propriétaire.
3. Gizmo et orbite toujours absents.

## 5. Validation

- [x] Gates vertes (§3)
- [x] Annulation IA vérifiée (code + test)
- [x] Recette 3D écrite
- [ ] Recette exécutée par un humain
- [ ] Tag `v0.6.3` créé par un humain
