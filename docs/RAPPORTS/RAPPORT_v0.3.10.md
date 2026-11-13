# Rapport de version — v0.3.10 « Les sprites SVG voyagent »

| Champ | Valeur |
|---|---|
| Version | `v0.3.10` |
| Phase | P1 (export) |
| Date | 13 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.9.md`](RAPPORT_v0.3.9.md) |

> Dette §5.3 (SVG non embarqués) soldée. Une nuance importante : le jeu de référence
> n'utilise **aucun** `SvgRef` — son art est procédural. Le correctif profite aux projets
> qui référencent des SVG.

## 1. Le vrai périmètre du bug

| Cas | Avant | Après |
|---|---|---|
| `hero`, `decor_torch`, `decor_crate` | rendaient déjà (repli `include_str!` compilé) | inchangé |
| SVG **arbitraire** présent seulement sur disque (`import_01`, œuvres du VectorWorkspace) | **rectangle vert uniforme** | sprite rasterisé |
| Sprite déjà résolu porté par une scène | écrasé par `None` à la résolution disque | conservé |

Le shadowing par `include_str!` masquait le défaut dans toutes les démos existantes. Le
cas réel est un SVG **non compilé** dans le binaire — celui qu'un auteur importe.

## 2. Preuve visuelle

Comparaison (même runtime, même scène, seule la présence de `svg_assets` change) :

- **Avant** : rectangle `#00FF00`.
- **Après** : l'œuvre SVG rasterisée (feuillage pixelisé), `window.__karacti_level = 1`.

Capture : `/tmp/svg_avant_apres.png` — le test automatisé mesure en plus **> 100 px** de
magenta du SVG quand la source est présente et **0 px** quand elle manque.

## 3. Preuves techniques

```text
cargo test --workspace        → 528 passed, 0 failed
cargo check --workspace       → 0 warning
npm run check:versions        → 0.3.10 partout

export référence → 3 niveau(x), 24 entités, 4 graphes, 5007 Ko
export-smoke référence → OK, niveaux=3/3, écrans=4/4
export-smoke preuve    → OK, 43 entités, niveaux=2/2, écrans=4/4
deux exports → même sha256 (référence et preuve)
```

Tests ajoutés : `svg_assets` de tous les niveaux ; résolution + survie à
`start_game`/`next_level`/`reload` ; SVG absent du disque rendu et non remplacé par le
repli ; ancien paquet sans `svg_assets` ; non-destruction du `vector_sprite` ; priorité de
la source embarquée ; `current_level()` (base de `wasm_get_level`).

## 4. Décisions

| Décision | Choix | Justification |
|---|---|---|
| Embarquer les **sources** SVG (texte) plutôt que cuire les images | option A | ~2 Ko, rigs/animations reconstruits à l'import, export déterministe |
| Ne pas modifier le `.karacti` de référence | art procédural voulu | le convertir en SVG aurait dégradé le showcase |
| Preuve sur un projet `/tmp` (`level_01` + démo héros + SVG disque) | aucun contenu inventé dans le dépôt | `level_01` contient déjà 3 `SvgRef` |
| Non-destruction du sprite existant | règle générale | ne jamais détruire ce que la scène porte |

## 5. Limites inscrites

1. Le jeu de référence a `svg_assets` vide : **son export ne change pas** (0,016 % de
   pixels, phase d'animation). C'est attendu.
2. Les `assets/sprites/{id}.anim.json` ne voyagent pas : en wasm, `default_clips` génère
   `idle`/`walk` ; les clips personnalisés ne sont pas embarqués (limite préexistante).
3. `__karacti_level` est rafraîchi à chaque frame, mais le smoke ne le vérifie qu'au
   démarrage (il faudrait atteindre le niveau 2 avec de vraies interactions).
4. `project.rs` a reçu une ligne (`svg_assets: HashMap::new()`) pour que le littéral
   compile — hors périmètre initial, signalé.

## 6. Validation

- [x] Gates vertes (§3)
- [x] Bug prouvé avant/après, y compris la non-régression du showcase
- [x] Dette v0.3.9 §5.3 soldée
- [x] Limites inscrites (§5)
- [ ] Tag `v0.3.10` créé par un humain
