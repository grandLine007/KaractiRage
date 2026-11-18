# Rapport de version — v0.6.1 « Undo 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.6.1` |
| Phase | 3D.7 — dernière exigence du contrat (undo) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.0.md`](RAPPORTS/RAPPORT_v0.6.0.md) |

> Les éditions 3D entrent dans l'historique du Studio : `⌘Z` les annule, `⇧⌘Z` les rétablit.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | les boutons Annuler/Rétablir existants (et `Ctrl+Z` / `Ctrl+Y`) agissent sur les mutations 3D |
| IA | même historique : une proposition validée passe par les mêmes fonctions |
| Moteur | nouvelle commande `StudioCommand::Edit3D { entity_id, label, before, after }` |

L'instantané `Snapshot3D` capture `transform3d` et `material` **avant** et **après** ; undo
applique `before`, redo applique `after`, et l'opération inverse est empilée dans l'autre
pile (comportement identique aux commandes 2D).

## 2. Ce qui est annulable

| Opération | Annulable |
|---|---|
| Transformée 3D (position, rotation, échelle) | ✅ |
| Matériau 3D (couleur, rugosité, métal) | ✅ |
| Ajout/retrait de lumière | ❌ (inscrit en limite) |
| Caméra | ❌ (état de vue, pas une mutation de contenu) |

## 3. Preuves

```text
cargo test --offline --workspace → 615 passed, 0 failed
  dont test_undo_redo_restaurent_transformee_et_materiau_3d :
    transformée + matériau appliqués → 2× undo → position ET couleur d'origine restaurées
    → 2× redo → état modifié rétabli
cargo check --offline --workspace --tests → 0 warning
export référence → 3 niveaux, 5118 Ko, moteur v0.6.1
npm run check:versions → 0.6.1
```

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Granularité | un instantané par validation (blur/Entrée) | une entrée d'historique par geste, pas par frappe |
| Caméra exclue | oui | se déplacer dans la scène n'est pas modifier la scène |
| Lumières exclues | oui, pour l'instant | leur mutation n'a pas encore d'instantané ; inscrit comme manque |

## 5. Limites inscrites

1. **Lumières non annulables** : ajout et retrait ne passent pas par l'historique.
2. Aucun undo 3D **côté IA** n'est déclenché automatiquement après une proposition :
   l'historique existe, mais l'orchestrateur ne l'utilise pas encore comme filet.
3. Pas de test d'interface (mock sans paquet 3D) : la preuve est moteur.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Undo/redo restaurent l'état exact
- [x] Une seule voie moteur (instantané commun)
- [x] Limites inscrites (§5)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.6.1` créé par un humain
