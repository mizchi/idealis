import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import {
  mkdir,
  mkdtemp,
  readFile,
  realpath,
  rm,
  stat,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, isAbsolute, join, relative, resolve, sep } from "node:path";
import { isDeepStrictEqual, promisify } from "node:util";
import { layoutGraph } from "./layout.mjs";
import { stateNames } from "./topology.js";

const exec = promisify(execFile);
const limit = 1024 * 1024;
const diagnostic = (area, error) => ({
  area,
  message: error.stderr?.trim() || error.message,
});
const list = (value) => (Array.isArray(value) ? value : []);

function inside(root, path) {
  const name = relative(root, resolve(root, path));
  if (
    !name ||
    name === ".." ||
    name.startsWith(".." + sep) ||
    isAbsolute(name)
  ) {
    throw new Error("ワークスペース外のパスです");
  }
  return name;
}

async function source(root, name) {
  const path = await realpath(join(root, name));
  inside(root, path);
  const info = await stat(path);
  if (!info.isFile() || info.size > limit)
    throw new Error(`読み込めないファイルです: ${name}`);
  return readFile(path, "utf8");
}

async function temporary(fn) {
  const path = await mkdtemp(join(tmpdir(), "idealis-studio-"));
  try {
    return await fn(path);
  } finally {
    await rm(path, { recursive: true, force: true });
  }
}

async function materialize(path, files) {
  for (const [name, text] of Object.entries(files)) {
    if (text === null) continue;
    const file = join(path, name);
    await mkdir(dirname(file), { recursive: true });
    await writeFile(file, text);
  }
}

const invoke = (checker, args, cwd) =>
  exec(checker, args, { cwd, timeout: 30_000, maxBuffer: 8 * limit });

async function collect(root, model, spec, known) {
  const files = Object.create(null);
  const diagnostics = [];
  const read = async (name, area) => {
    if (Object.hasOwn(files, name)) return files[name];
    try {
      files[name] = await source(root, name);
    } catch (error) {
      files[name] = null;
      diagnostics.push(diagnostic(area, error));
    }
    return files[name];
  };
  const components = [];
  const entry = await read(model, "model");
  if (model.endsWith(".project.json") && entry !== null) {
    try {
      const { fragments, dictionary } = JSON.parse(entry);
      if (!Array.isArray(fragments)) throw new Error("fragments が必要です");
      const directory = dirname(model);
      const memberPath = (parent, path) => {
        if (typeof path !== "string" || !path || isAbsolute(path)) {
          throw new Error("構成ファイルには相対パスが必要です");
        }
        const project = resolve(root, directory);
        return inside(root, join(project, inside(project, join(parent, path))));
      };
      if (dictionary) await read(memberPath(".", dictionary), "model");
      for (const fragment of fragments) {
        if (typeof fragment === "string" || !("component" in fragment)) {
          const path = typeof fragment === "string" ? fragment : fragment.path;
          const file = memberPath(".", path);
          await read(file, "model");
          components.push({
            file,
            name: fragment.namespace ?? path,
            ports: Object.entries(fragment.ports ?? {}).map(([name, port]) => ({
              name,
              ...port,
            })),
          });
          continue;
        }

        const descriptor = memberPath(".", fragment.component);
        const descriptorText = await read(descriptor, "model");
        if (descriptorText === null) continue;
        const {
          body,
          interface: interfaceFile,
          types = [],
          ports = {},
        } = JSON.parse(descriptorText);
        const parent = dirname(relative(directory, descriptor));
        const file = memberPath(parent, body);
        const dependencies = [
          descriptor,
          ...[interfaceFile, ...types].map((path) => memberPath(parent, path)),
        ];
        for (const dependency of [file, ...dependencies])
          await read(dependency, "model");
        components.push({
          file,
          name: fragment.namespace,
          dependencies,
          ports: Object.entries(fragment.ports ?? {}).map(([name, state]) => ({
            name,
            state,
            access: ports[name],
          })),
        });
      }
    } catch (error) {
      diagnostics.push(diagnostic("model", error));
    }
  }
  if (spec) await read(spec, "spec");
  for (const file of known) {
    // Removed fragments can still be present on disk and useful in the source view.
    if (!Object.hasOwn(files, file))
      files[file] = await source(root, file).catch(() => null);
  }
  return { files, components, diagnostics };
}

