# Rapport de version — modèle

> Copier ce fichier en `docs/RAPPORTS/RAPPORT_vX.Y.Z.md` **avant** de créer le tag.
> Un rapport manquant = release refusée (règle du contrat de contribution, §6).

---

## En-tête

| Champ | Valeur |
|---|---|
| Version | `vX.Y.Z` |
| Phase | `P1` / `P2` / … |
| Date | `JJ mois AAAA` |
| Statut | `brouillon` / `prêt pour revue` / `publié` |
| Commit du tag | `abc1234` |
| Rapport précédent | `RAPPORT_vA.B.C.md` |

## 1. Périmètre annoncé vs livré

| Élément annoncé | Livré | Écart |
|---|---|---|
| | ✅ / ⚠️ / ❌ | |

## 2. Preuves (obligatoire)

Chaque ligne = une commande **exécutée**, avec sa sortie réelle.

```text
cargo test --workspace          → X passed, 0 failed
cargo check --workspace         → 0 warning
cd apps/studio && npm run build → OK
npm run lint                    → X warnings, 0 erreur (baseline : Y)
npm run audit:ui -- --strict    → N captures, 0 violation, 0 dérive
```

Captures et rapports : `apps/studio/audit/`, `docs/RAPPORTS/assets/`.

## 3. Ce qui n'est pas livré (et pourquoi)

| Élément | Raison | Nouvelle échéance |
|---|---|---|
| | | |

## 4. Métriques

| Métrique | Valeur | Cible | Écart |
|---|---|---|---|
| Tests | | | |
| Warnings lint | | ≤ 14 | |
| Violations axe | | 0 | |
| Dérive baseline audit | | 0 | |
| Latence IA (si P5) | | < 15 s local | |
| Taille d'export | | | |

## 5. Décisions prises

| Décision | Choix | Justification | Document |
|---|---|---|---|
| | | | |

## 6. Écarts et dérogations (registre de flexibilité)

Tout écart au plan d'intégration est inscrit ici : nature, cause technique, conséquence
sur l'objectif final, décision de rattrapage.

| Écart | Cause | Impact objectif | Rattrapage |
|---|---|---|---|
| | | | |

## 7. Dette technique connue

| Dette | Sévérité | Coût estimé | Quand |
|---|---|---|---|
| | | | |

## 8. Risques ouverts

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| | | | |

## 9. Gel et prochaine version

- **Gelé à partir de cette version** : (fonctionnalités qu'on n'étend plus)
- **Prochain objectif** : `vX.Y+1.Z` — phase `Pn`
- **Critères d'acceptation de la suivante** : (renvoi au document de phase)

## 10. Validation

- [ ] Toutes les gates vertes et citées au §2
- [ ] Aucun écart non consigné au §6
- [ ] La dette ajoutée est inscrite au §7
- [ ] Le périmètre gelé est annoncé au §9
- [ ] Le tag est créé par un humain (jamais par l'agent)
