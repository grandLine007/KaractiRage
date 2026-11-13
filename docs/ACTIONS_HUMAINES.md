# Actions humaines — mode d'emploi

> **Pourquoi ce document** : six actions ne peuvent pas être faites par l'agent (contrat,
> droits, ou jugement humain). Voici comment les exécuter, dans l'ordre, avec les
> commandes exactes et ce qu'il faut me renvoyer.
> Dernière mise à jour : 13 novembre 2026 · commit `75cd528`.

---

## 1. Taguer les versions

### Ce qu'il faut savoir d'abord

Les rapports de version ont été publiés **en retard** (un `.gitignore` masquait
`docs/RAPPORTS/`). Résultat : pour `v0.3.0`, `v0.3.1` et `v0.3.3`, le code affichait
encore une version antérieure (`0.2.0` ou `0.3.2`) au commit du rapport.

| Tag | Commit | Ce que le code annonce | Taguer ? |
|---|---|---|---|
| `v0.3.0` | `0ead8bb` | 0.2.0 | ⚠️ incohérent |
| `v0.3.1` | `d5e1833` | 0.2.0 | ⚠️ incohérent |
| `v0.3.2` | `4c52c1c` | 0.3.2 | ✅ |
| `v0.3.3` | `116e120` | 0.3.2 | ⚠️ incohérent |
| `v0.3.4` | `c732267` | 0.3.4 | ✅ |
| `v0.3.5` | `c051ffb` | 0.3.5 | ✅ |
| `v0.3.6` | `75cd528` | 0.3.6 | ✅ |

### Commandes recommandées (série cohérente)

```bash
cd /Users/macbookair/Documents/Proj/KaractiRage

git tag -a v0.3.2 4c52c1c -m "v0.3.2 — cascade CSS corrigée, manifeste branché au pré-vol"
git tag -a v0.3.4 c732267 -m "v0.3.4 — AC1 mesuré, pile Échap LIFO, garde-fou runtime périmé"
git tag -a v0.3.5 c051ffb -m "v0.3.5 — correctifs du premier test réel (création, Ctrl+K, parse LLM)"
git tag -a v0.3.6 75cd528 -m "v0.3.6 — boss combattant, export autonome des trois niveaux"

git push origin v0.3.2 v0.3.4 v0.3.5 v0.3.6
```

### Si tu veux la série complète

Ajoute une mention explicite pour ne pas mentir dans le tag :

```bash
git tag -a v0.3.0 0ead8bb -m "v0.3.0 — rapport publié plus tard ; le code s'annonçait encore 0.2.0"
git tag -a v0.3.1 d5e1833 -m "v0.3.1 — rapport publié plus tard ; le code s'annonçait encore 0.2.0"
git tag -a v0.3.3 116e120 -m "v0.3.3 — export jouable ; rapport rattrapé dans ce commit, code en 0.3.2"
git push origin v0.3.0 v0.3.1 v0.3.3
```

### Vérifier

```bash
git tag -n1                      # liste + première ligne du message
git describe --tags --always     # ce que voit un outil à HEAD
git ls-remote --tags origin      # ce qui est réellement poussé
```

**Ne pas** utiliser `git push --tags` si tu ne veux pas pousser d'anciens tags locaux
oubliés : liste toujours les tags explicitement.

---

## 2. Relire les commits

### Vue d'ensemble (2 min)

```bash
git log --oneline 9ae2733..HEAD          # 31 commits depuis le rapport v0.2.0
git diff --stat 9ae2733..HEAD           # 113 fichiers, +14 241 / −868
git diff --check 9ae2733..HEAD          # espaces parasites, marqueurs de conflit
```

### Par zone, dans l'ordre de risque (le plus utile)

```bash
# 1. Le contrat d'export (le plus critique : c'est le fichier partagé)
git diff 9ae2733..HEAD -- crates/engine-core/src/exporter.rs

# 2. Le moteur gameplay (boss, écrans, sauvegarde, replay)
git diff 9ae2733..HEAD -- crates/engine-core/src/game.rs

# 3. Le web (runtime wasm, entrées, template)
git diff 9ae2733..HEAD -- crates/engine-wasm/src/lib.rs \
    crates/engine-core/src/standalone_template.html

# 4. L'interface
git diff 9ae2733..HEAD -- apps/studio/src apps/studio/src-tauri/src/lib.rs

# 5. Les tests ajoutés (ce qu'ils prouvent vraiment)
git diff 9ae2733..HEAD -- '*_test*' 'crates/**/tests/**'
```

### Ce que je te conseille de vérifier en priorité

