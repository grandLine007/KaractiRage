# Pack Plateforme — bonus de pièce

Pack de **contenu** de référence pour le système de plugins de KaractiRage.

- `plugin.json` : manifeste (API 1, type `content`, capacité `content-read`).
- `graphs/bonus_coin.kvl.json` : graphe KVL `OnPlayerOverlap → AddScore → GiveItem:coin:2`.

## Pourquoi ce pack existe

Il sert de test d'intégration réel : il est chargé par `PluginHost::load_workspace`, son
graphe est parsé, puis chaque type de nœud est résolu contre le catalogue (`AddScore`,
`GiveItem:`). Il n'exécute aucun code et ne déclare aucune capacité dangereuse.

## Vérifier

```sh
cargo test -p engine-core plugin::tests::reference_pack_loads_from_repository
```
