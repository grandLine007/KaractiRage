# Rapport de version — v0.4.3 « Les sons viennent de fichiers »

| Champ | Valeur |
|---|---|
| Version | `v0.4.3` |
| Phase | **P2.3** (audio par fichiers) |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.2.md`](RAPPORTS/RAPPORT_v0.4.2.md) |

> Le moteur ne sait plus seulement synthétiser : il **joue les WAV de l'auteur**, en
> natif et dans l'export. Sans Kira, sans dépendance.

## 1. Décision : Kira abandonné pour l'instant

L'arbre de Kira 0.10 (~40 crates : `cpal`, `symphonia` et ses décodeurs) n'est pas en
cache, et `crates.io` est bloqué. Le vendoring de `tiled` (2 crates) était raisonnable ;
40 ne l'est pas. Un décodeur WAV écrit à la main couvre le besoin sans une seule
dépendance.

## 2. Ce qui est livré

| Élément | Détail |
|---|---|
| Décodeur RIFF/WAVE | PCM 8/16/24/32 bits, float32, extensible, mono/stéréo → mono |
| Mixage | `ActiveVoice` = procédural **ou** échantillon ; rééchantillonnage linéaire |
| `PlaySound:<id>` | clip chargé prioritaire → repli procédural → silence (comportement historique intact) |
| Musique de fond | `loop_clip` (une seule à la fois), `stop_loops` |
| Export | `audio_assets` (WAV PCM16 mono base64) dans le paquet, décodé en mémoire à l'import |
| Pré-vol | `PlaySound` sans clip ni son procédural → **blocage** `asset-manquant` |

## 3. Preuves

```text
cargo test --offline --workspace → 581 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run check:versions → 0.4.3 partout

export → 3 niveaux, 24 entités, 5104 Ko
smoke → runtime=running, niveaux=3/3, écrans=4/4, entrée=6198 px, 274,8 FPS
```

Preuve d'embarquement : un `assets/audio/ding.wav` temporaire a été déposé, l'export
contient `"audio_assets":{"ding":"UklGR…"}` (11 occurrences), puis le fichier a été
supprimé — `git status assets/` est vide.

Tests (22) : décodage PCM 8/16/24/32 et float, stéréo → mono, en-tête invalide, ADPCM
refusé, données tronquées, mélange de deux clips sans saturation, libération des voix,
rééchantillonnage conservant la durée, boucle de musique coupée, `PlaySound` priorité
clip puis repli, survie de la banque aux reconstructions, export/import qui rejoue le
clip.

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Support de format | WAV PCM/float uniquement | décodable sans dépendance ; OGG/MP3 exigent Kira |
| Canaux | mixés en mono | l'appareil de sortie est mono dans tout le moteur |
| Export | ré-encodage PCM16 mono | taille et compatibilité ; les sources 24/32 bits sont quantifiées |
| Priorité sonore | fichier > procédural > silence | un auteur qui fournit un son veut l'entendre |

## 5. Limites inscrites

1. **Pas de streaming** : le fichier entier est décodé en RAM (3 min à 44,1 kHz ≈ 31 Mio,
   ≈ 4/3 en base64 dans le HTML).
2. Formats refusés avec message clair : ADPCM, A-law/µ-law, GSM, MP3, OGG/Vorbis, RF64,
   plus de 2 canaux.
3. Pas d'action KVL `LoopSound` : la boucle existe côté moteur (`loop_clip`) mais n'est
   pas encore exposée dans KVL.
4. `KaractiProject::to_game_package` (chemin historique) n'embarque pas les WAV ; seul
   `StandaloneExporter::create_package` le fait.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Aucune dépendance ajoutée
- [x] Comportement audio procédural inchangé (mêmes sons, même volume, même mute)
- [x] WAV réellement embarqué dans l'export, dépôt propre
- [x] Limites inscrites (§5)
- [ ] Tag `v0.4.3` créé par un humain
