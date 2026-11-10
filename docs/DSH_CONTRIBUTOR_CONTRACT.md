# Contrat de contribution — DSH dans KaractiRage

> **Version** : proposition 1.1 — 10 novembre 2026 (D1 amendée, cf. §3.1)
> **Statut** : D1 tranchée le 10/11/2026 ; D2 à D8 restent à valider avant la mise en place
> des droits GitHub et le pilote de 10 tâches.
> **Décision structurante** : DSH est un **contributeur de niveau 2** (branche + commits +
> PR draft), **jamais mergeur**, au service de l'objectif final de KaractiRage.

---

## 0. Pourquoi ce contrat existe

« Rendre DSH contributeur Git » peut signifier :

- lire le dépôt ;
- proposer un patch ;
- créer une branche ;
- committer ;
- pousser ;
- ouvrir une PR ;
- répondre aux revues ;
- merger ;
- publier une release.

Chaque niveau ajoute un risque. Sans contrat, on donne implicitement à un agent
expérimental un droit qu'on n'accorderait pas à un junior : merger, publier, toucher aux
secrets, modifier la CI ou réécrire l'historique.

Le contrat définit **ce que DSH peut faire**, **ce qu'il doit prouver**, et **ce qui reste
humain**.

---

## 1. Les deux objectifs

### 1.1 Objectif final du projet (la boussole)

D'après [`OBJECTIF_FINAL_KARACTIRAGE.md`](../OBJECTIF_FINAL_KARACTIRAGE.md) :

> KaractiRage doit permettre à tout créateur, sur une machine modeste, de concevoir,
> prototyper et publier un jeu de haute tenue grâce à KVL, des agents IA contrôlés et une
> architecture native déterministe.

Quatre piliers non négociables :

1. **Accessibilité matérielle** : low-end first, 60 FPS, budget mémoire strict.
2. **Continuum KVL** : guidé → visuel → expert, un seul modèle de données.
3. **IA contrôlée** : plan → patch → test → rollback, validation humaine.
4. **Runtime souverain** : jeu exporté déterministe, sans IA ni cloud obligatoire.

Le document est explicite : **« créer un jeu AAA complet en un prompt sans rien faire » est
une illusion**. L'IA assiste, accélère et structure ; l'humain garde le contrôle, la
direction artistique et la validation.

### 1.2 Objectif final de DSH (le rôle)

> **DSH est un contributeur technique de niveau 2 : il transforme une tâche spécifiée en
> une branche, des commits conventionnels et une PR draft prouvée, dans un périmètre
> autorisé, sans jamais merger, publier, réécrire l'historique ni contourner les
> garde-fous de KaractiRage.**

Sa réussite ne se mesure pas au nombre de lignes produites ni à son autonomie, mais à :

- la **qualité** des changements (tests, gates, absence de régression) ;
- le **respect** de l'architecture et des budgets ;
- la **traçabilité** de chaque décision ;
- le **temps humain économisé** (revue, pas réparation).

### 1.3 Ce que DSH n'est pas

- pas un mainteneur ;
- pas un propriétaire de release ;
- pas une source de vérité sur le schéma KVL ou le dictionnaire IA ;
- pas un agent qui écrit directement les scènes/KVL sans validation ;
- pas un remplaçant du pipeline Rust `AiOp` / `AiEdit` ;
- pas une raison de contourner la revue humaine.

---

## 2. Niveaux de contribution

| Niveau | Droits | Usage |
|---|---|---|
| **L0 — Lecteur** | lire, analyser, proposer un patch en suggestion | audit, revue, documentation |
| **L1 — Proposal** | branche locale, patch non poussé | expérimentation |
| **L2 — Contributeur** | branche distante + commits + PR draft | **cible DSH** |
| **L3 — Mainteneur** | merge, release, tags | **interdit à DSH** |
| **L4 — Propriétaire** | administration, secrets, facturation | **interdit** |

**Décision recommandée : L2, plafonné.** Même après plusieurs mois, DSH ne merge pas ;
seul un humain merge.

---

## 3. Identité et attribution Git

### 3.1 Identité dédiée

Les commits de l'agent ne portent pas l'identité personnelle `husseir <irahusse@gmail.com>`.
Ils portent une identité d'agent distincte :

```text
DSH Agent <dsh-agent@karactirage.local>
```

