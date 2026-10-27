# ADR-0002 — Système de plugins du moteur KaractiRage

- **Statut** : accepté pour le socle v1
- **Date** : 2026-10-26
- **Décideurs** : équipe moteur
- **ADR liés** : ADR-0001 (renderer 3D, voir `rapports/rapport_48_moteur_3d_architecture.md`)

## Contexte

KaractiRage est un moteur 2D/3D data-driven : scènes JSON, graphes KVL, recettes de genre,
export HTML autonome et export wasm. Trois contraintes rendent un système de plugins
nécessaire mais délicat :

1. **Aucun point d'extension n'existe.** Le workspace ne définit aucun trait d'extension.
   Les actions KVL sont exécutées par un `match` monolithique sur des chaînes
   (`crates/kvl-runtime/src/interpreter.rs`), les nœuds sont décrits à quatre endroits
   (runtime, palette native, formulaire Studio, prompt IA).
2. **La boucle est déterministe** : pas de simulation fixe (120 Hz), replay, budget
   60 FPS sur bas de gamme. Un plugin ne doit ni allouer dans le chemin chaud, ni casser
   la reproductibilité.
3. **Deux cibles de compilation** : natif (Tauri) et `wasm32-unknown-unknown`. Un plugin
   qui charge une bibliothèque dynamique est incompatible avec la cible web.

## Décision

### 1. Trois étages d'extension, pas un

| Étage | Format | Exécution | Confiance | Cible |
|---|---|---|---|---|
| **Pack de contenu** | dossier `.kpack` : `plugin.json` + scènes / graphes KVL / prefabs / tilesets | données validées, aucun code | non fiable | natif **et** wasm |
| **Node pack** | crate Rust compilée, schémas + handlers enregistrés au démarrage | code moteur | fiable | natif **et** wasm |
| **Extension native** | crate Rust feature-gated (systèmes : physique, importeurs, rendu) | code moteur | fiable | natif |
| **Plugin WASM** | module + manifeste, hôte wasmtime | sandbox | non fiable | natif desktop uniquement |

Le wasm **n'est pas** dans le socle v1 : il ne fonctionne pas dans le build navigateur et
ajoute une latence d'appel incompatible avec le budget par frame tant que le besoin de
modding n'est pas prouvé.

### 2. Un seul catalogue de nœuds, quatre consommateurs

`kvl_core::NodeSchema` devient l'unique description d'un type de nœud :
identifiant, flavor (event/condition/action/composite), libellé, catégorie, description,
paramètres typés, préfixes acceptés, entrées/sorties de flux. Le `NodeRegistry` le stocke
et résout les types préfixés (`DamagePlayer:2` → schéma `DamagePlayer`, argument `2`).

Consommateurs : interpréteur (exécution), palette native, éditeur Studio, prompt IA.
Un type inconnu est signalé par `validate_graph`, jamais silencieusement ignoré.

### 3. Les handlers sont des fonctions, pas des objets

`HandlerRegistry` associe un identifiant de type à un `fn(&mut ExecutionContext, &NodeCall)`
(pointeur de fonction). Pas de `dyn Plugin` dans le chemin chaud, pas d'allocation par
action, pas de verrou global. Le registre est construit une fois au chargement, puis
consultable en lecture seule.

L'interpréteur consulte le registre **avant** son `match` historique. Les actions non
migrées continuent de fonctionner (repli), ce qui permet une migration incrémentale sans
Big Bang.

### 4. Manifeste versionné et capacités déclaratives

`PluginManifest` : `id`, `name`, `version`, `api`, `kind`, `capabilities`,
`deterministic`, `budget_us`, `dependencies`, `content`, `enabled`.

- `api` est l'API hôte ; un plugin qui demande une version supérieure est refusé.
- Une capacité non déclarée ne peut pas être utilisée (`content-read`, `filesystem`,
  `network`, `audio`, `input`, `render`, `editor`).
- Les plugins runtime doivent être `deterministic: true` ; un budget par frame est
  déclaré et mesuré (télémétrie à venir).
- Les chemins de contenu sont relatifs, sans `..`, sans absolu : un pack ne peut pas
  sortir de son dossier.
- ID uniques, dépendances résolues, ordre d'application trié par ID (déterministe).

### 5. Intégration au moteur

`GameState` porte un `PluginHost` construit avec les nœuds cœur et chargé depuis
`plugins/` (puis `../../plugins/`). Le registre de handlers est passé à l'interpréteur
via `Interpreter::trigger_event_with`. Le catalogue (plugins + nœuds + diagnostics) est
exposé en JSON pour le Studio.

## Alternatives rejetées

- **Dylib + ABI stable (`abi_stable`)** : Rust n'a pas d'ABI stable ; taxe permanente,
  incompatible wasm, pour un bénéfice que le wasm apportera plus tard.
- **Scripting embarqué (Lua/Rhai/Wren)** : KVL est déjà la couche script ; ajouter un
  second langage double la surface pédagogique et le budget mémoire.
- **WASM d'emblée** : pas de wasmtime dans le navigateur, latence d'appel, complexité
  de sérialisation des commandes ; aucun besoin utilisateur prouvé.
- **API unique exposant `&mut GameState`** : impossible à versionner, à sandboxer et à
  tester ; préférer des registres étroits et des commandes.
- **Marketplace avant trois plugins de référence** : vitrine vide.

## Conséquences

- **Positives** : ajout d'un nœud sans toucher l'interpréteur ; validation des graphes
  avant exécution ; packs de contenu portables et sûrs ; base saine pour le modding.
- **Négatives** : le registre duplique temporairement l'information tant que la palette
  native et le Studio ne consomment pas `NodeSchema` ; dette de migration des ~60 actions
  restantes ; l'API v1 ne couvre que les actions (conditions et événements ensuite).

## Mise en œuvre (v1)

1. `kvl_core::schema` : `NodeSchema`, `NodeRegistry`, validation de graphe.
2. `kvl_runtime::actions` + `handlers` : registre de handlers, trois actions migrées
   (`AddScore`, `DamagePlayer`, `GiveItem`), repli historique conservé.
   **v1.1** : les 49 actions restantes ont été extraites vers `actions_batch.rs`
   (rapport 50) ; le catalogue compte 75 nœuds.
3. `engine_core::plugin` : manifeste, capacités, chargement `.kpack`, hôte, catalogue.
4. Pack de référence `plugins/karacti.pack.platformer`.
5. Tests unitaires et d'intégration, puis rapport 49.

## Références

- `crates/kvl-runtime/src/interpreter.rs` — dispatch historique.
- `crates/kvl-core/src/node.rs` — modèle de nœud.
- `crates/engine-core/src/game.rs` — `GameState`, boucle et intégration KVL.
- `rapports/rapport_48_moteur_3d_architecture.md` — précédent ADR et format de rapport.
