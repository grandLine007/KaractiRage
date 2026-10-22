#!/usr/bin/env node
// Wrapper KaractiRage : exécute le CLI pxl depuis sa build `dist/` (JS pur),
// sans la garde "Node >= 22.18" du `bin/pxl.js`. Cela permet de générer les
// assets pixel-art sur Node 22.14 (la garde ne concerne que le type-stripping
// des sources TS en direct, dont on n'a pas besoin une fois dist/ construit).
import { main } from "./dist/node/cli.js";
process.exitCode = await main(process.argv.slice(2));
