# Plugins KaractiRage — guide du socle v1.1

> Statut : socle expérimental (API 1), actions KVL entièrement migrées.
> Décision d'architecture : [ADR-0002](adr/ADR-0002-systeme-de-plugins.md).

Un **plugin** est un dossier contenant un `plugin.json`. Trois natures coexistent :

| Nature (`kind`) | Contenu | Code exécuté | Cible |
|---|---|---|---|
| `content` | scènes, graphes KVL, prefabs, tilesets | aucun | natif + wasm |
| `node-pack` | schémas de nœuds + handlers compilés | oui (compilé) | natif + wasm |
| `native` | systèmes, importeurs, rendu | oui (compilé) | natif |

En v1, **aucun code tiers n'est chargé**. Un pack de contenu est une donnée validée ; un
node pack est une crate compilée avec le moteur. Le wasm (wasmtime) est réservé à une
version ultérieure.

## Arborescence d'un pack

```
plugins/karacti.pack.platformer/
├── plugin.json                  # manifeste obligatoire
├── README.md                    # optionnel
└── graphs/
    └── bonus_coin.kvl.json      # contenu déclaré
```

Le dossier est scanné par `PluginHost::load_dir` : soit `plugin.json` à la racine, soit un
sous-dossier par pack. L'ordre d'application est trié par identifiant puis résolu par
dépendances ; il est exposé dans `load_order`.

## Manifeste `plugin.json`

```json
{
  "id": "karacti.pack.platformer",
  "name": "Pack Plateforme — bonus de pièce",
  "version": "0.1.0",
  "api": 1,
  "kind": "content",
  "description": "…",
  "capabilities": ["content-read"],
  "deterministic": true,
  "budget_us": 0,
  "dependencies": [],
  "enabled": true,
  "content": {
    "graphs": ["graphs/bonus_coin.kvl.json"]
  }
}
```

| Champ | Règle |
|---|---|
| `id` | unique, minuscules/chiffres/`.`/`_`/`-`, 64 caractères max, pas de `..` |
| `version` | `x.y.z` numérique |
| `api` | obligatoire ; `> 1` refusé, `0` refusé |
| `kind` | `content`, `node-pack`, `native` |
| `capabilities` | `content-read`, `filesystem`, `network`, `audio`, `input`, `render`, `editor` |
| `deterministic` | obligatoire pour `node-pack` et `native` |
| `budget_us` | ≤ 5 000 µs ; un pack de contenu doit rester à 0 |
| `dependencies` | identifiants de plugins, cycle interdit |
| `content` | chemins **relatifs** au pack, sans `..`, sans absolu |

Un pack `content` ne peut déclarer que `content-read` : il ne peut ni écrire, ni accéder au
réseau, ni déclencher de l'audio. Un chemin qui sort du pack est refusé au chargement.

## Catalogue de nœuds

Chaque type de nœud est décrit une seule fois par un `NodeSchema` (`kvl_core::schema`) :

```json
{
  "id": "DamagePlayer",
  "flavor": "action",
  "label": "Infliger des dégâts",
  "category": "Combat",
  "description": "…",
  "params": [
    { "name": "amount", "label": "Dégâts", "param_type": { "kind": "int" }, "default": { "type": "Int", "value": 1 } }
  ],
  "aliases": ["ChasePlayer"],
  "prefixes": ["DamagePlayer:"],
  "default_argument": "1",
  "flow_input": true,
  "flow_outputs": ["out_trigger"],
  "plugin": "karacti.nodes.core"
}
```

Le registre résout les types **préfixés** : `DamagePlayer:2` → schéma `DamagePlayer`,
argument `"2"`. Le préfixe le plus long gagne (`A:B:` avant `A:`). Un **alias** résout un
ancien identifiant vers le schéma canonique (`ChasePlayer` → `NavChasePlayer`), et
`default_argument` fournit la forme proposée par la palette (`SetVar:` → `score:0`).

Le catalogue compte **75 schémas** : 50 actions et 25 événements. Toutes les actions de
l'interpréteur historique sont migrées ; les conditions restent pour l'instant dans
l'interpréteur.

`NodeRegistry::validate_graph` signale :

