# Glossaire IA — KaractiRage

Termes FR/EN, synonymes et pièges à éviter dans les intentions et les prompts.

## Concepts

| Terme | Définition |
|---|---|
| Intention | Demande utilisateur en langage naturel, normalisée en `IntentEnvelope`. |
| Artefact | Unité générée : projet, scène, niveau, mécanique, sprite, mesh, audio, UI. |
| Op | Patch atomique validé (`AiOp`) appliqué sur un clone puis commité. |
| Nœud KVL | Élément du graphe logique (événement / condition / action). |
| Catalogue | Les 75 `NodeSchema` (50 actions + 25 événements) du moteur. |
| Playbook | Recette par genre (plateforme, RPG, shmup, puzzle, boss, 3D). |

## Synonymes (FR → EN)

- clé / cle → key
- coffre → chest
- piège / lave / piques → hazard
- garde / sentinelle / patrouille → guard
- portail / téléporteur → portal
- boss → boss
- objectif → objective

## Pièges (à ne PAS utiliser)

- `OnEntityDefeated` : n'existe pas. Utiliser `OnBossDefeated`.
- `WinGame` : n'existe pas. Utiliser `PlaySound:victory_bell` + variable `game_won`.
- `PlayerDeath` (condition de défaite) : valeur fantôme. Utiliser `PlayerHealthZero`.
- `movement: "jump"` : non géré par le moteur. Valeurs réelles : `patrol`, `chase`, `none`.
- `SetObjective:<texte>` : valide — affiche le texte via la variable KVL `objective`.
- `SetVar:…` : ne transporte que des **entiers** (pas de texte).
