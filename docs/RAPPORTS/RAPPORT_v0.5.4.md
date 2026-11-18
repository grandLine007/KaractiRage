# Rapport de version — v0.5.4 « Le modèle placé est visible »

| Champ | Valeur |
|---|---|
| Version | `v0.5.4` |
| Phase | C2/F (corrections 3D) |
| Date | 18 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.5.3.md`](RAPPORTS/RAPPORT_v0.5.3.md) |

> Deux retours : le sélecteur 2D/2.5D/3D restait visible avec un objet 3D ; un objet 3D
> placé n'était pas visible et une scène de démonstration tournait.

## 1. Ce qui causait les deux bugs

| Symptôme | Cause réelle |
|---|---|
| Sélecteur 2D/2.5D/3D encore proposé | le formulaire technique du Hub l'affichait même quand un maillage glTF était chargé |
| Scène par défaut qui tourne | `render_packet_frame` **auto-peuplait** une scène 3D de démonstration quand aucune entité 3D n'existait |
| Modèle placé invisible | il était posé **à la cible de la caméra** : le château (18 × 8 × 29,7 unités) englobait la caméra placée à z = 12 |

## 2. Correctifs

1. **Dimension imposée** : dès qu'un `.glb` est chargé, le `<select>` disparaît et
   « **3D (imposée par le modèle importé)** » le remplace.
2. **Plus de scène de démonstration automatique** : un projet 3D vide reste vide. Le
   contenu par défaut n'existe que par la commande explicite `engine_new_3d_scene`.
3. **Cadrage au placement** : le Studio ramène le modèle à ~4 unités (échelle calculée
   depuis la boîte englobante), et le moteur pose la caméra à 8 unités, visant le sol de
   l'entité.
4. Méthode `count_3d_entities` devenue inutile : retirée (0 warning).

## 3. Preuves

```text
GLB réel du propriétaire : Chateau_Gothique_Cour_Royale.glb (1 458 600 octets)
  importé      → chateau_gothique
  placé        → entité 43 « Chateau gothique » tag mesh3d
  paquet       → batch asset_id=14167, 1 instance
  caméra       → z=8.00, cible z=0.00, distance=8.00  (hors du modèle)

Playwright :
  avant import → 1 sélecteur de dimension
  après import → 0 sélecteur · « 3D (imposée par le modèle importé) »

cargo test --offline --workspace → 610 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
audit:ui --strict → 39 captures, 0 violation, 0 dérive
export référence → 3 niveaux, 5114 Ko, moteur v0.5.4
```

## 4. Ce qui reste — et qui manque encore

Le second point du propriétaire n'est **pas entièrement résolu** :

| Manque | Effet |
|---|---|
| Aucun **gizmo** de transformation 3D | on ne peut pas déplacer/rotationner à la souris |
| Aucun panneau de **transformée 3D** | l'inspecteur montre encore X/Y/Largeur/Hauteur (2D) |
| Pas de contrôles de caméra | orbite figée sur le cadrage automatique |
| Pas de gestion des **lumières/matériaux** | le modèle s'affiche en blanc |

Autrement dit : le modèle est **visible** et **cadré**, mais l'atelier n'est pas encore un
studio 3D. C'est le lot suivant, et il est plus gros que ces correctifs.

## 5. Validation

- [x] Les deux causes identifiées et corrigées
- [x] Vérifié avec le GLB réel de l'utilisateur
- [x] Gates vertes (§3)
- [x] Manques restants inscrits (§4)
- [ ] Tag `v0.5.4` créé par un humain
