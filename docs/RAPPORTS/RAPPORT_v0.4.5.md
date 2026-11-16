# Rapport de version — v0.4.5 « Consolidation avant P3 »

| Champ | Valeur |
|---|---|
| Version | `v0.4.5` |
| Phase | dette P2 (avant P3) |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.4.md`](RAPPORTS/RAPPORT_v0.4.4.md) |

> Trois dettes soldées avant d'ouvrir P3 : portabilité des projets Tiled, musique de
> fond pilotable par KVL, console propre hors Tauri.

## 1. Ce qui change

| Dette | Origine | Correctif |
|---|---|---|
| Chemin d'atlas **absolu** | v0.4.2 §5.1 | `tilesets/<nom>.png` relatif au projet, résolu via `racine_projet` |
| `LoopSound` absent de KVL | v0.4.3 §5.3 | `LoopSound:<id>` + `StopSounds`, registre KVL mis à jour |
| `invoke` non gardé dans `GameViewport` | v0.4.1 §5.4 | helper `envoyerMoteur()` : no-op hors Tauri, sans masquer les erreurs natives |

## 2. Preuve de portabilité

```text
import carte.tmx → asset = "tilesets/tuiles.png"   (relatif)
mv /tmp/carte_portable /tmp/carte_deplacee
export depuis le dossier déplacé → OK, 5084 Ko
smoke → runtime=running, niveaux=1/1, écrans=4/4, entrée=2580 px, 345,4 FPS
```

Le test automatisé fait la même chose : import, `std::fs::rename` du dossier, réouverture,
vérification que `sprite_cache` contient l'atlas **et** que le rendu produit les pixels des
deux tuiles (rouge et bleu).

## 3. Boucle de musique

`LoopSound:<id>` dépose une demande, consommée une fois par frame après `PlaySound`.
`StopSounds` coupe **uniquement** les boucles — les effets ponctuels continuent, ce qui est
le comportement attendu d'un jeu. Un id sans WAV est ignoré (pas de repli procédural pour
une musique), sans panic.

## 4. Preuves

```text
cargo test --offline --workspace → 598 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · lint → 14/0 · check:versions → 0.4.5

export référence → 3 niveaux, 24 entités, 5114 Ko
smoke référence → 3/3 niveaux, 4/4 écrans, entrée=6198 px, 363,1 FPS
smoke carte déplacée → 1/1 niveau, entrée=2580 px, 345,4 FPS
dictionnaire IA régénéré (dist/ai) pour les nouvelles actions
```

Tests ajoutés : projet déplacé (résolution + rendu), `LoopSound` démarre/`StopSounds`
arrête, id inconnu inoffensif, `StopSounds` ne coupe pas les effets ponctuels.

## 5. Limites inscrites

1. **Ouverture par JSON brut** (sans chemin disque) : `racine_projet = None`, résolution
   historique — l'interface devra transmettre le dossier quand elle le connaît.
2. **Ancrage limité aux PNG** (`ImageRef`, `Tilemap`) ; `SheetRef`, `AnimRef`, `SvgRef`
   gardent leur résolution historique.
3. Déplacer le `.karacti` **sans** son dossier `tilesets/` casse toujours (attendu) ;
   « Enregistrer sous » ne copie pas encore les assets.
4. Un `LoopSound` sans WAV n'est pas bloquant au pré-vol (cohérent avec « silencieux »).
5. Ancien projet à chemin absolu : fonctionne, mais reste non portable.

## 6. Validation

- [x] Gates vertes (§4)
- [x] Portabilité prouvée en déplaçant réellement un dossier
- [x] Musique de fond pilotable par un graphe
- [x] Console propre hors Tauri
- [x] Limites inscrites (§5)
- [ ] Tag `v0.4.5` créé par un humain
