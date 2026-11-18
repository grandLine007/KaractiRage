# Rapport de version — v0.6.2 « Undo des lumières »

| Champ | Valeur |
|---|---|
| Version | `v0.6.2` |
| Phase | 3D.8 — complément d'undo |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.1.md`](RAPPORTS/RAPPORT_v0.6.1.md) |

> Ajouter ou retirer une lumière s'annule et se rétablit, comme une transformée.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | `⌘Z` / `⇧⌘Z` agissent sur les ajouts et retraits de lumières |
| IA | `AddLight` passe par la même fonction, donc le même historique |
| Moteur | `StudioCommand::EditLights { label, before, after }` (instantané de la liste) |

## 2. Correction importante

L'implémentation initiale de l'undo 3D **échangeait** `before` et `after` en empilant la
commande inverse. C'était faux : la commande doit être déplacée telle quelle d'une pile à
l'autre, sinon `redo` rejoue l'état d'avant. Le test des lumières l'a révélé (il vérifiait
la taille de la liste après `redo`), et les deux commandes (`Edit3D`, `EditLights`) ont
été corrigées.

## 3. Preuves

```text
cargo test --offline --workspace → 616 passed, 0 failed
  dont test_undo_lumieres_restaure_la_liste :
    ajout → 1 · undo → liste initiale · redo → 1 · retrait → 0 · undo → 1
  dont test_undo_redo_restaurent_transformee_et_materiau_3d (toujours vert)
cargo check --offline --workspace --tests → 0 warning
export référence → 3 niveaux, 5122 Ko, moteur v0.6.2
```

## 4. Limites inscrites

1. La **caméra** reste hors historique (état de vue, décision assumée).
2. Les lumières sont réversibles **en bloc** : l'historique stocke la liste entière, pas
   une lumière isolée. Correct pour quelques lumières, coûteux pour des centaines.
3. Pas d'undo automatique après une proposition IA (l'historique existe, l'orchestrateur
   ne s'en sert pas encore).
4. Pas de test d'interface.

## 5. Validation

- [x] Gates vertes (§3)
- [x] Bug de pile corrigé et prouvé par test
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.6.2` créé par un humain
