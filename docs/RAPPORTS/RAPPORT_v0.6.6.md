# Rapport de version — v0.6.6 « Les exemples sont intouchables par l'application »

| Champ | Valeur |
|---|---|
| Version | `v0.6.6` |
| Phase | hygiène — extension de la garde des exemples |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.5.md`](RAPPORTS/RAPPORT_v0.6.5.md) |

## 1. Ce qui change

La garde « exemple du dépôt » couvre désormais **les écritures**, pas seulement l'auto-save :

| Commande | Refus sur un exemple ouvert |
|---|---|
| `save_scene` | ✅ (v0.6.5) |
| `engine_place_mesh_asset` | ✅ |
| `engine_import_mesh_asset` | ✅ |
| Moteur `place_mesh_asset` | ✅ — la garde passe **avant** la résolution d'asset, pour que l'erreur soit celle du dépôt, pas « asset introuvable » |

L'erreur est uniforme : « Projet d'exemple du dépôt : copiez-le d'abord pour le modifier. »

## 2. Preuves

```text
cargo test --offline --workspace → 618 passed, 0 failed
  dont test_placement_refuse_un_exemple_du_depot :
       projet de référence ouvert → place_mesh_asset → refus « exemple »
       (le test vérifie que l'erreur vient de la garde, pas de la résolution d'asset)
assets intacts après la suite (git diff vide)
export référence → 3 niveaux, moteur v0.6.6
```

## 3. Limites inscrites

1. `engine_import_tiled` n'a pas reçu la garde : l'import Tiled crée ses fichiers à côté
   de la carte fournie, pas nécessairement dans le projet ouvert. À vérifier quand
   l'import Tiled touchera le projet courant.
2. L'auteur ne peut pas éditer un exemple **par aucun chemin** : comportement voulu.

## 4. Validation

- [x] Gates vertes (§2)
- [x] Garde testée au niveau moteur (l'ordre des erreurs est vérifié)
- [x] Limites inscrites (§3)
- [ ] Tag `v0.6.6` créé par un humain
