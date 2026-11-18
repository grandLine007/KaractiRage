# Rapport de version — v0.5.9 « Matériaux 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.9` |
| Phase | 3D.5 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.8.md`](RAPPORTS/RAPPORT_v0.5.8.md) |

> Couleur, rugosité et métal par entité, dans l'inspecteur et via `SetMaterial`.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | bloc **Matériau 3D** dans l'inspecteur : couleur (sélecteur), rugosité, métal |
| IA | `SetMaterial { entity, material }` (préréglages `metal`, `glow`, défaut) |
| Moteur | `set_entity_material3d(id, color, roughness, metalness)` |

Les deux voies produisent le **même état** `MaterialRef::Solid { color, roughness, metalness }`.
Les préréglages IA sont des raccourcis vers les mêmes champs ; les bornes (0–1) sont
vérifiées une seule fois, côté moteur.

## 2. Preuves

```text
cargo test --offline --workspace → 613 passed, 0 failed
  dont test_set_entity_material3d_applique_et_refuse :
       (FF8800, 0.25, 0.8) appliqué · rugosité 2.0 refusée · métal −0.1 refusé ·
       entité inconnue refusée
tsc → 0 erreur · build OK · lint → 14/0
export référence → 3 niveaux, 5114 Ko, moteur v0.5.9
npm run check:versions → 0.5.9 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Couleur | sélecteur natif + masque 24 bits | un auteur choisit une couleur, pas un entier |
| Bornes | 0–1 strictes | une rugosité de 2 n'a pas de sens ; refus explicite |
| Préréglages IA | conservés | ne pas casser le catalogue existant |

## 4. Limites inscrites

1. Pas de **texture** ni de matériau nommé dans l'inspecteur : couleur unie seulement.
2. Le matériau n'est pas appliqué aux sous-meshes individuellement.
3. Pas de test d'interface (mock sans paquet 3D).
4. Pas d'undo dédié.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Deux voies sur le même état
- [x] Test d'acceptation et de refus
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.5.9` créé par un humain
