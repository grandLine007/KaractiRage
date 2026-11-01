# Playbook — Plateforme 2D

Boucle : saut → collecte (clé) → porte → boss.

## Structure type

1. Terrain : sol, plateformes, murs, hazards (piques/lave).
2. Entités : joueur (ninja), ennemis (drone/slime), clé, porte, boss.
3. KVL : objectif, victoire, défaite, collecte, interactions.

## Valeurs moteur (2D)

- Grille 32 px, gravité ~980, vitesse 180-260 px/s.
- Simulation **120 Hz** fixe (`SIM_DT = 1/120`).

## Événements utiles

`OnStart`, `OnPlayerOverlap`, `OnPlayerInteract`, `OnItemPickup`, `OnChestOpened`,
`OnKeyFound`, `OnPortal`, `OnBossAttack`, `OnBossDefeated`, `OnWinConditionMet`.

## Actions utiles

`PlaySound:`, `AddScore:`, `GiveItem:`, `HealPlayer:`, `DamagePlayer:`,
`TeleportPlayer:`, `ToggleDoor`, `SetBossPhase:`, `SetQuest:`, `SetVar:` (entier).

## Anti-modèles

- Ne pas utiliser `WinGame`, `OnEntityDefeated`, `SetObjective:<texte>`, `movement:"jump"`.
