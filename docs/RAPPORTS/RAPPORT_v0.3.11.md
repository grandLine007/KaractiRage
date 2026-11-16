# Rapport de version — v0.3.11 « Animations respectées, performance mesurée »

| Champ | Valeur |
|---|---|
| Version | `v0.3.11` |
| Phase | P1 (finitions export) |
| Date | 13 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.10.md`](RAPPORTS/RAPPORT_v0.3.10.md) |

> Deux dettes soldées : les clips d'animation SVG personnalisés voyagent, et la
> performance du jeu exporté est enfin mesurée par un test.

## 1. Ce qui change

| Élément | Avant | Après |
|---|---|---|
| `.anim.json` d'un sprite SVG | perdu en wasm → clips `idle`/`walk` générés | embarqué dans le paquet, **prioritaire** sur le disque et les défauts |
| Survie aux niveaux | — | réinstallé à `start_game` / `next_level` / `reload` |
| Performance du jeu exporté | jamais mesurée | **278,7 FPS** (3,59 ms/frame) mesurés par test |
| Smoke | 4 vérifications | + `--expect-fps 55` avec échecs distincts |

## 2. Preuves

```text
cargo test --workspace        → 538 passed, 0 failed
cargo check --workspace       → 0 warning
npm run check:versions        → 0.3.11 partout

export_game → 3 niveau(x), 24 entités, 4 graphes KVL, 5043 Ko
export-smoke --expect-runtime --expect-input --expect-levels 3 \
             --expect-screens 4 --expect-fps 55
→ OK — runtime=running, niveaux=3/3, écrans=4/4,
        entrée=6198 pixels modifiés, 278,7 FPS (3,59 ms/frame, 126 frames)
deux exports → même sha256
```

Tests ajoutés (10) : clips embarqués prioritaires sur disque et défauts ; repli disque
sans mémoire ; clips par défaut si rien ; `.anim.json` embarqué depuis le disque (réel de
`hero`) ; import qui utilise le fichier (clips `danse`/`saut`, distincts des défauts) ;
survie aux reconstructions ; ancien paquet ; absence totale ; ré-export conserve les clips.

## 3. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Priorité des clips | **paquet > disque > défauts** | sous wasm, seul le paquet existe ; au Studio, le disque reste la source vivante |
| Instrumentation FPS | moyenne glissante sur 120 frames, temps CPU de frame | robuste au bruit, mesure le coût réel du moteur |
| `--expect-fps` | seuil **explicite obligatoire** | un défaut implicite masquerait les régressions |
| Mesure non significative | échec si < 60 frames | mieux vaut échouer que valider sur une mesure vide |
| Fenêtre de mesure | complétion bornée (+3 s) | Chrome headless gèle `rAF` 1-3 s après la première frame |

## 4. Limites inscrites

1. **La mesure FPS est CPU, pas GPU** : elle couvre JS + wasm + copie canvas, pas la
   présentation. Un jeu léger côté CPU mais lent à l'écran passerait.
2. **Headless ≠ matériel réel** : Chrome headless utilise SwiftShader. Les 278 FPS sont
   une borne haute ; la validation 60 FPS sur le profil Low-End doit se faire aussi sur
   une vraie machine modeste.
3. Le format `.anim.json` actuel ne référence aucune image : si un futur format en
   référence, elles ne suivront pas.
4. Si plusieurs clips existent sans `idle`, le clip initial choisi peut varier
   (comportement préexistant, non modifié).
5. `KaractiProject::to_game_package` laisse `svg_assets`/`svg_anim_assets` vides (chemin
   historique) ; la collecte passe par `StandaloneExporter::create_package`.

## 5. Validation

- [x] Gates vertes (§2)
- [x] Deux dettes soldées et prouvées
- [x] Performance mesurée sur le vrai binaire (pas une fixture)
- [x] Limites inscrites (§4)
- [ ] Tag `v0.3.11` créé par un humain
