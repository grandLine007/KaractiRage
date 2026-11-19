# Rapport de version — v0.6.9 « Gizmo de translation »

| Champ | Valeur |
|---|---|
| Version | `v0.6.9` |
| Phase | 3D.12 — gizmo |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.8.md`](RAPPORTS/RAPPORT_v0.6.8.md) |

## 1. Ce qui change

| Geste | Effet |
|---|---|
| Sélectionner un objet | **trois poignées d'axe** apparaissent (X rouge, Y vert, Z bleu), taille liée à la distance caméra |
| Glisser une poignée | l'objet glisse **le long de cet axe** dans le plan caméra/axe |
| Relâcher | position finale envoyée au moteur |

Pendant le glisser, les envois sont throttlés (~20/s) vers `engine_set_entity_transform3d`
— la même commande que l'inspecteur et que l'opération IA `SetTransform3D`. Rotation et
échelle de l'entité sont préservées (relues de la sélection).

## 2. Comment le gizmo décide

- **Position** : matrice de l'instance sélectionnée, relue du paquet à chaque frame.
- **Prise** : raycast sur les trois flèches ; remontée du mesh touché à sa poignée
  (`userData.axis`).
- **Projection** : plan engendré par l'axe et la direction de vue, figé à la prise ;
  le déplacement est la projection du rayon sur l'axe — pas de saut au clic.
- **Priorités** : poignée > pan > orbite > déplacement au sol.

## 3. Preuves

```text
tsc → 0 erreur · build OK · lint → 14/0
cargo test --offline --workspace → 618 passed, 0 failed
export référence → 3 niveaux, moteur v0.6.9
assets intacts après la suite
```

## 4. Limites inscrites — plus lourdes que d'habitude

1. **Aucun test automatisé du geste** : le mock ne transporte pas de paquet 3D, donc ni
   les poignées, ni la prise, ni la projection ne sont exécutées par un harnais. La
   géométrie est vérifiée par la construction, pas par l'exécution.
2. **Translation seulement** : pas de rotation ni d'échelle au gizmo.
3. L'inspecteur **ne suit pas en direct** pendant le glisser (rafraîchi à la sélection
   suivante) : les champs peuvent afficher l'ancienne position jusqu'au prochain clic.
4. Les poignées traversent les objets (pas de test de profondeur) : visibles toujours,
   parfois à travers un mur.

## 5. Validation

- [x] Gates vertes (§3)
- [x] Gizmo sur la même commande moteur que l'inspecteur et l'IA
- [x] Limites inscrites (§4)
- [ ] Gestes exécutés par un humain — **indispensable pour ce lot**
- [ ] Tag `v0.6.9` créé par un humain
