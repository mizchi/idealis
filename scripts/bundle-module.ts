import { isAbsolute, relative, resolve } from "node:path";
import { build, normalizePath } from "vite-plus";

interface BundleOptions {
  globalName?: string;
  platform?: "node" | "browser";
  source?: string;
  alias?: Record<string, string>;
  define?: Record<string, string>;
  external?: string[];
}

// Browser fixtures and historical Node modules must each load from one script.
export async function bundleModule(entry: string, options: BundleOptions = {}) {
  const input = normalizePath(resolve(entry));
  const inputs: string[] = [];
  const node = options.platform === "node";
  const result = await build({
    configFile: false,
    envDir: false,
    publicDir: false,
    logLevel: "warn",
    resolve: { alias: options.alias ?? {} },
    define: options.define ?? {},
    ssr: { noExternal: true },
    plugins: [
      {
        name: "fixture-module",
        resolveId(id) {
          if (options.source !== undefined && id === input) return input;
        },
        load(id) {
          if (id === input) return options.source;
        },
        generateBundle() {
          for (const id of this.getModuleIds()) {
            inputs.push(
              isAbsolute(id) ? normalizePath(relative(process.cwd(), id)) : id,
            );
          }
        },
      },
    ],
    build: {
      ssr: node,
      target: node ? "node24" : "esnext",
      lib: {
        entry: input,
        ...(options.globalName && { name: options.globalName }),
        formats: [options.globalName ? "iife" : "es"],
      },
      write: false,
      minify: false,
      modulePreload: false,
      rolldownOptions: {
        external: options.external ?? [],
        output: { codeSplitting: false },
      },
    },
  });
  const [output] = Array.isArray(result) ? result : [result];
  const files = output && "output" in output ? output.output : [];
  const [chunk] = files;
  if (!chunk || chunk.type !== "chunk" || !chunk.code || files.length !== 1)
    throw new Error(`Expected one JavaScript bundle for ${entry}`);
  return { code: chunk.code, inputs };
}
