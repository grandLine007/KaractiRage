/// <reference types="vite/client" />

/**
 * Version de l'application, injectée à la compilation par Vite depuis `package.json`
 * (voir `vite.config.ts`). Ne jamais coder la version en dur dans un composant.
 */
declare const __APP_VERSION__: string;
