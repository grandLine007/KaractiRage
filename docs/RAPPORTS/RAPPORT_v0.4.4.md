# Rapport de version — v0.4.4 « Rechargement à chaud » (fin de P2)

| Champ | Valeur |
|---|---|
| Version | `v0.4.4` |
| Phase | **P2.4** — clôture de P2 |
| Date | 16 novembre 2026 |
| Statut | **prêt pour revue** — non taguée |
| Rapport précédent | [`RAPPORT_v0.4.3.md`](RAPPORTS/RAPPORT_v0.4.3.md) |

> Dernière brique de P2 : modifier une scène ou un asset sur disque la recharge dans le
> Studio, en moins d'une seconde, sans rouvrir le projet.

## 1. Décision : pas de `notify`

`notify` apporte `crossbeam-channel`, `filetime`, `walkdir` **et un backend par
plateforme** (`fsevent`, `inotify`, `winapi`). Sur une machine où `crates.io` est bloqué,
c'est un vendoring à répétition pour un service que 4 `stat` par seconde rendent aussi
bien.

Un veilleur par interrogation de `mtime` : zéro dépendance, latence ~800 ms, testable.

## 2. Comportement

```text
Veilleur { poll() -> Vec<PathBuf> }            // trié, une fois par changement
NativeEngine::hot_reload_check() -> JSON       // { reloaded, fichiers[] }
engine_hot_reload_check()                      // commande Tauri
App.tsx : intervalle 800 ms → refresh() + toast
```

Machine à états :

| Situation | Effet |
|---|---|
| Premier passage / liste de cibles changée | photographie silencieuse, **pas** de rechargement |
| Aucun changement | `false` |
| Scène, script ou asset modifié | `reload()`, réponse `true` avec les fichiers |
| `.karacti` modifié | relecture + `load_project`, niveau et mode conservés |
| JSON tronqué (écriture en cours) | repli sur `reload()`, état conservé |

La veille est suspendue en mode hors-jeu, Hub ouvert, modale active, barre IA ouverte, ou
champ de saisie actif : le Studio ne se recharge pas sous les doigts de l'auteur.

## 3. Coût réel

| Projet | `stat` par tick (800 ms) |
|---|---|
| Démo `level_01` | 4 |
| Jeu de référence | 5 |
| Pire cas (manifeste plein, 256 assets) | ~260 |

Aucune I/O de dossier, aucun hash. Un `stat` par chemin, plus une IPC par tick.

## 4. Preuves

```text
cargo test --offline --workspace → 593 passed, 0 failed
cargo check --offline --workspace --tests → 0 warning
npm run build → OK · npm run lint → 14/0
npm run audit:ui -- --strict → 39 captures, 0 violation, dérive max 0,248 %
npm run check:versions → 0.4.4 partout
export → 3 niveaux, 24 entités, 5106 Ko, smoke 280,0 FPS
```

12 tests dédiés : 9 sur le veilleur (silence au premier passage, modification signalée
une fois, suppression, apparition, suspension, ordre déterministe, chemins dédupliqués)
et 3 sur le moteur (machine `false → true → false`, rechargement des entités et du joueur
sans panic, `.karacti` relu).

## 5. Limites inscrites

1. **Résolution de `mtime`** : un contenu modifié sans changer la date (outil qui préserve
   les timestamps) n'est pas vu ; un `touch` rechargera même à contenu identique. Choix
   sans dépendance, assumé.
2. **Fenêtre de 0–800 ms** après ouverture : la première passe est silencieuse.
3. **Progression runtime** : `reload()` reconstruit la scène — inventaire et contexte KVL
   réinitialisés (comportement existant). Un rechargement de projet conserve le niveau et
   le mode, pas la progression.
4. **Projets sans chemin disque** (paquet importé, projet vierge) : scène/scripts/assets
   surveillés, pas de fichier projet à relire.
5. **Écriture en cours** : un JSON tronqué fait échouer le rechargement proprement ;
   il faut une nouvelle écriture pour réessayer.

## 6. Clôture de P2

| Lot | État |
|---|---|
| P2.1 manifeste d'assets | ✅ |
| P2.2 import Tiled + tilesets | ✅ |
| P2.3 audio par fichiers (WAV) | ✅ |
| P2.4 rechargement à chaud | ✅ |

## 7. Validation

- [x] Gates vertes (§4)
- [x] Aucune dépendance ajoutée
- [x] Le Studio ne recharge pas pendant une saisie
- [x] Limites inscrites (§5)
- [ ] Tag `v0.4.4` créé par un humain
