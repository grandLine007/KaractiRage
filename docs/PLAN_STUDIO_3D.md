# Plan — studio 3D pilotable par l'utilisateur **et** par l'IA

| Champ | Valeur |
|---|---|
| Statut | plan de travail |
| Date | 18 novembre 2026 |
| Référence | [`CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md`](CONTRAT_GLOBAL_KARACTIRAGE_2D_3D_IA.md) §5.2, §9.2 |
| Objectif | chaque capacité 3D a **deux commandes** : un contrôle visible, une opération IA |

## 1. La règle des deux commandes

Une capacité n'est livrée que si elle existe **deux fois** :

| Voie | Forme | Contrainte |
|---|---|---|
| Utilisateur | champ, bouton, gizmo | visible, annulable (`⌘Z`), sans jargon |
| IA | opération du catalogue + cible | même fonction moteur, même validation, même undo |

Si l'une des deux manque, la capacité est incomplète. C'est cette règle qui a
manqué au placement de mesh : l'opération existait, le contrôle menait à un cul-de-sac.

Chaque capacité réutilise **la même fonction de moteur**. L'interface ne contourne
jamais le contrat d'édition ; l'IA ne contourne jamais la validation.

## 2. Les capacités à couvrir

| Capacité | Contrôle utilisateur | Opération IA | Cible |
|---|---|---|---|
| Sélectionner un objet 3D | clic dans le viewport, surbrillance | `SelectEntity` | `entity:<id>` |
| Déplacer / tourner / mettre à l'échelle | champs X Y Z, rotation, échelle | `SetTransform3D` | `entity:<id>/transform3d` |
| Matériau | couleur, rugosité, métal | `SetMaterial` | `entity:<id>/material` |
| Caméra | orbite, cible, FOV, réinitialiser | `SetCamera` | `camera3d` |
| Lumières | ajouter, type, couleur, intensité, supprimer | `AddLight` / `UpdateLight` / `RemoveLight` | `lights` |
| Importer un mesh | bouton du viewport (fait) | `PlaceMesh` | `asset:<id>` |
| Supprimer | touche `Suppr`, bouton | `DeleteEntity` | `entity:<id>` |

## 3. Ordre de travail

| Lot | Contenu | Critère de sortie |
|---|---|---|
| 3D.1 | **Inspecteur 3D** : champs position/rotation/échelle quand l'entité sélectionnée porte `Transform3D` | modifier la position déplace l'objet dans le viewport ; une opération IA équivalente donne le même résultat |
| 3D.2 | **Sélection 3D** : clic → entité sélectionnée, surbrillance, `Suppr` | cliquer sur le château le sélectionne, l'inspecteur s'ouvre |
| 3D.3 | **Caméra** : orbite souris, cible, réinitialiser | orbite sans perdre l'objet ; opération IA `SetCamera` équivalente |
| 3D.4 | **Lumières** : panneau minimal (une lumière ajoutée par défaut) | le modèle cesse d'être blanc ; opération IA `AddLight` |
| 3D.5 | **Matériaux** : couleur/rugosité dans l'inspecteur | changer la couleur se voit immédiatement |
| 3D.6 | **Nettoyage Hub** : retirer le bloc « Enregistrer / Placer » et le code mort | build vert, aucune fonction orpheline |

## 4. Ce que chaque lot doit prouver

- `npm run build`, `tsc -b`, `npm run lint` (14/0), `cargo test --offline --workspace` ;
- une preuve Playwright ou un test Rust pour la voie IA **et** la voie utilisateur ;
- une recette humaine de trois lignes dans `docs/RECETTE_DEMARRAGE.md` ou son équivalent 3D ;
- un rapport de version.

## 5. Limites assumées

- Le **gizmo** (poignées à la souris dans le viewport) est un lot à part : les champs
  numériques le précèdent, parce qu'ils se testent et se pilotent par IA sans ambiguïté.
- Le **rendu 3D à l'export** reste hors périmètre (lot P4).
- Les animations de clips ne sont pas jouées (comptées seulement).
