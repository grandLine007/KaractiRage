# Rapport de version — v0.3.4 « AC1 mesuré, P1 soldé »

| Champ | Valeur |
|---|---|
| Version | `v0.3.4` |
| Phase | clôture de **P1** |
| Date | 11 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.3.3.md`](RAPPORT_v0.3.3.md) |

> Vague 6 : deux agents (ν garde-fou runtime, ξ AC1 + pile Échap), corrections et
> intégration par le meneur.

## 1. Périmètre annoncé vs livré

| Élément | Livré | Écart |
|---|---|---|
| **AC1** — « nouveau jeu jouable en < 2 min » **mesuré** | ✅ | **borne basse automatisée** : 3,25 s ; un chrono humain reste à faire |
| `Échap` ferme la couche la plus récemment ouverte (LIFO) | ✅ | testé dans les deux ordres |
| Garde-fou contre un runtime `dist/pkg` **périmé** | ✅ | avertissement `runtime-perime`, méta `karacti-runtime` |
| Bug du hub : hauteurs SVG négatives (9 erreurs console) | ✅ | corrigé (décalage non signé) |
| Alignement de version | ✅ | le code était resté en 0.3.2 alors que le rapport v0.3.3 existait → **0.3.4** |

## 2. Preuves

```text
node scripts/first-game.mjs --json
→ ok=true · elapsedMs=3249 · ac1.pass=true · interaction.ok=true · problems=[]
  (3 runs de l'agent : 2 995 / 3 293 / 3 264 ms ; plage 2,6–3,3 s)

cargo test --workspace        → 493 passed, 0 failed
cargo check --workspace --tests → 0 warning
npm run build                 → OK
npm run lint                  → 14 warnings, 0 erreur
npm run audit:ui -- --strict  → 39 captures, 0 violation, 0 dérive
node scripts/export-smoke.mjs /tmp/ref_034.html --expect-runtime --expect-input
→ OK — runtime=running, 960000 px non noirs, entrée=5778 pixels modifiés
npm run check:versions        → 0.3.4 partout
```

## 3. AC1 : ce qui est prouvé et ce qui ne l'est pas

**Prouvé** : le parcours automatisé Hub → « Créer mon premier jeu » → nom + modèle →
Studio → mode Jouer → canvas rendu prend **~3,3 s** sur cette machine, avec un seuil
d'échec à 120 s. La mesure est stable (3 runs) et les étapes sont détaillées.

**Non prouvé** : le temps **humain**. Le harnais utilise Chrome headless et un IPC Tauri
simulé : c'est une **borne basse**, sans boot natif, sans lecture ni hésitation. Le rapport
du script l'affiche littéralement — il ne prétend pas mesurer une personne. Un chrono
humain (5 clics réels) reste à faire pour clore AC1 sans réserve.

**Arbitrage tranché** : ma consigne contenait une contradiction (« ouvre la barre IA puis
le panneau, `Échap` → la barre se ferme »). ξ a appliqué la bonne règle — **LIFO**, la
couche la plus récemment ouverte se ferme d'abord — et l'a signalée au lieu de l'ignorer.
C'est la règle retenue.

## 4. Décisions prises

| Décision | Choix | Justification |
|---|---|---|
| Ordre `Échap` | **pile LIFO** (`lastOpenedLayer`) + handler en phase capture | le listener local de la barre IA passait avant celui de l'app |
| Garde-fou runtime | manifeste sans horodatage + avertissement par **version** | un manifeste daté casserait le déterminisme de l'export |
| Test de pré-vol | versionné hermétique (`preflight_with_runtime_dir`) | un test ne doit pas échouer parce que `dist/pkg` est périmé |
| Hub | `>>>` au lieu de `>>` | `>>` sur un u32 donne un négatif → `height` SVG < 0 → 9 erreurs console |

## 5. Métriques

| Métrique | Valeur | Cible |
|---|---|---|
| Tests Rust | **493** | ≥ 489 |
| Warnings cargo | 0 | 0 |
| Warnings lint | 14 | ≤ 14 |
| Violations axe | 0 | 0 |
| Dérive baseline | 0 | 0 |
| AC1 (borné, machine) | 3,25 s | < 120 s |
| Erreurs console à l'ouverture du hub | 0 (9 avant) | 0 |
| Export jouable | 4,7 Mo, 1 fichier | < 10 Mo |

## 6. Ce qui n'est pas livré

| Élément | Raison | Échéance |
|---|---|---|
| Chrono **humain** d'AC1 | nécessite une personne devant l'écran | release |
| Tiled (P2.2), Kira (P2.3) | crates.io 403, hors ligne | session réseau / vendor |
| Clics souris dans l'export | non demandé cette vague | lot gameplay |
| Passe typographique `h1-h6` | refusée avec preuve en v0.3.2, à faire délibérément | lot UI |

## 7. Dettes connues

| Dette | Sévérité | Coût | Quand |
|---|---|---|---|
| Chrono humain AC1 | moyenne | 15 min de quelqu'un | release |
| `dist/pkg` non versionné : à reconstruire après tout changement moteur | moyenne | automatisable en CI | lot CI |
| Manifeste runtime **cru** (sha non revérifié) | faible | — | si dist/pkg devient distribué |
| Seuil d'entrée du smoke inclut les animations | faible | baseline « sans entrée » | lot QA |
| Tiled/Kira bloqués réseau | **bloquant P2** | — | réseau / vendor |

## 8. Risques

| Risque | Probabilité | Impact | Mitigation |
|---|---|---|---|
| CI rouge à cause du réseau GitHub (déjà arrivé) | élevée | pas de validation distante | timeouts + retries + chemin rapide `dpkg` (v0.3.3) ; relance manuelle |
| Un export publié avec un runtime d'une autre version | faible | comportement incohérent | avertissement `runtime-perime` |

## 9. Gel et prochaine version

- **Gelé** : pile `Échap`, protocole `__karacti_runtime` (+ `__karacti_runtime_version`),
  format `runtime_manifest.json`, `KEY_MAP` du template.
- **P1 est soldé** : AC1 ✅ borné, AC2 ✅, AC3 ✅, AC4 ✅, AC5 ✅, AC6 ✅.
- **Prochaine version** : `v0.4.0` — P2 dès qu'un réseau ou un vendor est disponible.

## 10. Validation

- [x] Gates vertes (§2)
- [x] AC1 mesuré et le caractère « borne basse » explicitement dit (§3)
- [x] Écarts consignés (§1, §6)
- [x] Dette inscrite (§7)
- [ ] Tag `v0.3.4` créé par un humain