**Décision D1 (10 novembre 2026) : compte personnel + trailers.** Le push s'effectue avec un
token fine-grained du compte propriétaire ; aucun machine user `karacti-dsh` n'est créé à ce
stade. Le bot n'existe donc pas comme compte GitHub : il est identifié par l'auteur du commit
et par les trailers `Agent:` / `Task:` (§3.2).

| Option | Avantages | Inconvénients | Statut |
|---|---|---|---|
| **Machine user** (`karacti-dsh`) + accès Write | simple, PR/commits normaux, audit clair | compte supplémentaire, token à gérer | **écarté pour l'instant** |
| **GitHub App** | permissions fines, installation contrôlée | mise en place plus lourde | écarté |
| **Compte personnel + trailers** | zéro compte, mise en route immédiate | attribution non vérifiée côté GitHub, droits trop larges | **retenu** |

Limites assumées de D1 :

- côté GitHub, `DSH Agent` apparaît comme un auteur **non vérifié**, rattaché à aucun compte ;
- le token hérite des droits du compte propriétaire (**admin** sur ce dépôt) : le périmètre
  du §4 est une discipline, pas un garde-fou technique ;
- la révocation est globale (révoquer le token), pas sélective par permission ;
- l'audit log GitHub attribue les push au compte propriétaire, pas à l'agent.

**Réexamen obligatoire** si l'une des conditions suivantes survient : un incident de
permission (§8), un push non autorisé constaté, ou l'échec des cibles du pilote. Dans ce cas,
réouvrir D1 et basculer sur un machine user.

### 3.2 Trailer de commit obligatoire

C'est le **seul** mécanisme d'identification retenu par D1 : sans `Agent:` et `Task:`, aucun
commit de l'agent n'est distinguable des commits humains, et l'audit devient impossible.

Chaque commit de DSH porte :

```text
Agent: dsh/0.2.2-rc.1
Task: KAR-123
```

Optionnel :

```text
Co-authored-by: husseir <irahusse@gmail.com>
Run-ID: <identifiant de session>
```

Cela permet de retrouver les commits, de les filtrer et de les auditer.

### 3.3 Signature

- clé SSH de signature portée par le compte propriétaire (D1 n'introduit pas de compte bot
  distinct, donc la clé ne peut pas être isolée par compte) ;
- commits signés si GitHub le permet ;
- sinon, `Agent:` + PR suffisent pour la v1, mais la signature est recommandée.

### 3.4 Espace de travail isolé

Pour ne pas changer la config Git de l'utilisateur :

- DSH travaille dans un **clone dédié** ou un worktree dédié ;
- `user.name` / `user.email` configurés **localement** dans ce clone ;
- jamais de `git config --global`.

---

## 4. Périmètre des droits

### 4.1 Autorisé sans demande supplémentaire

- `crates/**` (code moteur, tests) ;
- `apps/studio/src/**` et `apps/studio/src-tauri/src/**` ;
- `docs/**`, `ai/**`, `dist/ai/**` (généré) ;
- `scripts/**`, tests, benchmarks ;
- corrections de bugs bornées, tests manquants, documentation.

### 4.2 Nécessite une approbation humaine explicite