function modelSymbols(ir, components, checks) {
  return [
    ...list(ir.relations).map((item) => ({
      kind: "state",
      name: item.name,
      value: item,
    })),
    ...list(ir.actions).map((item) => ({
      kind: "action",
      name: item.name,
      value: item,
    })),
    ...checks.map(({ rules: _rules, ...item }) => ({
      kind: "check",
      name: item.name,
      value: item,
    })),
    ...["sigs", "enums", "records", "sums"].flatMap((field) =>
      list(ir[field]).map((item) => ({
        kind: "type",
        name: item.name,
        value: item,
      })),
    ),
    ...components.map((item) => ({
      kind: "component",
      name: item.name,
      value: item,
    })),
    ...["facts", "init", "scope"].map((field) => ({
      kind: "model",
      name: field,
      value: ir[field],
    })),
  ];
}

function readSpec(text, model, checks) {
  if (text === undefined) return { rules: [], terms: [] };
  const { spec } = JSON.parse(text);
  if (!spec || !Array.isArray(spec.rules ?? []))
    throw new Error("仕様の spec.rules を読み込めません");
  const checkNames = new Set(checks.map(({ name }) => name));
  const rules = list(spec.rules).map((item) => {
    if (typeof item.id !== "string" || typeof item.text?.ja !== "string")
      throw new Error("ルールに id と text.ja が必要です");
    const claims = [
      ...list(item.claims),
      ...list(item.clauses)
        .filter((clause) => clause.claim)
        .map((clause) => ({ ...clause.claim, clause: clause.id })),
    ].map((claim) => ({
      ...claim,
      resolved:
        typeof claim.model === "string" &&
        claim.model === model &&
        checkNames.has(claim.check),
    }));
    return { ...item, claims };
  });
  return { rules, terms: list(spec.terms) };
}

async function analyze(snapshot, model, spec, checker) {
  const diagnostics = [...snapshot.diagnostics];
  let ir = null;
  let descriptions = {};
  let domains = [];
  let stateDeclarations = [];
  if (!diagnostics.some(({ area }) => area === "model")) {
    try {
      ({
        ir,
        descriptions,
        domains = [],
        stateDeclarations = [],
      } = await temporary(async (path) => {
        await materialize(path, snapshot.files);
        const { stdout } = await invoke(
          checker,
          ["generate", model, "source-view"],
          path,
        );
        return JSON.parse(stdout);
      }));
    } catch (error) {
      diagnostics.push(diagnostic("model", error));
    }
  }
  const checks = [
    ...list(ir?.checks),
    ...list(ir?.transitions).map((item) => ({ ...item, kind: "transition" })),
  ].map((item) => ({ ...item, rules: [] }));
  let specification = { rules: [], terms: [] };
  if (spec && snapshot.files[spec] !== null) {
    try {
      specification = readSpec(snapshot.files[spec], model, checks);
    } catch (error) {
      diagnostics.push(diagnostic("spec", error));
    }
  }
  for (const rule of specification.rules) {
    for (const { check, resolved } of rule.claims) {
      if (!resolved) continue;
      const linked = checks.find(({ name }) => name === check);
      if (!linked.rules.includes(rule.id)) linked.rules.push(rule.id);
    }
  }
  const bySource = new Map(
    Object.entries(descriptions).map(([file, text]) => [
      relative(".", resolve(file)),
      text,
    ]),
  );
  const components = snapshot.components.map((component) => ({
    ...component,
    description: bySource.get(component.file),
  }));
  const description = bySource.get(model) ?? bySource.get(components[0]?.file);
  const stateCatalog = stateDeclarations.map((state) => ({
    ...state,
    kind: state.owner
      ? "internal"
      : new Set(state.ports.map(({ model }) => model)).size > 1
        ? "shared"
        : "project",
  }));
  let graph = null;
  try {
    graph = await layoutGraph(
      components,
      {
        ja: stateNames(specification.terms, model, "ja"),
        en: stateNames(specification.terms, model, "en"),
      },
      domains,
    );
  } catch (error) {
    diagnostics.push(diagnostic("layout", error));
  }
  const symbols =
    ir && !diagnostics.some(({ area }) => area === "spec")
      ? [
          ...modelSymbols(ir, components, checks),
          ...domains.map((domain) => ({
            kind: "domain",
            name: domain.id,
            value: domain,
          })),
          ...specification.rules.map((rule) => ({
            kind: "rule",
            name: rule.id,
            value: {
              ...rule,
              claims: rule.claims.map(
                ({ resolved: _resolved, ...claim }) => claim,
              ),
            },
          })),
          ...specification.terms.map((term) => ({
            kind: "term",
            name: term.id,
            value: term,
          })),
        ]
      : null;
  return {
    symbols,
    name: ir?.name ?? model,
    files: snapshot.files,
    description,
    graph,
    stateCatalog,
    components,
    actions: list(ir?.actions),
    states: list(ir?.relations),
    checks,
    ...specification,
    diagnostics,
  };
}

