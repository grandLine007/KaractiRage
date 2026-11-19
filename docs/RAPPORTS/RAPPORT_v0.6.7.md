# Rapport de version — v0.6.7 « Une seule histoire, annulable dans l'ordre »

| Champ | Valeur |
|---|---|
| Version | `v0.6.7` |
| Phase | unification des piles d'annulation (défaut du contrat) |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.6.md`](RAPPORTS/RAPPORT_v0.6.6.md) |

## 1. Le défaut corrigé

Jusqu'ici, `⌘Z` choisissait **soit** la transaction IA (interceptée dans l'App), **soit**
la mutation manuelle (pile du moteur), selon un ordre que rien ne garantissait. Une
édition manuelle et une proposition IA dans la même seconde s'annulaient dans le mauvais
ordre.

Désormais il y a **une seule pile**, dans l'ordre réel des événements.

## 2. Comment

| Étape | Détail |
|---|---|
| `NativeEngine::ai_edit_apply` | sérialise le projet **avant** application, applique, sérialise **après**, pousse `StudioCommand::EditProject { label, before, after }` dans la pile du moteur |
| `game.undo` / `game.redo` | `EditProject` restaure l'instantané (entités, graphes KVL, nom, objectif) et déplace la commande dans l'autre pile |
| App | l'interception spéciale `⌘Z` de la transaction IA est **supprimée** : `⌘Z` est toujours l'undo du moteur |

Les instantanés illisibles ne font pas planter l'annulation : message d'état explicite.

## 3. Preuves

```text
cargo test --offline --workspace → 618 passed, 0 failed
  dont template_preview_then_apply_then_restore, étendu :
       appliquer « mets-le en rouge » → couleur DC2626
       engine.undo() → couleur d'origine restaurée
       engine.redo() → couleur réappliquée
tsc → 0 erreur · build OK · lint → 14/0
export référence → 3 niveaux, moteur v0.6.7
assets intacts après la suite (git diff vide)
```

## 4. Limites inscrites

1. `undoAiEdit` (transaction + session) reste disponible comme bouton de toast, mais il
   est **redondant** avec la pile : à retirer dans un nettoyage ultérieur.
2. La restauration porte sur entités, graphes KVL, nom et objectif — pas sur le manifeste
   d'assets ni les écrans UI si une proposition IA les modifiait. À couvrir avec l'op
   correspondante.
3. Pas de test d'interface : preuve moteur.

## 5. Validation

- [x] Gates vertes (§3)
- [x] Une seule pile, ordre global, prouvé par test
- [x] Raccourci unifié (`⌘Z` = moteur, toujours)
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.6.7` créé par un humain
