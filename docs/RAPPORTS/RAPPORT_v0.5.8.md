# Rapport de version — v0.5.8 « Lumières 3D »

| Champ | Valeur |
|---|---|
| Version | `v0.5.8` |
| Phase | 3D.4 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.7.md`](RAPPORTS/RAPPORT_v0.5.7.md) |

> Les lumières du projet se listent, s'ajoutent et se retirent dans le viewport, avec
> l'opération IA `AddLight` sur le même état.

## 1. Ce qui change

| Voie | Forme |
|---|---|
| Utilisateur | bouton **Lumières** → liste (`type #couleur ×intensité`), **Ajouter une lumière**, ✕ pour retirer |
| IA | `AddLight { kind }` (valeurs par défaut) |
| Moteur | `add_light` / `remove_light` / `lights_json` |

Validation identique : type parmi `point`, `directional`, `ambient` ; coordonnées finies ;
intensité dans **0–100** ; index existant pour le retrait.

## 2. Preuves

```text
cargo test --offline --workspace → 612 passed, 0 failed
  dont test_lumieres_ajout_retrait_et_refus : ajout (point, 1.5), retrait,
       refus type inconnu « spot », intensité 500, index 7 inexistant
tsc → 0 erreur · build OK · lint → 14/0
export référence → 3 niveaux, 5114 Ko, moteur v0.5.8
npm run check:versions → 0.5.8 partout
```

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Intensité bornée | 0–100 | une valeur absurde ne doit pas blanchir la scène |
| Couleur | masquée sur 24 bits | cohérent avec le format du paquet |
| Liste vide | éclairage par défaut du moteur | ne pas casser un projet existant |

## 4. Limites inscrites

1. **Pas d'édition** d'une lumière existante (ni position, ni couleur, ni intensité) :
   on retire et on rajoute. C'est le prochain raffinement.
2. L'ajout utilise une position et une intensité fixes (`(3,5,3)`, 1,0) : pas de saisie.
3. Le panneau n'est pas couvert par un test d'interface (mock sans paquet 3D).
4. Pas d'undo dédié.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Deux voies sur le même état
- [x] Test d'acceptation et de refus
- [x] Limites inscrites (§4)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.5.8` créé par un humain
