// Keep an ESM entry for the content mapper; tsserver loads the shared code via CJS.
export { transform } from "./transform.cjs";