- un type inconnu du catalogue ;
- un nœud dont la flavor ne correspond pas (une action qui référence une condition).

Ces diagnostics sont remontés par l'hôte (`PluginIssueKind::UnknownNodeType`) et exposés
au Studio, sans bloquer le chargement du pack.

## Ajouter un nœud (v1.1, compilé)

1. **Schéma** — dans `crates/kvl-runtime/src/actions.rs` (ou `actions_batch.rs` pour une
   action historique), ajoutez un `NodeSchema` à `builtin_node_schemas()` : catégorie,
   description, paramètres, préfixe, alias et `default_argument` éventuels.
2. **Handler** — une fonction `fn(&mut ExecutionContext, &NodeCall) -> Result<(), RuntimeError>`
   qui ne fait que lire/écrire le contexte (variables, journal). Pas d'accès au moteur.
3. **Enregistrement** — dans `builtin_handlers()` : `register("MonAction", mon_handler)` ou
   `register_prefix("MonAction:", mon_handler)`.
4. **Test** — un cas dans `actions.rs` (variables + journal) et, si le nœud est préfixé, un
   cas `MonAction:42`. Le test `every_action_in_asset_graphs_is_catalogued_and_handled`
   vérifie automatiquement que tout type utilisé dans `assets/scripts` a un schéma **et**
   un handler.

`actions_batch.rs` contient les 49 actions extraites du `match` historique : le code est
volontairement proche de l'original (comportement identique), à découper par catégorie
lors d'un prochain passage.

## Utiliser l'hôte dans le moteur

```rust
let mut host = PluginHost::with_builtins(); // pack cœur : schémas + handlers
let report = host.load_workspace();         // plugins/ puis ../../plugins/
for issue in &report.issues {
    tracing::warn!(plugin = %issue.plugin, "{}", issue.message);
}

// Exécution KVL avec les handlers des plugins :
Interpreter::trigger_event_with(graph, &mut ctx, "OnPlayerOverlap", host.handlers());

// Catalogue pour le Studio :
let json = host.catalog_json()?;
```

`GameState` porte déjà un `PluginHost` et l'utilise pour tous les déclenchements KVL ;
le Studio peut récupérer le catalogue via la commande Tauri `engine_plugin_catalog`.

## Palette du Studio

L'onglet **Nœuds** de l'éditeur KVL charge `engine_plugin_catalog` et affiche les schémas
groupés par catégorie, avec recherche par identifiant, libellé ou description. Un clic :

1. appelle `PluginHost::instantiate` pour obtenir le type concret
   (`SetVar` → `SetVar:score:0`, `PlaySound` → `PlaySound:coin`) ;
2. insère le nœud avec ses pins de flux (`in_trigger`, `out_trigger`, `out_true/false`) ;
3. persiste le graphe et rafraîchit la validation.

L'insertion ne crée pas encore de connexions : l'utilisateur câble les pins dans l'éditeur.

## Garanties et limites v1.1

**Garanties**

- Aucune allocation par dispatch : le registre est construit une fois.
- Ordre déterministe : `BTreeMap`, tri des préfixes par longueur, `load_order` stable.
- Chemins de contenu confinés au pack ; manifeste validé avant toute lecture.
- Un pack invalide ne bloque pas les autres (diagnostics par pack).

**Limites assumées**

- Les **conditions** ne sont pas encore branchées sur un registre (elles restent dans
  l'interpréteur), et les événements n'ont pas de handler (ils sont déclenchés par le moteur).
- L'insertion depuis la palette ne crée pas de connexions et ne propose pas encore de
  formulaire pour les paramètres : le type concret est matérialisé avec ses valeurs par
  défaut.
- Aucune exécution de code tiers : pas de wasm, pas de dylib.
- Les scènes et prefabs déclarés sont vérifiés (existence) mais pas encore parsés.
- Le panneau Extensions complet (activation/désactivation, diagnostics) reste à faire ;
  la palette consomme déjà le catalogue.

## Vérifier

```sh
cargo test -p kvl-core schema
cargo test -p kvl-runtime actions
cargo test -p kvl-runtime every_action_in_asset_graphs
cargo test -p engine-core plugin::
cargo test --workspace
```
