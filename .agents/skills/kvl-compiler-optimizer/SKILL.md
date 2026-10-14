---
name: kvl-compiler-optimizer
description: >-
  Visual scripting engine, AST compiler, cycle detection, and runtime execution optimizations for KVL (Karacti Visual Logic).
  Use when extending KVL nodes (Event, Action, Condition, Math), optimizing the Interpreter execution pipeline,
  adding visual wiring capabilities, validating graph integrity, or managing data-driven JSON serialization.
---

# KVL Compiler & Runtime Optimizer Skill

Ce skill définit les standards techniques et algorithmiques de **KVL (Karacti Visual Logic)**, le moteur de programmation visuelle natif de KaractiRage.

---

## 1. Modèle du Graphe KVL

Un script KVL est un **graphe dirigé** représenté par :
1. **Nodes (`Node`)** :
   * `kind` : `Event` (point d'entrée), `Action` (effet sur le monde), `Condition` (branchement booléen), `Math` (opérations).
   * `inputs` / `outputs` : Broches typées (`Trigger`, `Int`, `Float`, `Bool`, `String`).
2. **Connections (`Connection`)** :
   * Liaison orientée entre `(from_node, from_pin)` et `(to_node, to_pin)`.
   * Invariant : un pin d'entrée ne peut recevoir qu'une seule source d'exécution Trigger active.
3. **Variables (`VariableDefinition`)** :
   * Variables locales ou globales partagées dans l'`ExecutionContext`.

---

## 2. Pipeline d'Exécution & Détection de Cycles

### A. Déclenchement d'Événements (`trigger_event`)
1. L'interpréteur recherche tous les nœuds de type `NodeKind::Event` correspondant au déclencheur (ex: `OnPlayerOverlap`, `OnDoorInteract`, `OnPlayerSeen`).
2. Pour chaque nœud trouvé, il suit le pin de sortie d'exécution `out_trigger`.

### B. Protection Anti-Boucle Infinie
* Chaque passe d'exécution suit un compteur `ctx.steps_count`.
* Si `steps_count > ctx.max_steps` (par défaut 1 000 étapes), une erreur `RuntimeError::MaxStepsExceeded` est levée immédiatement.
* Cela empêche tout freeze de l'application en cas de boucle cyclique créée par l'utilisateur dans l'éditeur.

### C. Branchement Conditionnel
* Tout nœud de condition (`NodeKind::Condition`) évalue son prédicat et route le flux vers :
  * `out_true` si la condition est validée.
  * `out_false` si la condition n'est pas remplie.
* Chaque étape est journalisée dans `ctx.execution_log` pour permettre l'audit en direct dans l'interface HUD.

---

## 3. Règles de Sérialisation & Hot-Reloading

* Les scripts KVL sont persistés au format JSON standard (`assets/scripts/*.kvl.json`).
* Les positions graphiques UI (`position_x`, `position_y`, `color_tag`) sont conservées pour préserver l'agencement visuel défini par le concepteur dans l'inspecteur.
* Lors de la modification à chaud d'un script ou de la sauvegarde (`Ctrl+S`), le graphe est re-parsé et réinjecté sans redémarrer le binaire.