async function fileDiffs(base, current) {
  const changes = Object.keys({ ...base, ...current })
    .filter((name) => (base[name] ?? null) !== (current[name] ?? null))
    .sort();
  if (changes.length === 0) return [];
  return temporary(async (path) => {
    const output = [];
    for (const name of changes) {
      const before = base[name] ?? null;
      const after = current[name] ?? null;
      const left = before === null ? "/dev/null" : "before";
      const right = after === null ? "/dev/null" : "after";
      if (before !== null) await writeFile(join(path, left), before);
      if (after !== null) await writeFile(join(path, right), after);
      let patch;
      try {
        ({ stdout: patch } = await exec(
          "git",
          [
            "--no-pager",
            "diff",
            "--no-index",
            "--no-ext-diff",
            "--no-textconv",
            "--text",
            "--",
            left,
            right,
          ],
          { cwd: path, maxBuffer: 4 * limit },
        ));
      } catch (error) {
        if (error.code !== 1) throw error;
        patch = error.stdout;
      }
      output.push({
        name,
        status:
          before === null ? "added" : after === null ? "removed" : "modified",
        patch: patch
          .split("\n")
          .filter(
            (line, index) => index >= 4 || /^(--- |\+\+\+ |@@)/.test(line),
          )
          .join("\n"),
      });
    }
    return output;
  });
}

function symbolDiffs(base, current) {
  if (!base || !current) return null;
  const identify = ({ kind, name }) => `${kind}:${name}`;
  const original = new Map(base.map((item) => [identify(item), item]));
  const updated = new Map(current.map((item) => [identify(item), item]));
  return [...new Set([...original.keys(), ...updated.keys()])].flatMap((id) => {
    const before = original.get(id);
    const after = updated.get(id);
    if (isDeepStrictEqual(before, after)) return [];
    const { name, kind } = after ?? before;
    return [
      {
        kind,
        name,
        status: !before ? "added" : !after ? "removed" : "modified",
      },
    ];
  });
}

export async function createWorkspace({ root, model, spec, checker }) {
  root = await realpath(root);
  model = inside(root, model);
  spec = spec ? inside(root, spec) : undefined;
  checker = resolve(checker);
  const initial = await collect(root, model, spec, []);
  const base = await analyze(initial, model, spec, checker);
  let latest;
  let pending = Promise.resolve();
  const known = new Set(Object.keys(initial.files));
  const refresh = () => {
    const response = pending.then(async () => {
      const snapshot = await collect(root, model, spec, known);
      for (const file of Object.keys(snapshot.files)) known.add(file);
      const revision = createHash("sha256")
        .update(JSON.stringify(snapshot))
        .digest("hex");
      if (latest?.revision === revision) return latest;
      const current = isDeepStrictEqual(initial, snapshot)
        ? base
        : await analyze(snapshot, model, spec, checker);
      latest = {
        model,
        spec,
        revision,
        current,
        preview: {
          files: await fileDiffs(base.files, current.files),
          symbols: symbolDiffs(base.symbols, current.symbols),
        },
      };
      return latest;
    });
    pending = response.catch(() => {});
    return response;
  };
  return {
    refresh,
    async witness(check, revision) {
      const { current, revision: actual } = await refresh();
      if (revision !== actual)
        throw new Error(
          "ファイルが変更されました。表示を更新してから実行してください。",
        );
      if (!current.checks.some(({ name }) => name === check))
        throw new Error("検査項目が見つかりません");
      return temporary(async (path) => {
        await materialize(path, current.files);
        const { stdout } = await invoke(
          checker,
          ["witness", model, check, "--max-states", "200000"],
          path,
        );
        return JSON.parse(stdout);
      });
    },
  };
}
