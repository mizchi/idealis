import { createHash } from "node:crypto";
import { readdir, readFile } from "node:fs/promises";
import { corpus as parserCorpus } from "../rtla-parser/corpus.js";
import {
  boundaries,
  foldCases,
  foldOrderControls,
  foldRejections,
  functionOrderCases,
  graphs,
  operatorSharingCases,
  seeds,
} from "../tlc-differential/corpus.js";
import type { Input as Request } from "./outcome.js";
import { shapes } from "./shapes.js";
import { sources } from "./sources.js";

export type Surface = "ts" | "wasm" | "native";

export interface Corpus {
  readonly name: string;
  readonly source: string;
  readonly config: string;
}

const directories = [
  "fixtures/tlc-operator-sharing",
  "fixtures/quint-generated/generated",
] as const;

const fixtureSharing = async (): Promise<readonly Corpus[]> => {
  const root = directories[0];
  const entries = (await readdir(root, { withFileTypes: true }))
    .filter((entry) => entry.isDirectory() && entry.name.startsWith("sharing-"))
    .map((entry) => entry.name)
    .sort();
  return Promise.all(
    entries.map(async (name) => ({
      name: `${root}/${name}`,
      source: await readFile(`${root}/${name}/Differential.tla`, "utf8"),
      config: await readFile(`${root}/${name}/Differential.cfg`, "utf8"),
    })),
  );
};

const fixtureQuint = async (): Promise<readonly Corpus[]> => {
  const root = directories[1];
  const modules = (await readdir(root))
    .filter((name) => name.endsWith(".tla"))
    .map((name) => name.slice(0, -4))
    .sort();
  return Promise.all(
    modules.flatMap((name) =>
      (["safety", "broken"] as const).map(async (property) => ({
        name: `${root}/${name}-${property}`,
        source: await readFile(`${root}/${name}.tla`, "utf8"),
        config: await readFile(`${root}/${name}-${property}.cfg`, "utf8"),
      })),
    ),
  );
};

export const corpora = async (): Promise<readonly Corpus[]> => {
  const inputs: Corpus[] = [
    ...(await parserCorpus()).map((input) => ({
      name: `parser:${input.name}`,
      source: input.source,
      config: input.config,
    })),
    ...[
      ...foldCases(),
      ...foldOrderControls(),
      ...foldRejections(),
      ...functionOrderCases(),
      ...boundaries(),
      ...graphs(seeds.development),
      ...(await operatorSharingCases()),
    ].map((input) => ({
      name: `differential:${input.name}`,
      source: input.source,
      config: input.config,
    })),
    ...(await fixtureSharing()),
    ...(await fixtureQuint()),
    ...sources()
      .filter((input) => input.equivalent === null && input.semantic === null)
      .map((input) => ({
        name: `nesting:${input.name}`,
        source: input.source,
        config: input.config,
      })),
    ...shapes().map((input) => ({
      name: `shape:${input.name}`,
      source: input.source,
      config: input.config,
    })),
  ];
  const seen = new Set<string>();
  return inputs.filter((input) => {
    const key = JSON.stringify([input.source, input.config]);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

export const sha256 = (text: string): string =>
  createHash("sha256").update(text).digest("hex");

export const fingerprint = async (
  compile: (input: Request) => Promise<unknown>,
  input: Corpus,
): Promise<string> => {
  const cells: string[] = [];
  for (const irVersion of [1, 2] as const)
    try {
      cells.push(
        `p:${sha256(JSON.stringify(await compile({ source: input.source, config: input.config, irVersion })))}`,
      );
    } catch (error) {
      cells.push(`e:${error instanceof Error ? error.message : String(error)}`);
    }
  return cells.join("\n");
};
