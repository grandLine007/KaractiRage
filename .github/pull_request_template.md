## Objectif

<!-- Une phrase. Référence de tâche : KAR-xxx ou issue #xxx.
     Une branche = une tâche = une PR (contrat §5.2). -->

## Nature

- [ ] `docs` — documentation, plan, contrat
- [ ] `test` — tests manquants ou régression
- [ ] `fix` — correction bornée
- [ ] `feat` — fonctionnalité
- [ ] `chore` — outillage, CI, dépendances (**approbation humaine requise**)

## Changements

<!-- Liste courte, orientée comportement. Pas de diff collé. -->

## Preuves

<!-- Obligatoire : une ligne par gate exécutée, avec la sortie réelle. -->

| Gate | Commande | Résultat |
|---|---|---|
| Tests | `cargo test --workspace` | |
| Check | `cargo check --workspace` | |
| Build Studio | `npm run build` | |
| Lint Studio | `npm run lint` | |
| Audit UI | `npm run audit:ui -- --strict` | |

<!-- Si une gate n'a pas été lancée, écrire pourquoi (ex. changement Rust seul). -->

## Risques et rollback

- **Risque** : `safe` / `sensible` / `critique` — et pourquoi.
- **Rollback** : comment annuler (revert du commit, feature flag, restauration de fichier).

## Checklist (contrat §6)

- [ ] La tâche avait des critères d'acceptation explicites
- [ ] Les commits portent `Agent:` et `Task:`
- [ ] Les tests couvrent le changement (ou expliquent pourquoi non)
- [ ] `dist/ai` régénéré si le dictionnaire ou `AiOp` a changé
- [ ] Aucun secret, `.env`, artefact de build ou fichier généré par les tests commité
- [ ] Lint Studio : aucune erreur et aucun warning ajouté
- [ ] PR en **draft** ; merge humain uniquement
- [ ] DSH n'a ni mergé, ni taggé, ni publié

## Contexte IA (si pertinent)

- Cible résolue : `<entité ou nœud>`
- Proposition : `proposal_…` · transaction : `tx_…`
- Ops appliquées : `<update_entity …>`
