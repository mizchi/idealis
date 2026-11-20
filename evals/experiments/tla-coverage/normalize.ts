import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { isRecord } from "../../../packages/idealis/src/trace.js";

interface Items {
  definitions: unknown[];
  instances: unknown[];
  recursive: unknown[];
  modules: unknown[];
  constants: unknown[];
  variables: unknown[];
  assumptions: unknown[];
  theorems: unknown[];
  directives: unknown[];
}
interface Tree {
  readonly image: string;
  readonly children: readonly Tree[];
}
const catalog: { aliases: readonly { spelling: string; canonical: string }[] } =
  JSON.parse(
    await readFile("crates/rtla/tests/fixtures/operators.json", "utf8"),
  );
const aliases = new Map(
  catalog.aliases.map((alias) => [alias.spelling, alias.canonical]),
);
const structural = new Map([
  ["∀", "\\A"],
  ["∃", "\\E"],
  ["⟨", "<<"],
  ["⟩_", ">>_"],
  ["ℕ", "Nat"],
  ["ℤ", "Int"],
  ["ℝ", "Real"],
]);
const required = <Value>(value: Value | undefined): Value => {
  assert.notEqual(value, undefined, "Missing required SANY syntax child");
  assert.ok(value !== undefined);
  return value;
};
const spelling = (text: string): string =>
  structural.get(text) ?? aliases.get(text) ?? text;
const record = (value: unknown): Record<string, unknown> => {
  assert.ok(isRecord(value));
  return value;
};
const array = (value: unknown): unknown[] => {
  assert.ok(Array.isArray(value));
  return value;
};
const identifier = (name: string) => ({
  kind: "Identifier",
  name: spelling(name),
});
const declaration = (name: string, arity = 0) => ({
  name: spelling(name),
  arity,
});
const namePattern = (name: string) => ({ kind: "Name", name });
const ast = (node: Tree) =>
  node.children.filter((child) => child.image.startsWith("N_"));
const tokens = (node: Tree): string[] =>
  node.children.length ? node.children.flatMap(tokens) : [node.image];
const cleanNumber = (text: string) => {
  const radix = /^\\([boh])(.+)$/i.exec(text);
  return BigInt(
    radix
      ? `${{ b: "0b", o: "0o", h: "0x" }[required(radix[1]).toLowerCase()]}${required(radix[2])}`
      : text,
  ).toString();
};
const apply = (operator: string, args: unknown[]) => {
  const special = new Map([
    ["'", "Prime"],
    ["UNCHANGED", "Unchanged"],
    ["ENABLED", "Enabled"],
    ["[]", "Always"],
    ["<>", "Eventually"],
  ]);
  const kind = special.get(operator);
  if (kind) return { kind, expression: args[0] };
  if (["/\\", "\\/"].includes(operator))
    return { kind: operator === "/\\" ? "And" : "Or", operands: args };
  return { kind: "Apply", operator, args };
};

// Both adapters produce ordered trees. Only spelling, parentheses, source locations,
// implicit empty fields, and the parser's split of grouped binders are normalized.
export const normalizeRtla = (value: unknown, level = 0): unknown => {
  if (Array.isArray(value))
    return value.map((entry) => normalizeRtla(entry, level));
  if (!isRecord(value)) return value;
  if (["Number", "Integer"].includes(String(value.kind)))
    return {
      kind: "Number",
      value: cleanNumber(String(value.value ?? value.text)),
    };
  if (value.kind === "Identifier" && typeof value.name === "string")
    return identifier(value.name.replace(/^<\*>/, `<${level}>`));
  const result = Object.fromEntries(
    Object.entries(value)
      .filter(([key]) => !["location", "isRecursive"].includes(key))
      .map(([key, entry]) => [key, normalizeRtla(entry, level)]),
  );
  if (value.kind === "LetModule") result.kind = "Let";
  for (const key of ["instances", "recursive", "modules", "directives"])
    if (Array.isArray(result[key]) && result[key].length === 0)
      delete result[key];
  if (result.isBulleted === false) delete result.isBulleted;
  if (
    result.params &&
    Array.isArray(result.params) &&
    result.params.length === 0 &&
    typeof result.module === "string"
  )
    delete result.params;
  if (value.kind === "Steps")
    result.steps = array(value.steps).map((step) => {
      const source = record(step);
      const item = record(normalizeRtla(step, Number(source.level)));
      item.number = String(source.number)
        .replace(/^<[^>]+>/, `<${source.level}>`)
        .replace(/\.+$/, "");
      return item;
    });
  return result;
};

