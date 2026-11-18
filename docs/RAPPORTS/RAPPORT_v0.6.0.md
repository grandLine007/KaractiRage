# Rapport de version — v0.6.0 « Studio 3D complet, Hub nettoyé »

| Champ | Valeur |
|---|---|
| Version | `v0.6.0` |
| Phase | 3D.6 du [`PLAN_STUDIO_3D`](PLAN_STUDIO_3D.md) — clôture du plan |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.9.md`](RAPPORTS/RAPPORT_v0.5.9.md) |

## 1. Ce qui change

Le bloc « Enregistrer dans le projet / Placer dans la scène » disparaît du Hub. Il était
**redondant** depuis que l'import vit dans la barre du viewport 3D, et c'était le piège
initial : un premier bouton qui ne produisait rien de visible.

À la place, une consigne :

> « Pour l'utiliser : passez en **vue 3D**. Le bouton *Importer un modèle 3D* est dans la
> barre du viewport : un seul clic l'enregistre dans le projet et le place dans la scène. »

## 2. Le code mort retiré

| Élément | Pourquoi il existait |
|---|---|
| `enregistrerMesh` | ancien flux d'enregistrement du Hub |
| `placerMesh` | ancien placement en deux temps |
| `bufferVersBase64` | encodage pour l'ancien import |
| `MESH_LICENCES` + `meshLicenseId/Custom` | licence saisie dans le Hub (reste au viewport) |
| `meshSaving`, `meshSavedId`, `meshPlacing` | états du bloc disparu |
| `onSceneChanged`, `importMeshAsset`, `placeMeshAsset`, icône `Save` | dépendances devenues inutiles |

## 3. Preuves

```text
tsc -b → 0 erreur · npm run build → OK · lint → 14/0
audit:ui --strict → 39 captures, 0 violation, 0 dérive (Hub inchangé visuellement
  dans les captures : la consigne remplace des boutons non capturés)
cargo test --offline --workspace → 613 passed, 0 failed
export référence → 3 niveaux, 5114 Ko, smoke : 3/3 niveaux, 4/4 écrans, 296,4 FPS
npm run check:versions → 0.6.0 partout
```

## 4. Bilan du plan 3D

| Lot | Capacité | Voie utilisateur | Voie IA | Même fonction moteur |
|---|---|---|---|---|
| 3D.1 | Transformée | champs X/Y/Z, rot, échelle | `SetTransform3D` | `set_entity_transform3d` |
| 3D.2 | Sélection | clic + `Suppr` | cible `entity:<id>` | `select_entity` / `delete_entity` |
| 3D.3 | Caméra | panneau position/cible + recadrer | `SetCamera` | `set_camera3d` |
| 3D.4 | Lumières | liste, ajout, retrait | `AddLight` | `add_light` / `remove_light` |
| 3D.5 | Matériaux | couleur, rugosité, métal | `SetMaterial` | `set_entity_material3d` |
| 3D.6 | Nettoyage Hub | — | — | — |

## 5. Limites inscrites

1. **Pas d'undo dédié** aux opérations 3D : la sauvegarde du projet reste le point de retour.
2. Pas d'**orbite souris** : la caméra se règle par champs.
3. Pas de **gizmo** : pas de manipulation directe dans la scène.
4. Les panneaux 3D ne sont **pas couverts par Playwright** (mock sans paquet 3D) : les
   preuves sont moteur + build, pas visuelles.
5. Rendu 3D à l'**export** toujours absent (runtime 2D).
6. Un modèle à plusieurs matériaux n'en expose qu'un seul à l'inspecteur.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Les six lots du plan sont livrés
- [x] Code mort retiré, build vert
- [x] Limites inscrites (§5)
- [ ] Validation à l'écran par le propriétaire
- [ ] Tag `v0.6.0` créé par un humain
