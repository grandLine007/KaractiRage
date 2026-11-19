# Rapport de version — v0.6.8 « Panoramique de la caméra 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.6.8` |
| Phase | 3D.11 — manipulation de la caméra (complément) |
| Date | 19 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.6.7.md`](RAPPORTS/RAPPORT_v0.6.7.md) |

## 1. Ce qui change

| Geste | Effet |
|---|---|
| **Bouton central + glisser** | panoramique : caméra et cible glissent ensemble sur le plan de vue |
| Bouton droit + glisser | orbite (v0.6.4) |
| Molette | zoom (v0.6.4) |
| Clic gauche + glisser sur un objet | déplacement au sol |

Le pan suit la même règle que l'orbite : il écrit dans le moteur par la commande
`engine_set_camera3d` (throttlé), la cible se déplace avec la caméra — la scène glisse,
le point visé reste cohérent.

## 2. Preuves

```text
tsc → 0 erreur · build OK · lint → 14/0
cargo test --offline --workspace → 618 passed, 0 failed
export référence → 3 niveaux, moteur v0.6.8
```

## 3. Limites inscrites

1. Pas de test automatisé du geste (le mock ne transporte pas de paquet 3D).
2. Le pan est **non borné** : on peut glisser la caméra à l'infini. À borner si les
   scènes deviennent grandes.
3. Le bouton central est celui par défaut ; pas de variante `maj + glisser` pour les
   souris sans bouton central.

## 4. Validation

- [x] Gates vertes (§2)
- [x] Pan sur la même commande que le panneau et l'IA
- [x] Limites inscrites (§3)
- [ ] Gestes exécutés par un humain
- [ ] Tag `v0.6.8` créé par un humain