export const normalizeSany = (tree: unknown): unknown => {
  const validate = (value: unknown): Tree => {
    const item = record(value);
    assert.equal(typeof item.image, "string");
    const children = array(item.children).map(validate);
    return { image: String(item.image), children };
  };
  const exprs = (nodes: readonly Tree[], level = 0) =>
    nodes
      .filter(
        (node) =>
          node.image.startsWith("N_") ||
          node.image.startsWith('"') ||
          node.image === "@",
      )
      .map((node) => expression(node, level));
  const decl = (node: Tree): unknown => {
    const leaves = tokens(node);
    if (node.image === "N_InfixDecl")
      return declaration(required(leaves[1]), 2);
    if (node.image === "N_PrefixDecl")
      return declaration(required(leaves[0]), 1);
    if (node.image === "N_PostfixDecl")
      return declaration(required(leaves[1]), 1);
    return declaration(
      required(leaves[0]),
      leaves.filter((leaf) => leaf === "_").length,
    );
  };
  const args = (node: Tree, level: number) => exprs(node.children, level);
  const general = (node: Tree, level: number): unknown => {
    const prefix = required(node.children[0]);
    const final = required(node.children[1]);
    const selectors: unknown[] = [];
    let root: unknown;
    for (const entry of [
      ...prefix.children,
      { image: "N_IdPrefixElement", children: [final] },
    ]) {
      const head = required(entry.children[0]);
      const opArgs = entry.children.find((child) => child.image === "N_OpArgs");
      const values = opArgs ? args(opArgs, level) : [];
      if (head.image === "N_IdentifierTuple") {
        root = expression(head, level);
      } else if (
        root !== undefined &&
        ["<<", ">>", ":", "@"].includes(spelling(head.image))
      ) {
        selectors.push({
          kind: { "<<": "Left", ">>": "Right", ":": "Body", "@": "Bound" }[
            spelling(head.image)
          ],
        });
      } else if (head.image === "N_StructOp") {
        const leaf = required(head.children[0]);
        const text = spelling(leaf.image);
        selectors.push(
          text === "<<"
            ? { kind: "Left" }
            : text === ">>"
              ? { kind: "Right" }
              : text === ":"
                ? { kind: "Body" }
                : text === "@"
                  ? { kind: "Bound" }
                  : leaf.image === "N_OpArgs"
                    ? { kind: "Arguments", args: args(leaf, level) }
                    : { kind: "Index", index: Number(tokens(head)[0]) },
        );
      } else if (head.image === "N_OpArgs") {
        selectors.push({ kind: "Arguments", args: args(head, level) });
      } else {
        const name = spelling(head.image.replace(/^<\*>/, `<${level}>`));
        if (root === undefined)
          root = values.length
            ? { kind: "Apply", operator: name, args: values }
            : identifier(name);
        else selectors.push({ kind: "Name", name, args: values });
      }
    }
    assert.ok(root !== undefined);
    if (!selectors.length)
      return record(root).name === "@" ? { kind: "ExceptAt" } : root;
    const base = record(root);
    if (
      base.kind === "Identifier" &&
      selectors.every(
        (selector) =>
          record(selector).kind === "Name" &&
          array(record(selector).args).length === 0,
      )
    )
      return identifier(
        [
          String(base.name),
          ...selectors.map((selector) => String(record(selector).name)),
        ].join("!"),
      );
    return { kind: "Selection", expression: root, selectors };
  };
  const patterns = (nodes: readonly Tree[]) =>
    nodes
      .filter((node) => node.image !== ",")
      .map((node) =>
        node.image === "N_IdentifierTuple"
          ? {
              kind: "Tuple",
              names: tokens(node).filter(
                (token) => !["<<", ">>", "⟨", "⟩", ","].includes(token),
              ),
            }
          : namePattern(node.image),
      );
  const bounds = (nodes: readonly Tree[], level = 0) =>
    nodes
      .filter((node) => node.image === "N_QuantBound")
      .flatMap((node) => {
        const index = node.children.findIndex(
          (child) => spelling(child.image) === "\\in",
        );
        assert.ok(index > 0);
        const set = expression(required(node.children[index + 1]), level);
        return patterns(node.children.slice(0, index)).map((pattern) => ({
          pattern,
          set,
        }));
      });
  const proofBindings = (nodes: readonly Tree[], level: number) =>
    nodes.some((node) => node.image === "N_QuantBound")
      ? { kind: "Bounded", bounds: bounds(nodes, level) }
      : {
          kind: "Unbounded",
          names: nodes
            .filter((node) => node.image !== ",")
            .map((node) => node.image),
        };
  const head = (node: Tree) => {
    const leaves = tokens(node);
    if (node.image === "N_InfixLHS")
      return {
        name: spelling(required(leaves[1])),
        params: [
          declaration(required(leaves[0])),
          declaration(required(leaves[2])),
        ],
      };
    if (node.image === "N_PrefixLHS")
      return {
        name: spelling(required(leaves[0])),
        params: [declaration(required(leaves[1]))],
      };
    if (node.image === "N_PostfixLHS")
      return {
        name: spelling(required(leaves[1])),
        params: [declaration(required(leaves[0]))],
      };
    return { name: spelling(required(leaves[0])), params: ast(node).map(decl) };
  };
  const instance = (
    node: Tree,
    local = false,
    heading?: ReturnType<typeof head>,
  ): unknown => {
    if (node.image === "N_Instance")
      return instance(
        required(node.children.at(-1)),
        node.children[0]?.image === "LOCAL",
        heading,
      );
    return {
      ...(heading
        ? {
            name: heading.name,
            ...(heading.params.length ? { params: heading.params } : {}),
          }
        : {}),
      module: node.children[1]?.image,
      substitutions: node.children
        .filter((child) => child.image === "N_Substitution")
        .map((child) => ({
          name: spelling(required(child.children[0]).image),
          expression: expression(required(child.children[2])),
        })),
      isLocal: local,
    };
  };
  const definitions = (node: Tree, local = false): unknown => {
    const children = node.children.filter((child) => child.image !== "LOCAL");
    if (node.image === "N_FunctionDefinition")
      return {
        name: children[0]?.image,
        kind: "Function",
        bounds: bounds(children),
        body: expression(required(children.at(-1))),
        isLocal: local || node.children[0]?.image === "LOCAL",
      };
    const heading = head(required(children[0]));
    return {
      ...heading,
      kind: "Operator",
      body: expression(required(children.at(-1))),
      isLocal: local || node.children[0]?.image === "LOCAL",
    };
  };
  const items = (nodes: readonly Tree[]) => {
    const result: Items = {
      definitions: [],
      instances: [],
      recursive: [],
      modules: [],
      constants: [],
      variables: [],
      assumptions: [],
      theorems: [],
      directives: [],
    };
    for (const node of nodes) {
      switch (node.image) {
        case "N_OperatorDefinition":
        case "N_FunctionDefinition":
          result.definitions.push(definitions(node));
          break;
        case "N_Recursive":
          result.recursive.push(...ast(node).map(decl));
          break;
        case "N_Module":
          result.modules.push(module(node));
          break;
        case "N_Instance":
          result.instances.push(instance(node));
          break;
        case "N_ModuleDefinition": {
          const children = node.children.filter(
            (child) => child.image !== "LOCAL",
          );
          result.instances.push(
            instance(
              required(children.at(-1)),
              node.children[0]?.image === "LOCAL",
              head(required(children[0])),
            ),
          );
          break;
        }
        case "N_ParamDeclaration":
          result.constants.push(
            ...ast(node)
              .filter((child) => child.image !== "N_ConsDecl")
              .map(decl),
          );
          break;
        case "N_VariableDeclaration":
          result.variables.push(
            ...node.children
              .slice(1)
              .filter((child) => child.image !== ",")
              .map((child) => declaration(child.image)),
          );
          break;
        case "N_Assumption": {
          const named =
            node.children[2]?.image === "==" || node.children[2]?.image === "≜";
          result.assumptions.push({
            ...(named ? { name: node.children[1]?.image } : {}),
            expression: expression(required(node.children.at(-1))),
            isAxiom: node.children[0]?.image === "AXIOM",
          });
          break;
        }
        case "N_Theorem": {
          const children = node.children;
          const named = ["==", "≜"].includes(children[2]?.image ?? "");
          const proofNode = children.find((child) =>
            ["N_Proof", "N_TerminalProof"].includes(child.image),
          );
          result.theorems.push({
            ...(named ? { name: children[1]?.image } : {}),
            statement: expression(required(children[named ? 3 : 1])),
            ...(proofNode ? { proof: proof(proofNode, 0) } : {}),
          });
          break;
        }
        case "N_UseOrHide":
          result.directives.push({
            isHide: node.children[0]?.image === "HIDE",
            usage: usage(node, 0),
          });
          break;
        default:
          if (/^-{4,}$/.test(node.image)) break;
          throw new Error(`Unnormalized module item ${node.image}`);
      }
    }
    return result;
  };
  const module = (node: Tree): unknown => {
    const heading = required(node.children[0]);
    const extending = required(node.children[1]);
    const body = required(node.children[2]);
    const entries = items(body.children);
    return {
      name: heading.children[1]?.image,
      extends: extending.children
        .slice(1)
        .filter((child) => child.image !== ",")
        .map((child) => child.image),
      ...entries,
    };
  };
  const usage = (node: Tree, level: number): unknown => {
    const split = node.children.findIndex((child) =>
      ["DEF", "DEFS"].includes(child.image),
    );
    const references = (nodes: readonly Tree[]) => {
      const result: unknown[] = [];
      for (let index = 0; index < nodes.length; index++) {
        const child = required(nodes[index]);
        if ([",", "BY", "USE", "HIDE", "ONLY", "PROOF"].includes(child.image))
          continue;
        if (child.image === "MODULE") {
          result.push({ kind: "Module", name: nodes[++index]?.image });
          continue;
        }
        result.push({
          kind: "Expression",
          expression: expression(child, level),
        });
      }
      return result;
    };
    return {
      only: node.children.some((child) => child.image === "ONLY"),
      facts: references(
        split < 0 ? node.children : node.children.slice(0, split),
      ),
      definitions: split < 0 ? [] : references(node.children.slice(split + 1)),
    };
  };
  const proof = (node: Tree, parent: number): unknown => {
    if (node.image === "N_TerminalProof") {
      const key = node.children.find((child) =>
        ["BY", "OBVIOUS", "OMITTED"].includes(child.image),
      )?.image;
      return key === "BY"
        ? { kind: "By", ...record(usage(node, parent)) }
        : { kind: key === "OBVIOUS" ? "Obvious" : "Omitted" };
    }
    assert.equal(node.image, "N_Proof");
    let proofLevel: number | undefined;
    const steps = ast(node).map((child) => {
      assert.equal(child.image, "N_ProofStep");
      const raw = required(child.children[0]).image;
      const match = /^<([^>]+)>/.exec(raw);
      assert.ok(match);
      const level = Number.isNaN(Number(required(match[1])))
        ? (proofLevel ?? parent)
        : Number(required(match[1]));
      proofLevel = level;
      const number = raw.replace(/^<[^>]+>/, `<${level}>`).replace(/\.+$/, "");
      const body = required(child.children[1]);
      const children = body.children;
      let statement: unknown;
      switch (body.image) {
        case "N_AssertStep":
          statement = {
            kind: "Assert",
            suffices: children[0]?.image === "SUFFICES",
            statement: expression(required(children.at(-1)), level),
          };
          break;
        case "N_QEDStep":
          statement = { kind: "Qed" };
          break;
        case "N_CaseStep":
          statement = {
            kind: "Case",
            expression: expression(required(children[1]), level),
          };
          break;
        case "N_HaveStep":
          statement = {
            kind: "Have",
            expression: expression(required(children[1]), level),
          };
          break;
        case "N_PickStep": {
          const colon = children.findIndex((child) => child.image === ":");
          statement = {
            kind: "Pick",
            bindings: proofBindings(children.slice(1, colon), level),
            expression: expression(required(children[colon + 1]), level),
          };
          break;
        }
        case "N_TakeStep":
          statement = {
            kind: "Take",
            bindings: proofBindings(children.slice(1), level),
          };
          break;
        case "N_WitnessStep":
          statement = {
            kind: "Witness",
            expressions: exprs(children.slice(1), level),
          };
          break;
        case "N_UseOrHide":
          statement = {
            kind: children[0]?.image === "HIDE" ? "Hide" : "Use",
            usage: usage(body, level),
          };
          break;
        case "N_DefStep": {
          const entries = items(ast(body));
          statement = {
            kind: "Define",
            definitions: entries.definitions,
            instances: entries.instances,
          };
          break;
        }
        case "N_Instance":
        case "N_NonLocalInstance":
          statement = { kind: "Instance", instance: instance(body) };
          break;
        default:
          throw new Error(`Unnormalized proof step ${body.image}`);
      }
      const childProof = child.children[2];
      return {
        number,
        level,
        body: statement,
        ...(childProof ? { proof: proof(childProof, level + 1) } : {}),
      };
    });
    return { kind: "Steps", steps };
  };
  const expression = (node: Tree, level = 0): unknown => {
    const children = node.children;
    const expressions = () => exprs(children, level);
    switch (node.image) {
      case "N_Module":
        return module(node);
      case "N_Number":
        return { kind: "Number", value: cleanNumber(tokens(node).join("")) };
      case "N_Real":
        return { kind: "Decimal", text: tokens(node).join("") };
      case "N_ParenExpr":
        return expression(required(children[1]), level);
      case "N_GeneralId":
      case "N_GenInfixOp":
      case "N_GenPrefixOp":
      case "N_GenNonExpPrefixOp":
      case "N_GenPostfixOp":
        return general(node, level);
      case "N_OpApplication": {
        const target = record(expression(required(children[0]), level));
        const values = args(required(children[1]), level);
        if (target.kind === "Identifier")
          return { kind: "Apply", operator: target.name, args: values };
        if (target.kind === "Selection") {
          const selectors = array(target.selectors).map(record);
          const last = required(selectors.at(-1));
          if (last.kind === "Name" && array(last.args).length === 0) {
            return {
              ...target,
              selectors: [...selectors.slice(0, -1), { ...last, args: values }],
            };
          }
        }
        throw new Error("Unnormalized selected operator application");
      }
      case "N_InfixExpr":
        return apply(
          String(record(expression(required(children[1]), level)).name),
          [
            expression(required(children[0]), level),
            expression(required(children[2]), level),
          ],
        );
      case "N_PrefixExpr": {
        const operator = String(
          record(expression(required(children[0]), level)).name,
        );
        return apply(operator === "-" ? "-." : operator, [
          expression(required(children[1]), level),
        ]);
      }
      case "N_PostfixExpr":
        return apply(
          String(record(expression(required(children[1]), level)).name),
          [expression(required(children[0]), level)],
        );
      case "N_ConjList":
      case "N_DisjList":
        return {
          kind: node.image === "N_ConjList" ? "And" : "Or",
          isBulleted: true,
          operands: children.map((child) =>
            expression(required(child.children[1]), level),
          ),
        };
      case "N_IfThenElse":
        return {
          kind: "If",
          condition: expression(required(children[1]), level),
          consequent: expression(required(children[3]), level),
          alternative: expression(required(children[5]), level),
        };
      case "N_Case":
        return {
          kind: "Case",
          arms: children
            .filter((child) => child.image === "N_CaseArm")
            .map((child) => ({
              condition: expression(required(child.children[0]), level),
              body: expression(required(child.children[2]), level),
            })),
          ...(children.some((child) => child.image === "N_OtherArm")
            ? {
                other: expression(
                  required(
                    required(
                      children.find((child) => child.image === "N_OtherArm"),
                    ).children[2],
                  ),
                  level,
                ),
              }
            : {}),
        };
      case "N_BoundedQuant":
        return {
          kind:
            spelling(required(children[0]).image) === "\\A"
              ? "Forall"
              : "Exists",
          bounds: bounds(children, level),
          body: expression(required(children.at(-1)), level),
        };
      case "N_UnboundedQuant": {
        const colon = children.findIndex((child) => child.image === ":");
        const quantifier = spelling(required(children[0]).image);
        return {
          kind: "UnboundedQuantifier",
          quantifier: ["\\A", "\\AA"].includes(quantifier)
            ? "Forall"
            : "Exists",
          names: children
            .slice(1, colon)
            .filter((child) => child.image !== ",")
            .map((child) => child.image),
          isTemporal: ["\\AA", "\\EE"].includes(quantifier),
          body: expression(required(children.at(-1)), level),
        };
      }
      case "N_UnBoundedOrBoundedChoose": {
        const bound = children.find((child) => child.image === "N_MaybeBound");
        return {
          kind: "Choose",
          pattern: patterns([required(children[1])])[0],
          ...(bound?.children.length
            ? { set: expression(required(bound.children[1]), level) }
            : {}),
          body: expression(required(children.at(-1)), level),
        };
      }
      case "N_IdentifierTuple":
        return {
          kind: "Tuple",
          elements: tokens(node)
            .filter((token) => !["<<", ">>", "⟨", "⟩", ","].includes(token))
            .map(identifier),
        };
      case "N_Tuple":
      case "N_SetEnumerate":
        return {
          kind: node.image === "N_Tuple" ? "Tuple" : "SetEnumeration",
          elements: expressions(),
        };
      case "N_Times":
        return {
          kind: "Product",
          factors: children
            .filter((child) => child.image !== "N_GenInfixOp")
            .map((child) => expression(child, level)),
        };
      case "N_FcnConst":
        return {
          kind: "FunctionConstruction",
          bounds: bounds(children, level),
          body: expression(required(children.at(-2)), level),
        };
      case "N_SetOfFcns":
        return {
          kind: "FunctionSet",
          domain: expression(required(children[1]), level),
          range: expression(required(children[3]), level),
        };
      case "N_SetOfAll":
        return {
          kind: "SetMap",
          expression: expression(required(children[1]), level),
          bounds: bounds(children, level),
        };
      case "N_SubsetOf":
        return {
          kind: "SetFilter",
          bound: {
            pattern: patterns([required(children[1])])[0],
            set: expression(required(children[3]), level),
          },
          predicate: expression(required(children[5]), level),
        };
      case "N_RcdConstructor":
      case "N_SetOfRcds":
        return {
          kind: node.image === "N_RcdConstructor" ? "Record" : "RecordSet",
          fields: ast(node).map((child) => ({
            name: child.children[0]?.image,
            value: expression(required(child.children[2]), level),
          })),
        };
      case "N_RecordComponent":
        return {
          kind: "FieldAccess",
          record: expression(required(children[0]), level),
          field: children[2]?.image,
        };
      case "N_FcnAppl":
        if (children[0]?.image === "[")
          return { kind: "Bracket", args: exprs(children.slice(1, -1), level) };
        return {
          kind: "FunctionApplication",
          function: expression(required(children[0]), level),
          args: exprs(children.slice(2, -1), level),
        };
      case "N_Except":
        return {
          kind: "Except",
          function: expression(required(children[1]), level),
          updates: children
            .filter((child) => child.image === "N_ExceptSpec")
            .map((child) => ({
              path: child.children
                .filter((part) => part.image === "N_ExceptComponent")
                .map((part) =>
                  part.children[0]?.image === "."
                    ? { kind: "Field", name: part.children[1]?.image }
                    : {
                        kind: "Index",
                        args: exprs(part.children.slice(1, -1), level),
                      },
                ),
              value: expression(required(child.children.at(-1)), level),
            })),
        };
      case "N_ActionExpr":
        return {
          kind: children[0]?.image === "[" ? "BoxAction" : "AngleAction",
          action: expression(required(children[1]), level),
          subscript: expression(required(children[3]), level),
        };
      case "N_FairnessExpr":
        return {
          kind:
            children[0]?.image === "WF_" ? "WeakFairness" : "StrongFairness",
          subscript: expression(required(children[1]), level),
          action: expression(required(children[3]), level),
        };
      case "N_Label": {
        const target = record(expression(required(children[0]), level));
        return {
          kind: "Label",
          name: target.name ?? target.operator,
          params: target.args
            ? array(target.args).map((item) => record(item).name)
            : [],
          body: expression(required(children[2]), level),
        };
      }
      case "N_Lambda": {
        const colon = children.findIndex((child) => child.image === ":");
        return {
          kind: "Lambda",
          params: children
            .slice(1, colon)
            .filter((child) => child.image !== ",")
            .map((child) => declaration(child.image)),
          body: expression(required(children.at(-1)), level),
        };
      }
      case "N_LetIn": {
        const entries = items(required(children[1]).children);
        return {
          kind: "Let",
          definitions: entries.definitions,
          instances: entries.instances,
          recursive: entries.recursive,
          body: expression(required(children[3]), level),
        };
      }
      case "N_AssumeProve": {
        const prove = children.findIndex((child) =>
          ["PROVE", "[]PROVE"].includes(child.image),
        );
        assert.ok(prove > 0);
        return {
          kind: "AssumeProve",
          assumptions: children
            .slice(1, prove)
            .filter((child) => child.image !== ",")
            .map((child) =>
              child.image === "N_NewSymb"
                ? expression(child, level)
                : { kind: "Expression", expression: expression(child, level) },
            ),
          prove: expression(required(children[prove + 1]), level),
          isBoxedAssume: children[0]?.image !== "ASSUME",
          isBoxedProve: children[prove]?.image !== "PROVE",
        };
      }
      case "N_NewSymb": {
        const declNode = children.find((child) => child.image.endsWith("Decl"));
        assert.ok(declNode);
        const name =
          children.find((child) =>
            ["CONSTANT", "VARIABLE", "STATE", "ACTION", "TEMPORAL"].includes(
              child.image,
            ),
          )?.image ?? "CONSTANT";
        const domain = children.findIndex(
          (child) => spelling(child.image) === "\\in",
        );
        return {
          kind: "New",
          declaration: decl(declNode),
          level: name[0] + name.slice(1).toLowerCase(),
          isNew: children[0]?.image === "NEW",
          ...(domain >= 0
            ? { set: expression(required(children[domain + 1]), level) }
            : {}),
        };
      }
      default:
        if (node.image.startsWith('"'))
          return { kind: "String", value: node.image.slice(1, -1) };
        if (node.image === "@") return { kind: "ExceptAt" };
        throw new Error(`Unnormalized expression ${node.image}`);
    }
  };
  return normalizeRtla(expression(validate(tree)));
};