- ajout/suppression de dépendance (`Cargo.toml`, `Cargo.lock`, `package.json`, lockfiles) ;
- modification de `plugins/**` ou du chargement de plugins ;
- modification de `KARACTI_AI_SPEC.md`, `OBJECTIF_FINAL_KARACTIRAGE.md`,
  `VISION_CIBLE_VERSION_FINALE.md` (documents d'autorité) ;
- modification de la CI, des workflows, des scripts de release ;
- changement de schéma de données ou de format de fichier ;
- PR > 10 fichiers ou > 400 lignes ajoutées (seuil souple, à justifier).

### 4.3 Interdit

- push direct sur `main` ou sur la branche d'intégration ;
- `git push --force`, `git rebase` d'une branche partagée, suppression de branche ;
- tags, releases, version bump, publication npm/crates ;
- lecture/écriture de secrets, `.env`, `karactirage.ai.json` (clé API), `~/.dsh` ;
- modification des règles de protection de branche ;
- exécution de commandes destructrices hors projet ;
- auto-merge, auto-approve, contournement des checks ;
- génération de code three.js/TSL ou de scripts arbitraires dans le jeu.

---

## 5. Workflow Git

### 5.1 Entrée

Une tâche doit avoir :

- un identifiant (`KAR-123` ou issue GitHub) ;
- un objectif en une phrase ;
- des critères d'acceptation vérifiables ;
- un périmètre de fichiers ;
- un niveau de risque (`safe`, `sensible`, `critique`) ;
- si nécessaire, une maquette ou un plan validé.

Sans cela, DSH peut analyser mais **ne code pas**.

### 5.2 Branche

```text
agent/dsh/<task-id>-<slug>
```

Exemple : `agent/dsh/kar-123-fix-kvl-pin-types`.

Base :

- `feat/tauri-react-studio` pour les travaux Studio/UX en cours ;
- `main` pour les correctifs indépendants, après validation humaine.

Une branche = une tâche = une PR.

### 5.3 Commits

- petits, cohérents, un sujet par commit ;
- convention existante : `fix(engine): …`, `feat(studio): …`, `test(ai): …`,
  `docs(ai): …` ;
- message en français, explicite ;
- trailers `Agent:` et `Task:` ;
- pas de commit de fixtures générées, `target/`, `node_modules/`, `.tools/`.

### 5.4 Gate local avant push

```bash
cargo test --workspace
cargo check --workspace
cd apps/studio
npm run build
npm run lint            # 0 erreur ; ne pas ajouter de warning
npm run audit:ui -- --strict
```

Selon le changement :

- `cargo test -p engine-core ai_dictionary` + vérification `dist/ai` ;
- benchmark si chemin chaud ;
- capture HUD/3D si UI ;
- test de non-régression KVL.

### 5.5 PR

- **draft** à l'ouverture ;
- base et head explicites ;
- description : objectif, changements, tests, preuves, risques, rollback ;
- captures pour l'UI ;
- labels : `agent:dsh`, `risk:*`, `area:*` ;
- jamais de merge par DSH ;
- DSH répond aux commentaires, corrige, re-push (sans force push).

**État constaté le 10/11/2026** : le dépôt n'utilise pas encore de PR. `feat/ux-consolidation`
est **64 commits d'avance** sur `feat/tauri-react-studio`, et tout y est empilé par merges
locaux. Une PR dans ce sens serait un merge de consolidation, pas une PR de tâche : elle
contredirait le §5.5. Conséquence : le canal PR (template, labels, CI) doit être installé
**avant** le pilote, et chaque tâche du pilote partira d'une branche `agent/dsh/…` créée
depuis la base validée, pas depuis une branche de consolidation déjà empilée.

### 5.6 Revue et merge

- un humain approuve ;
- checks requis verts ;
- merge par un humain ;
- branche supprimée après merge ;
- DSH ne touche pas à la branche après merge.

---

## 6. Definition of Done

Une contribution DSH est terminée seulement si :

- [ ] la tâche avait des critères d'acceptation explicites ;
- [ ] la branche respecte la convention ;
- [ ] les commits sont conventionnels et tracés (`Agent:`, `Task:`) ;
- [ ] les tests couvrent le changement (ou expliquent pourquoi non) ;
- [ ] `cargo test --workspace` passe ;
- [ ] `cargo check --workspace` passe ;
- [ ] `npm run build` passe ;
- [ ] `npm run lint` ne ajoute aucune erreur/warning ;
- [ ] `npm run audit:ui -- --strict` passe si UI ;
- [ ] aucune régression de performance/budget si chemin chaud ;
- [ ] la documentation est mise à jour si le comportement change ;
- [ ] la PR contient les preuves (commandes, sorties, captures) ;
- [ ] aucun secret, artefact ou fichier généré n'est commité ;
- [ ] la PR est en draft et attend une revue humaine ;
- [ ] DSH n'a ni mergé, ni taggé, ni publié.

---

## 7. Ce que DSH peut apporter en priorité

### P0 — immédiat, faible risque

1. **Documentation** : `AGENTS.md`, guides, rapports, mise à jour des plans.
2. **Tests** : tests manquants, régression, couverture KVL/AI.
3. **Petits bugs** : corrections bornées avec test.
4. **Maintenance** : formatage, liens morts, cohérence des commandes.
5. **Revue assistée** : analyse de diff, tickets, audits.

### P1 — après calibration

6. **Fonctionnalités Studio** : composants, raccourcis, accessibilité.
7. **Moteur** : correctifs ciblés, validation, benchmarks.
8. **IA/KVL** : ops, validation, dictionnaire, exemples.
9. **3D** : assets, matériaux, caméra, validation.

### P2 — plus tard, avec plan humain

10. **Macro-génération de jeu** via MCP/ACP.
11. **CI/CD** (workflow, release) : sensible, revue humaine obligatoire.
12. **Refactors transverses** : plan et découpage préalables.

---

## 8. Métriques de réussite

Pilote de 10 tâches sur 2 semaines :

- 3 docs/tests, 4 bugs ciblés, 3 petites fonctionnalités ;
- mesurer :
  - taux d'acceptation sans réécriture ;
  - nombre de cycles de revue ;
  - taux de gates vertes au premier push ;
  - régressions échappées ;
  - temps humain de revue ;
  - taille moyenne de PR ;
  - incidents de permission.

Cibles :

| Métrique | Cible |
|---|---|
| PR acceptées | ≥ 80 % |
| Cycles de revue | ≤ 2 |
| Gates vertes au 1er push | ≥ 90 % |
| Régressions échappées | 0 |
| Actions non autorisées | 0 |
| PR > 400 lignes | ≤ 10 % |
| Secrets/artefacts commités | 0 |

Si les cibles ne sont pas tenues après le pilote : **rétrograder DSH en L1** (propositions
non poussées) jusqu'à correction.

---

## 9. Sécurité et révocation

- token fine-grained du compte stocké dans le **trousseau macOS**
  (`security add-generic-password -a dsh-agent -s karactirage-pat -w`), jamais dans un fichier
  du projet, jamais dans l'URL du remote, jamais dans l'historique de commande ;
- un token collé en clair (conversation, ticket, log, capture) est **révoqué immédiatement**
  et remplacé : il est considéré comme public dès qu'il a été affiché ;
- périmètre du token : *Contents: Read and write* + *Pull requests: Read and write*, rien
  d'autre — en particulier **aucune permission Administration**, quel que soit le compte ;
- permissions minimales : Write sur le dépôt, pas d'admin, pas de bypass ;
- branches `main` et d'intégration protégées : PR obligatoire, review, checks, no force
  push, no deletion ;
- DSH ne peut pas modifier les workflows sans approbation ;
- journalisation : commits `Agent:`, PR, runs ;
- révocation : retirer le collaborateur/token, révoquer la clé, archiver les branches ;
- audit trimestriel des contributions.

---

## 10. Décisions à valider

1. **Identité** : ✅ **tranchée le 10/11/2026 — compte personnel + trailers `Agent:`/`Task:`**
   (§3.1). Machine user et GitHub App écartés pour l'instant ; réexamen prévu en cas
   d'incident ou d'échec du pilote.
2. **Droits** : L2 (branche + PR draft, pas de merge) — recommandé, **à valider**.
3. **Base** : `feat/tauri-react-studio` pour l'instant, `main` plus tard ?
4. **Périmètre** : code/tests/docs/ai ; CI/release/deps sur approbation ?
5. **Revue** : un humain approuve chaque PR, aucun auto-merge.
6. **Taille** : 10 fichiers / 400 lignes par PR (seuil souple).
7. **CI** : créer un workflow GitHub Actions avant le pilote, ou gates locales seulement ?
8. **Espace** : clone dédié pour DSH (recommandé) ou worktree partagé ?

---

## 11. Mise en place (après validation)

- [x] Trancher l'identité : compte personnel + trailers `Agent:`/`Task:` (D1, 10/11/2026).
- [x] Créer un token fine-grained et le stocker au trousseau macOS.
- [ ] Restreindre le token à *Contents* + *Pull requests* et retirer toute permission admin.
- [ ] Protéger `main` et la branche d'intégration (`feat/tauri-react-studio`).
- [ ] Créer `AGENTS.md` (commandes, invariants, checklist).
- [ ] Créer le clone/worktree dédié + config Git locale.
- [ ] Ajouter le template de PR et les labels.
- [ ] Ajouter un workflow CI (si décision 7 = oui).
- [ ] Lancer le pilote de 10 tâches.
- [ ] Décider L2 permanent ou rétrogradation.

---

## 12. Règle finale

> **DSH contribue au projet, il ne possède pas le projet.**
> L'agent propose, les gates prouvent, l'humain merge.
