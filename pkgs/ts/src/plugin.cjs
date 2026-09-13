const {
  createLanguageServicePlugin,
} = require("@volar/typescript/lib/quickstart/createLanguageServicePlugin.js");
const { transform } = require("./transform.cjs");

/** @typedef {import("@volar/typescript").TypeScriptServiceScript} ServiceScript */

// tsserver requires a CommonJS factory. Always use its injected TypeScript.
module.exports = createLanguageServicePlugin((ts) => ({
  languagePlugins: [
    {
      getLanguageId(fileName) {
        if (
          fileName.endsWith(".ts") &&
          !fileName.endsWith(".d.ts") &&
          !/[\\/]node_modules[\\/]/.test(fileName)
        ) {
          return "errorscript";
        }
      },
      createVirtualCode(_fileName, languageId, snapshot) {
        if (languageId !== "errorscript") return;
        const { text, mappings } = transform(snapshot.getText(0, snapshot.getLength()));
        return {
          id: "typescript",
          languageId: "typescript",
          snapshot: ts.ScriptSnapshot.fromString(text),
          mappings: mappings.map(([generatedStart, length, sourceStart]) => ({
            sourceOffsets: [sourceStart],
            generatedOffsets: [generatedStart],
            lengths: [length],
            data: {
              verification: true,
              completion: true,
              semantic: true,
              navigation: true,
              structure: true,
            },
          })),
        };
      },
      typescript: {
        extraFileExtensions: [],
        /** @returns {ServiceScript} */
        getServiceScript(code) {
          return {
            code,
            extension: ".ts",
            scriptKind: ts.ScriptKind.TS,
            preventLeadingOffset: true,
          };
        },
      },
    },
  ],
}));
