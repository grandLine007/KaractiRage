# Rapport de version — v0.6.5 « Les exemples du dépôt ne sont plus écrasés »

| Champ | Valeur |
|---|---|
| Version | `v0.6.5` |
| Phase | hygiène — suite de l'incident d'auto-save |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.4.md`](RAPPORTS/RAPPORT_v0.6.4.md) |

## 1. Le problème

Trois fois, l'application en cours d'exécution a réécrit des scènes **versionnées** du
dépôt (`level_01.scene.json`, scripts KVL), faisant échouer la suite de tests (jusqu'à
22 échecs). Un projet du dépôt est un **exemple**, pas une œuvre en cours.

## 2. La décision appliquée (option 1 recommandée)

`NativeEngine::save_scene` refuse désormais d'écrire dans deux cas :

| Cas | Effet |
|---|---|
| Projet situé sous `assets/` du dépôt | sauvegarde ignorée, message « Projet d'exemple du dépôt : sauvegarde désactivée » |
| Aucun projet ouvert | sauvegarde ignorée, message « Aucun projet ouvert » |

La sauvegarde est **acceptée en apparence** (pas de dialogue d'erreur) et l'état reste
cohérent : le message est lisible dans la barre d'état.

Un auteur qui veut modifier un exemple peut le **copier** dans `~/.karactirage/projects`
— c'est là que vivent ses œuvres.

## 3. Preuves

```text
cargo test --offline --workspace → 617 passed, 0 failed
  dont test_projet_exemple_du_depot_n_est_pas_sauvegarde :
       assets/… → exemple (refus d'écriture, mtime inchangé)
       /tmp/… → projet normal · aucun projet ouvert → sauvegarde ignorée
assets intacts après la suite complète (vérifié par git diff)
cargo check --offline --workspace --tests → 0 warning
export référence → 3 niveaux, 5122 Ko, moteur v0.6.5
```

## 4. Limites inscrites

1. La garde porte sur `save_scene` ; le **chargement** d'un exemple ne réécrit rien
   (vérifié : aucun fichier sale après les tests).
2. Un auteur qui **veut** modifier un exemple ne peut plus le faire depuis ce chemin :
   il passe par une copie. C'est le comportement voulu.
3. Les commandes d'import (mesh, Tiled) écrivent encore dans le projet ouvert : si un
   exemple est ouvert, un import le modifierait. À couvrir si l'on autorise l'édition
   d'exemples.

## 5. Validation

- [x] Gates vertes (§3)
- [x] Plus d'écriture sur les assets versionnés, prouvé par test et par `git diff`
- [x] Limites inscrites (§4)
- [ ] Tag `v0.6.5` créé par un humain
