import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { pathToFileURL } from "node:url";
import { API } from "typescript/unstable/async";
import { TextDocument } from "vscode-languageserver-textdocument";
import { transform } from "./transform.js";

/** @typedef {import("typescript/unstable/async").Diagnostic} TSDiagnostic */
/** @typedef {ReturnType<typeof transform> & { original: string }} VirtualFile */

/** @param {string} fileName */
function findConfig(fileName) {
  let directory = dirname(fileName);
  while (true) {
    const config = join(directory, "tsconfig.json");
    if (existsSync(config)) return config;
    const parent = dirname(directory);
    if (parent === directory) return undefined;
    directory = parent;
  }
}

/** @param {unknown} raw */
function isEnabled(raw) {
  // This wrapper interprets the entry; the native compiler does not load it.
  const config = /** @type {{ compilerOptions?: { plugins?: { name?: string }[] } }} */ (raw);
  const plugins = config?.compilerOptions?.plugins;
  return Array.isArray(plugins) && plugins.some((plugin) => plugin?.name === "@errorscript/ts");
}

/**
 * A fresh API program per operation keeps this PoC's unsaved-buffer handling
 * simple. All open buffers (including imported files) override filesystem reads.
 * @template T
 * @param {string} fileName
 * @param {Map<string, string>} buffers
 * @param {(project: {
 *   program: import("typescript/unstable/async").Program,
 *   files: Map<string, VirtualFile>,
 *   configErrors: readonly TSDiagnostic[],
 *   configFile: string | undefined,
 * }) => Promise<T>} action
 */
export async function withEditorProject(fileName, buffers, action) {
  const configFile = findConfig(fileName);
  let enabled = !configFile;
  /** @type {Map<string, VirtualFile>} */
  const files = new Map();
  const api = new API({
    cwd: dirname(configFile ?? fileName),
    fs: {
      fileExists: (path) => (buffers.has(path) ? true : undefined),
      readFile(path) {
        let original = buffers.get(path);
        const source =
          path.endsWith(".ts") && !path.endsWith(".d.ts") && !/[\\/]node_modules[\\/]/.test(path);
        if (original === undefined && !source) return undefined;
        if (original === undefined) {
          try {
            original = readFileSync(path, "utf8");
          } catch (error) {
            if (/** @type {NodeJS.ErrnoException} */ (error).code === "ENOENT") return null;
            throw error;
          }
        }
        const output =
          enabled && source
            ? transform(original)
            : {
                text: original,
                extension: ".ts",
                mappings: /** @type {VirtualFile["mappings"]} */ ([
                  [0, original.length, 0, original.length, 0],
                ]),
              };
        files.set(path, { ...output, original });
        return output.text;
      },
    },
  });
  try {
    const config = configFile ? await api.parseConfigFile(configFile) : undefined;
    if (config) enabled = isEnabled(config.raw);
    const program = await api.createProgram(
      [...new Set([...(config?.fileNames ?? []), fileName])],
      {
        compilerOptions: {
          ...(config?.options ?? { strict: true, moduleDetection: 3, types: [] }),
          noEmit: true,
        },
        projectReferences: config?.projectReferences,
      },
    );
    return await action({ program, files, configErrors: config?.errors ?? [], configFile });
  } finally {
    await api.close();
  }
}

/**
 * @param {VirtualFile} file
 * @param {number} offset
 * @param {boolean} toOriginal
 */
export function mapOffset(file, offset, toOriginal) {
  for (const [virtualStart, length, originalStart] of file.mappings) {
    const start = toOriginal ? virtualStart : originalStart;
    if (offset >= start && offset <= start + length) {
      return (toOriginal ? originalStart : virtualStart) + offset - start;
    }
  }
  return undefined;
}

/** @param {TSDiagnostic} diagnostic @returns {string} */
function diagnosticText(diagnostic) {
  return [diagnostic.text, ...(diagnostic.messageChain ?? []).map(diagnosticText)].join("\n");
}

/**
 * @param {string} fileName
 * @param {VirtualFile | undefined} file
 * @param {TSDiagnostic} diagnostic
 * @returns {import("vscode-languageserver").Diagnostic}
 */
export function mapDiagnostic(fileName, file, diagnostic) {
  const original = TextDocument.create(
    pathToFileURL(fileName).href,
    "typescript",
    0,
    file?.original ?? "",
  );
  const start =
    file && diagnostic.fileName === fileName ? mapOffset(file, diagnostic.pos, true) : undefined;
  const end =
    file && diagnostic.fileName === fileName ? mapOffset(file, diagnostic.end, true) : undefined;
  const mapped = start !== undefined && end !== undefined;
  const location =
    diagnostic.fileName && diagnostic.fileName !== fileName
      ? `${diagnostic.fileName}: `
      : diagnostic.fileName && !mapped
        ? "In generated code: "
        : "";
  return {
    range: {
      start: original.positionAt(mapped ? start : 0),
      end: original.positionAt(mapped ? end : 0),
    },
    severity: diagnostic.category === 1 ? 1 : diagnostic.category === 0 ? 2 : 3,
    code: diagnostic.code,
    source: "errorscript-ts7",
    message: location + diagnosticText(diagnostic),
  };
}

/** @param {string} fileName @param {Map<string, string>} buffers */
export function getEditorDiagnostics(fileName, buffers) {
  return withEditorProject(
    fileName,
    buffers,
    async ({ program, files, configErrors, configFile }) => {
      const diagnostics = [
        ...configErrors,
        ...(await program.getProgramDiagnostics()),
        ...(await program.getGlobalDiagnostics()),
        ...(await program.getSyntacticDiagnostics(fileName)),
        ...(await program.getSemanticDiagnostics(fileName)),
      ];
      return diagnostics
        .filter(
          (diagnostic) =>
            !diagnostic.fileName ||
            diagnostic.fileName === fileName ||
            diagnostic.fileName === configFile,
        )
        .map((diagnostic) => mapDiagnostic(fileName, files.get(fileName), diagnostic));
    },
  );
}

/**
 * @param {string} fileName
 * @param {Map<string, string>} buffers
 * @param {import("vscode-languageserver").Position} position
 * @returns {Promise<import("vscode-languageserver").Hover | null>}
 */
export function getEditorHover(fileName, buffers, position) {
  return withEditorProject(fileName, buffers, async ({ program, files }) => {
    const file = files.get(fileName);
    if (!file) return null;
    const document = TextDocument.create(
      pathToFileURL(fileName).href,
      "typescript",
      0,
      file.original,
    );
    const offset = document.offsetAt(position);
    const word = [...file.original.matchAll(/[A-Za-z_$][\w$]*/g)].find(
      (match) => offset >= match.index && offset < match.index + match[0].length,
    );
    if (!word) return null;
    const virtualOffset = mapOffset(file, word.index, false);
    if (virtualOffset === undefined) return null;
    const checker = program.getProject().checker;
    const type = await checker.getTypeAtPosition(fileName, virtualOffset);
    if (!type) return null;
    return {
      contents: {
        kind: "markdown",
        value: `\`\`\`typescript\n${word[0]}: ${await checker.typeToString(type)}\n\`\`\`\nErrorScript · TypeScript 7`,
      },
      range: {
        start: document.positionAt(word.index),
        end: document.positionAt(word.index + word[0].length),
      },
    };
  });
}
