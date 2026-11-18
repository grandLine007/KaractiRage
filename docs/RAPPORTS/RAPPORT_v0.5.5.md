# Rapport de version — v0.5.5 « Inspecteur 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.5` |
| Phase | 3D.1 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.4.md`](RAPPORTS/RAPPORT_v0.5.4.md) |

> Première capacité livrée selon la **règle des deux commandes** : la transformée 3D
> s'édite dans l'inspecteur **et** par l'IA, sur la même fonction de moteur.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | dans l'inspecteur, une entité portant `transform3d` affiche **X/Y/Z, RX/RY/RZ (degrés), SX/SY/SZ** au lieu de X/Y/Largeur/Hauteur |
| IA | `SetTransform3D` (déjà au catalogue) — cible `entity:<id>` |
| Moteur | **une seule** fonction : `NativeEngine::set_entity_transform3d` |

La validation est identique par les deux voies : valeurs finies, échelle strictement
positive, entité portant un `transform3d`. Un refus est un refus, quelle que soit la
main qui le demande.

## 2. Preuves

```text
cargo test --offline --workspace → 611 passed, 0 failed
  dont test_set_entity_transform3d_valide_et_refuse :
    accepte (1,2,3) / 90° / échelle 2 · refuse échelle nulle, NaN, entité inconnue
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · tsc → 0 erreur · lint → 14/0
audit:ui --strict → 39 captures, 0 violation, 0 dérive
export référence → 3 niveaux, 5114 Ko, moteur v0.5.5
npm run check:versions → 0.5.5 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Rotation | champs en **degrés** | un auteur pense en degrés, pas en quaternions |
| Champs numériques avant le gizmo | oui | testables, pilotables par IA, annulables ; le gizmo viendra après |
| Vue 2D | inchangée | une entité sans `transform3d` garde ses champs 2D |

## 4. Limites inscrites

1. Le viewport 3D **n'est pas encore rafraîchi en direct** pendant la saisie : le commit
   se fait au blur / Entrée, puis l'état est relu.
2. Pas de gizmo : pas de déplacement à la souris dans la scène.
3. Pas d'undo dédié à la transformée 3D (la sauvegarde du projet reste le point de retour).
4. La voie utilisateur n'a **pas** été validée par Playwright en conditions réelles (le
   mock ne transporte pas d'entité 3D) : seule la voie moteur est prouvée par test.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Deux voies sur la même fonction de moteur
- [x] Test de validation (acceptions et refus)
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.5.5` créé par un humain
