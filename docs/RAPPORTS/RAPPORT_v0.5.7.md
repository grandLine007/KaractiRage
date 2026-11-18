# Rapport de version — v0.5.7 « Caméra 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.7` |
| Phase | 3D.3 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.6.md`](RAPPORTS/RAPPORT_v0.5.6.md) |

> La caméra se règle dans le viewport et se pilote par IA, sur le même état.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | bouton **Caméra** dans la barre 3D → panneau position (X/Y/Z) et cible (X/Y/Z), plus **Recadrer** |
| IA | `SetCamera { position }` (vise l'origine) |
| Moteur | `NativeEngine::set_camera3d(position, target)` — un seul état `game.camera3d` |

La commande Tauri `engine_get_camera3d` permet au panneau d'afficher l'état courant
plutôt que des valeurs inventées.

## 2. Preuves

```text
cargo test --offline --workspace → 611 passed, 0 failed
  dont test_set_camera3d_applique_et_refuse : applique (1,2,3)→(0,1,0), refuse caméra=cible et l'infini
tsc → 0 erreur · build OK · lint → 14/0
export référence → 3 niveaux, 5114 Ko, moteur v0.5.7
npm run check:versions → 0.5.7 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Orbite souris | **pas encore** | le paquet écrase la caméra à chaque frame ; il faut d'abord une caméra éditable par l'état (fait ici) |
| Refus caméra = cible | erreur explicite | une caméra sans direction n'affiche rien ; mieux vaut refuser |
| Recadrer | position (0, 2.5, 8), cible (0, 1, 0) | même cadrage que le placement d'un modèle |

## 4. Limites inscrites

1. **Pas d'orbite à la souris** : le panneau remplace l'orbite pour l'instant.
2. Le panneau n'a pas de test d'interface (mock sans paquet 3D) : preuve moteur + build.
3. `SetCamera` IA ne règle que la position (cible à l'origine) — asymétrie assumée,
   documentée, à corriger si l'IA doit viser ailleurs.
4. Aucun undo dédié.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Deux voies sur le même état caméra
- [x] Test d'acceptation et de refus
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.5.7` créé par un humain
