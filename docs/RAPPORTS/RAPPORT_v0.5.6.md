# Rapport de version — v0.5.6 « Sélection 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.6` |
| Phase | 3D.2 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.5.md`](RAPPORTS/RAPPORT_v0.5.5.md) |

> Cliquer sur le château le sélectionne, l'inspecteur 3D s'ouvre, `Suppr` le retire.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | clic dans le viewport 3D → entité sélectionnée (badge `Sélection #n`), inspecteur 3D ouvert ; `Suppr` supprime |
| IA | cible `entity:<id>` — une IA n'a pas besoin de « cliquer » : elle adresse |
| Moteur | sélection et suppression passent par `select_entity` / `delete_entity` existants |

## 2. Ce qui manquait

Le picking existait déjà pour les **primitives instanciées**, pas pour les maillages
glTF importés : cliquer sur le château ne renvoyait rien. Et la sélection 3D ne
remontait pas vers l'atelier : l'inspecteur restait sur l'entité 2D précédente.

## 3. Correctifs

1. Chaque clone glTF porte `userData.entityId` ; le raycast remonte du mesh touché à ce
   porteur.
2. Le viewport remonte la sélection (`onSelectEntity`) et l'App la transmet à
   `handleSelectEntity` : l'inspecteur suit.
3. `Suppr` en 3D supprime l'entité sélectionnée et vide la sélection.

## 4. Preuves

```text
tsc → 0 erreur · npm run build → OK · lint → 14/0
cargo test --offline --workspace → 611 passed, 0 failed
export référence → 3 niveaux, 5114 Ko, moteur v0.5.6
npm run check:versions → 0.5.6 partout
```

## 5. Limites inscrites

1. **Pas de test Playwright du picking** : le mock ne transporte pas de paquet de rendu
   avec mesh glTF. La preuve est structurelle (typage, build) et moteur, pas visuelle.
2. Cliquer dans le vide ne désélectionne pas (seul `Suppr` ou la hiérarchie change la
   sélection).
3. La suppression n'a pas d'undo dédié (comme en 2D).
4. Le picking glTF intersecte les clones un par un : correct pour quelques modèles,
   coûteux pour des centaines.

## 6. Validation

- [x] Gates vertes (§4)
- [x] Sélection 3D reliée à l'inspecteur
- [x] Suppression par touche
- [x] Limites inscrites (§5)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.5.6` créé par un humain