| Point | Où | Question à se poser |
|---|---|---|
| Le paquet d'export contient-il **tout** ce qu'il faut ? | `exporter.rs`, `create_package` | entités, scènes, graphes, assets |
| Le pré-vol peut-il bloquer à tort ? | `exporter.rs`, `preflight` | un projet sain passe-t-il ? |
| Le runtime embarqué est-il déterministe ? | test `export_html..._deterministe` | deux exports = même sha256 |
| Les tests de bout en bout utilisent-ils de **vraies** interactions ? | `game.rs`, `reference_*` | pas de téléportation déguisée |
| Rien de secret ni d'artefact généré commité ? | `git ls-files` | `dist/`, `target/`, `export/` absents |

---

## 3. Protéger les branches

GitHub → **Settings** → **Rules** → **Rulesets** (ou **Branches**) → *New ruleset*.

Pour `main` **et** `feat/tauri-react-studio` :

| Réglage | Valeur recommandée |
|---|---|
| Target | les deux branches |
| Require a pull request before merging | ✅ (1 approbation) |
| Require status checks | ✅ — les 3 jobs : `Moteur Rust (engine, KVL, wasm)`, `Studio TypeScript`, `Backend Tauri` |
| Require branches to be up to date | ✅ |
| Block force pushes | ✅ |
| Restrict deletions | ✅ |
| Bypass list | toi uniquement (admin) |

Sans cela, un push direct peut écraser l'historique que les rapports décrivent.

---

## 4. Restreindre le token

GitHub → **Settings** → **Developer settings** → **Personal access tokens** → le token
utilisé (`karactirage-pat` dans le trousseau).

### Permissions minimales

| Permission | Accès | Pourquoi |
|---|---|---|
| **Contents** | Read and write | pousser des commits |
| **Pull requests** | Read and write | ouvrir/commenter une PR |
| **Metadata** | Read (imposé) | — |
| Administration | **retirer** | aucun besoin, risque maximal |
| Workflow | **retirer** | nécessaire une seule fois pour créer `ci.yml` |
| Actions | **ajouter si tu veux** | me permettre d'annuler/relancer la CI |

Avec `Actions: write`, je pourrais relancer un run rouge sans toi. Sans lui, c'est le clic
« Re-run failed jobs » qui reste à ta main.

⚠️ Retirer `Workflow` m'empêchera de **modifier** `.github/workflows/` — c'est le but.

---

## 5. Chronométrer AC1 (à la main)

C'est le seul critère d'acceptation qui n'a qu'une **borne machine** (3,3 s). Ton chrono
humain clôt AC1 pour de vrai.

### Protocole

1. Ferme l'application. Rouvre-la (`./target/debug/karactirage-studio`).
2. **Démarre le chrono à l'instant où tu cliques sur l'icône.**
3. Va au bouton **« Créer mon premier jeu »** sans lire la doc.
4. Remplis le nom, choisis un modèle, clique **« Créer & ouvrir dans le Studio »**.
5. **Arrête le chrono** quand le mode **Jouer** affiche le jeu à l'écran.

### À me renvoyer

| Mesure | Ta valeur |
|---|---|
| Temps total (icône → jeu visible) | ______ s |
| Nombre d'hésitations / clics perdus | ______ |
| Qu'est-ce qui t'a ralenti ? | ______ |
| Le jeu était-il jouable sans rien lire ? | oui / non |

Je l'inscrirai dans un `PREUVES_HUMAINES.md` et je repasserai AC1 de « borne basse » à
« mesuré ».

---

## 6. Valider le focus clavier

Dernière case P1 non cochée : « aucune touche avalée entre écrans ».

| Écran | Touches à tester | Attendu |
|---|---|---|
| Menu | `Entrée`, `Échap` | démarre / ne fait rien d'anormal |
| Jeu | `←` `→` `↑` `↓`, `E`, `F`, `Échap` | déplacement, interaction, tir, pause — **sans faire défiler la page** |
| Pause | `Échap`, `Entrée` | reprend / reste cohérent |
| Défaite | `Entrée`, `Échap` | retour au menu |
| Victoire | `Entrée` | retour au menu |
| Édition | `Ctrl+K`, `Échap`, `Ctrl+Z` | barre IA, fermeture, annulation |
| KVL | clic nœud, glisser | pas de perte de focus |

À me renvoyer : écran + touche + ce qui s'est passé si ce n'est pas l'attendu.

---

## 7. Ce que j'attends de toi (résumé)

| # | Action | Temps | Sortie |
|---|---|---|---|
| 1 | Tags | 2 min | sortie de `git ls-remote --tags origin` |
| 2 | Revue | 30-60 min | remarques, ou « rien à signaler » |
| 3 | Protection | 5 min | capture de la règle |
| 4 | Token | 5 min | liste des permissions finales |
| 5 | AC1 | 5 min | le tableau du §5 |
| 6 | Focus | 10 min | anomalies éventuelles |

Dès que j'ai le tableau §5, je peux clore AC1 et inscrire ta mesure dans le rapport.
