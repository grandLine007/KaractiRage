# Rapport de version — v0.3.5 « Correctifs du premier test réel »

| Champ | Valeur |
|---|---|
| Version | `v0.3.5` |
| Phase | P1 (correctifs) |
| Date | 12 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.4.md`](RAPPORT_v0.3.4.md) |

> Premiers bugs signalés par le propriétaire **en utilisant le Studio**. Aucun n'était
> couvert par les tests existants — c'est la leçon principale de cette version.

## 1. Bugs signalés et corrigés

| # | Symptôme | Cause | Correctif |
|---|---|---|---|
| 1 | Créer un projet → « Config invalide : unknown variant `2D` » | le Studio envoyait `"2D"` alors que `GameDimension` attend `two_d` (serde `snake_case`) | mapping `dimensionToWire()` à la frontière + **alias serde** côté moteur pour accepter les deux |
| 2 | `⌘K` / `Ctrl+K` ne faisait rien | la barre inline n'est rendue qu'en **mode Édition**, mais le raccourci ouvrait l'état sans changer de mode | le raccourci bascule en mode Édition avant d'ouvrir |
| 3 | `cargo test` échouait après une partie réelle | le Studio **auto-sauvegarde** la scène de démonstration ; un test affirmait ses coordonnées d'origine | test rendu hermétique (position placée explicitement) |

## 2. Pourquoi les tests ne les ont pas vus

- **Bug 1** : `createProject()` renvoie un projet de démonstration quand `isTauri()` est
  faux. Le harnais AC1 tourne avec un IPC simulé : il n'a jamais traversé la
  désérialisation Rust. Le test ajouté désérialise désormais **le payload exact du Studio**
  (`new_project_config_accepte_le_payload_du_studio`).
- **Bug 2** : la vérification visait « la barre s'ouvre en mode Édition ». Personne ne
  testait le raccourci **depuis le mode Jouer**.
- **Bug 3** : les tests supposaient une scène figée, alors que l'application écrit dedans
  pendant une partie.

## 3. Preuves

```text
cargo test --workspace            → 495 passed, 0 failed
cargo test -p engine-core dimension_accepte → ok (2D, 2.5D, 3D, two_d acceptés)
cargo test -p engine-core new_project_config_accepte → ok (payload Studio + legacy)
npm run build                     → OK
npm run lint                      → 14 warnings, 0 erreur
npm run check:versions            → 0.3.5 partout
vérification navigateur           → Ctrl+K depuis le mode Jouer : barre présente et visible
```

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Sens de correction du `dimension` | **UI vers le contrat** (`two_d`) + alias Rust | le contrat du moteur est la référence ; l'alias protège les anciens clients |
| `Ctrl+K` hors mode Édition | **bascule** en mode Édition | la barre n'existe que là ; ne rien faire était le bug |
| Scène de démonstration auto-sauvegardée | **assumé** (comportement produit : le projet se souvient de la partie) | c'est au test d'être hermétique, pas à l'app de refuser d'enregistrer |

## 5. Dette et risque restants

| Élément | Sévérité | Note |
|---|---|---|
| 2 autres assertions de position absolue subsistent dans les tests | moyenne | elles peuvent casser si une partie réelle déplace le joueur avant un run |
| Le Studio écrit dans `assets/scenes/level_01.scene.json` (fichier versionné) | moyenne | assumé, mais il faut des tests hermétiques partout, ou un projet de test dédié |
| `createProject` en mode non-Tauri renvoie une démo | faible | masque des contrats : à faire échouer explicitement en test |

## 6. Validation

- [x] Les trois bugs corrigés et prouvés
- [x] Tests de contrat ajoutés (payload Studio, alias d'étiquettes)
- [x] Gates vertes (§3)
- [ ] Tag `v0.3.5` créé par un humain
