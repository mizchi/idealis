import { parseModule } from "tla-ts";
import { isRecord } from "../../packages/idealis/src/trace.js";

export const analysisBounds = { maxDepth: 8, maxStates: 100, maxWork: 10_000 };
export const analysisLimits = {
  sourceInputs: 3168,
  positions: 198,
  supplement: 1696,
  total: 4864,
  deadlineMs: 180_000,
  inputMs: 1000,
  bytes: 2048,
  definitions: 16,
  binders: 2,
  recipeDepth: 4,
  calls: 34048,
};

export interface AnalysisPosition {
  readonly constructor: string;
  readonly spelling: string;
  readonly position: string;
  readonly arity?: number;
  readonly id: string;
}
interface Recipe {
  readonly node: string;
  readonly safe: string;
  readonly prefix: string;
  readonly declarations?: string;
  readonly property?: boolean;
  readonly expectedOp: string;
}
export interface AnalysisCase {
  readonly id: string;
  readonly position: AnalysisPosition;
  readonly probe: "old" | "binder" | "target" | "membership";
  readonly context: number;
  readonly control: boolean;
  readonly input: { source: string; config: string; irVersion: 2 };
  readonly sourcePath: string;
  readonly expectedOp: string;
  readonly refusal: string | null;
}
type Builder = (probe: string) => Recipe;
const pair = (node: string, result: string, expectedOp: string): Recipe => ({
  node,
  safe: `(${node}) = (${result})`,
  prefix: "args[0].",
  expectedOp,
});
const direct = (node: string, expectedOp: string): Recipe => ({
  node,
  safe: node,
  prefix: "",
  expectedOp,
});
const num = (p: string) => `(IF ${p} THEN 9 ELSE 8)`;
const set = (p: string) => `(IF ${p} THEN {9} ELSE {8})`;
const seq = (p: string) => `(IF ${p} THEN <<9>> ELSE <<8>>)`;
export const createAnalysisCorpus = (
  entries: readonly { spelling: string; arity: number }[],
) => {
  const builders = new Map<string, Builder>();
  const positions: AnalysisPosition[] = [];
  const add = (
    nodeKind: string,
    spelling: string,
    position: string,
    build: Builder,
    arity?: number,
  ) => {
    const id = [nodeKind, spelling, arity ?? "", position]
      .map(encodeURIComponent)
      .join("/");
    positions.push({
      constructor: nodeKind,
      spelling,
      position,
      id,
      ...(arity === undefined ? {} : { arity }),
    });
    builders.set(id, build);
  };
  for (const [nodeKind, spellings, neutral, op] of [
    ["And", ["/\\", "\\land", "∧"], "TRUE", "and"],
    ["Or", ["\\/", "\\lor", "∨"], "FALSE", "or"],
  ] as const)
    for (const spelling of spellings)
      for (const i of [0, 1])
        add(nodeKind, spelling, `operands[${i}]`, (p) =>
          direct(
            i === 0
              ? `(${p}) ${spelling} ${neutral}`
              : `${neutral} ${spelling} (${p})`,
            op,
          ),
        );
  add("Prime", "'", "expression", (p) => ({
    node: `${num(p)}'`,
    safe: `ENABLED (x' = x /\\ ${num(p)}' = 9)`,
    prefix: "expression.operands[1].args[0].",
    expectedOp: "prime",
  }));
  add("Unchanged", "UNCHANGED", "expression", (p) => ({
    node: `UNCHANGED ${num(p)}`,
    safe: `ENABLED (x' = x /\\ (IF UNCHANGED ${num(p)} THEN UNCHANGED x ELSE UNCHANGED x))`,
    prefix: "expression.operands[1].condition.",
    expectedOp: "eq",
  }));
  add("Enabled", "ENABLED", "expression", (p) =>
    direct(`ENABLED ((${p}) /\\ UNCHANGED x)`, "enabled"),
  );
  for (const [nodeKind, spellings, op] of [
    ["Always", ["[]", "□"], "always"],
    ["Eventually", ["<>", "◇"], "eventually"],
  ] as const)
    for (const spelling of spellings)
      add(nodeKind, spelling, "expression", (p) => ({
        ...direct(`${spelling} (${p})`, op),
        property: true,
      }));
  for (const field of ["condition", "consequent", "alternative"])
    add("If", "", field, (p) =>
      direct(
        `IF ${field === "condition" ? p : field === "consequent" ? "TRUE" : "FALSE"} THEN ${field === "consequent" ? p : "TRUE"} ELSE ${field === "alternative" ? p : "TRUE"}`,
        "if",
      ),
    );
  for (const i of [0, 1])
    for (const field of ["condition", "body"])
      add("Case", "", `arms[${i}].${field}`, (p) =>
        direct(
          `CASE ${i === 0 ? (field === "condition" ? p : "TRUE") : "FALSE"} -> ${i === 0 && field === "body" ? p : "TRUE"} [] ${i === 1 ? (field === "condition" ? p : "TRUE") : "FALSE"} -> ${i === 1 && field === "body" ? p : "TRUE"} [] OTHER -> TRUE`,
          "if",
        ),
      );
  add("Case", "", "other", (p) =>
    direct(`CASE FALSE -> TRUE [] FALSE -> TRUE [] OTHER -> ${p}`, "if"),
  );
  for (const position of ["definitions[0]", "definitions[1]", "body"])
    add("Let", "", position, (p) =>
      direct(
        `LET A == ${position === "definitions[0]" ? p : "TRUE"} C == ${position === "definitions[1]" ? p : "TRUE"} IN ${position === "body" ? p : position === "definitions[0]" ? "A" : "C"}`,
        "call",
      ),
    );
  for (const [nodeKind, spellings, op] of [
    ["Forall", ["\\A", "\\forall", "∀"], "forall"],
    ["Exists", ["\\E", "\\exists", "∃"], "exists"],
  ] as const)
    for (const spelling of spellings)
      for (const position of ["bounds[0].set", "bounds[1].set", "body"])
        add(nodeKind, spelling, position, (p) =>
          direct(
            `${spelling} a \\in ${position === "bounds[0].set" ? set(p) : "{9}"}, b \\in ${position === "bounds[1].set" ? set(p) : "{9}"}: ${position === "body" ? p : "TRUE"}`,
            op,
          ),
        );
  for (const position of ["set", "body"])
    add("Choose", "CHOOSE", position, (p) =>
      pair(
        `CHOOSE a \\in ${position === "set" ? set(p) : "{9}"}: ${position === "body" ? p : "TRUE"}`,
        "9",
        "choose",
      ),
    );
  for (const [nodeKind, spellings, op] of [
    ["SetEnumeration", ["{}"], "set"],
    ["Tuple", ["<<>>", "⟨⟩"], "tuple"],
  ] as const)
    for (const spelling of spellings)
      for (const i of [0, 1])
        add(nodeKind, spelling, `elements[${i}]`, (p) => {
          const body = `${i === 0 ? num(p) : "9"}, ${i === 1 ? num(p) : "9"}`;
          const node =
            nodeKind === "SetEnumeration"
              ? `{${body}}`
              : spelling === "⟨⟩"
                ? `⟨${body}⟩`
                : `<<${body}>>`;
          return pair(
            node,
            nodeKind === "SetEnumeration" ? "{9}" : "<<9,9>>",
            op,
          );
        });
  for (const position of ["bound.set", "predicate"])
    add("SetFilter", "", position, (p) =>
      pair(
        `{a \\in ${position === "bound.set" ? set(p) : "{9}"}: ${position === "predicate" ? p : "TRUE"}}`,
        "{9}",
        "filter",
      ),
    );
  for (const position of ["expression", "bounds[0].set", "bounds[1].set"])
    add("SetMap", "", position, (p) =>
      pair(
        `{${position === "expression" ? num(p) : "9"}: a \\in ${position === "bounds[0].set" ? set(p) : "{9}"}, b \\in ${position === "bounds[1].set" ? set(p) : "{9}"}}`,
        "{9}",
        "map-set",
      ),
    );
  for (const position of ["bounds[0].set", "bounds[1].set", "body"])
    add("FunctionConstruction", "|->", position, (p) => ({
      node: `[a \\in ${position === "bounds[0].set" ? set(p) : "{9}"}, b \\in ${position === "bounds[1].set" ? set(p) : "{9}"} |-> ${position === "body" ? num(p) : "9"}]`,
      safe: `([a \\in ${position === "bounds[0].set" ? set(p) : "{9}"}, b \\in ${position === "bounds[1].set" ? set(p) : "{9}"} |-> ${position === "body" ? num(p) : "9"}])[9,9] = 9`,
      prefix: "args[0].function.",
      expectedOp: "function",
    }));
  for (const position of ["domain", "range"])
    add("FunctionSet", "->", position, (p) =>
      pair(
        `[${position === "domain" ? set(p) : "{9}"} -> ${position === "range" ? set(p) : "{9}"}]`,
        "[{9} -> {9}]",
        "function-set",
      ),
    );
  for (const nodeKind of ["Record", "RecordSet"])
    for (const i of [0, 1])
      add(
        nodeKind,
        nodeKind === "Record" ? "|->" : ":",
        `fields[${i}].value`,
        (p) => {
          const separator = nodeKind === "Record" ? "|->" : ":";
          const value = nodeKind === "Record" ? num : set;
          return pair(
            `[a ${separator} ${i === 0 ? value(p) : nodeKind === "Record" ? "9" : "{9}"}, b ${separator} ${i === 1 ? value(p) : nodeKind === "Record" ? "9" : "{9}"}]`,
            nodeKind === "Record" ? "[a |-> 9, b |-> 9]" : "[a: {9}, b: {9}]",
            nodeKind === "Record" ? "record" : "record-set",
          );
        },
      );
  const exceptBase = "[t \\in {<<9,9>>} |-> [u \\in {<<9,9>>} |-> 9]]";
  for (const position of [
    "function",
    ...[0, 1].flatMap((i) =>
      [0, 1].flatMap((j) =>
        [0, 1].map((k) => `updates[${i}].path[${j}].Index.args[${k}]`),
      ),
    ),
    "updates[0].value",
    "updates[1].value",
  ])
    add("Except", "EXCEPT", position, (p) => {
      const base =
        position === "function"
          ? `(IF ${p} THEN ${exceptBase} ELSE ${exceptBase})`
          : exceptBase;
      const updates = [0, 1].map(
        (i) =>
          `![${[0, 1].map((k) => (position === `updates[${i}].path[0].Index.args[${k}]` ? num(p) : "9")).join(",")}][${[0, 1].map((k) => (position === `updates[${i}].path[1].Index.args[${k}]` ? num(p) : "9")).join(",")}] = ${position === `updates[${i}].value` ? num(p) : "9"}`,
      );
      const node = `[${base} EXCEPT ${updates.join(", ")}]`;
      return {
        node,
        safe: `(${node})[9,9][9,9] = 9`,
        prefix: "args[0].function.function.",
        expectedOp: "update",
      };
    });
  for (const position of ["function", "args[0]", "args[1]"])
    add("FunctionApplication", "[]", position, (p) =>
      pair(
        `${position === "function" ? `(IF ${p} THEN [a \\in {9}, b \\in {9} |-> 9] ELSE [a \\in {9}, b \\in {9} |-> 8])` : "[a \\in {9}, b \\in {9} |-> 9]"}[${position === "args[0]" ? num(p) : "9"},${position === "args[1]" ? num(p) : "9"}]`,
        "9",
        "at",
      ),
    );
  add("FieldAccess", ".", "record", (p) =>
    pair(`(IF ${p} THEN [a |-> 9] ELSE [a |-> 8]).a`, "9", "at"),
  );
  for (const [nodeKind, spellings] of [
    ["BoxAction", ["[]_"]],
    ["AngleAction", ["<<>>_", "⟨>>_"]],
  ] as const)
    for (const spelling of spellings)
      for (const position of ["action", "subscript"])
        add(nodeKind, spelling, position, (p) => {
          const action =
            position === "action"
              ? p
              : nodeKind === "BoxAction"
                ? "FALSE"
                : "TRUE";
          const subscript = position === "subscript" ? num(p) : "x";
          const node =
            nodeKind === "BoxAction"
              ? `[${action}]_${subscript}`
              : `${spelling === "⟨>>_" ? "⟨" : "<<"}${action}>>_${subscript}`;
          return {
            node,
            safe: `ENABLED (x' = x /\\ ((${node}) = ${nodeKind === "BoxAction" ? "TRUE" : "FALSE"}))`,
            prefix: "expression.operands[1].args[0].",
            expectedOp: nodeKind === "BoxAction" ? "or" : "and",
          };
        });
  for (const [nodeKind, spelling] of [
    ["WeakFairness", "WF_"],
    ["StrongFairness", "SF_"],
  ] as const)
    for (const position of ["subscript", "action"])
      add(nodeKind, spelling, position, (p) => ({
        ...direct(
          `${spelling}${position === "subscript" ? num(p) : "x"}(${position === "action" ? `(${p}) /\\ x' = x` : "x' = x"})`,
          nodeKind === "WeakFairness" ? "always" : "or",
        ),
        property: true,
      }));
  add("Label", "::", "body", (p) => direct(`L:: (${p})`, "call"));

  const binary = new Map([
    ["+", ["add", "9", "18"]],
    ["-", ["sub", "9", "0"]],
    ["*", ["mul", "9", "81"]],
    ["#", ["ne", "9", "FALSE"]],
    ["<", ["lt", "9", "FALSE"]],
    ["<=", ["le", "9", "TRUE"]],
    [">", ["gt", "9", "FALSE"]],
    [">=", ["ge", "9", "TRUE"]],
    ["=>", ["implies", "TRUE", "TRUE"]],
    ["\\cup", ["cup", "{9}", "{9}"]],
    ["\\cap", ["cap", "{9}", "{9}"]],
    ["\\", ["difference", "{9}", "{}"]],
    ["\\subseteq", ["subset-eq", "{9}", "TRUE"]],
    ["\\o", ["concat", "<<9>>", "<<9,9>>"]],
    ["<=>", ["eq", "TRUE", "TRUE"]],
  ]);
  const aliases: Record<string, string> = {
    "/=": "#",
    "≠": "#",
    "\\leq": "<=",
    "=<": "<=",
    "≤": "<=",
    "\\geq": ">=",
    "≥": ">=",
    "⇒": "=>",
    "\\union": "\\cup",
    "∪": "\\cup",
    "\\intersect": "\\cap",
    "∩": "\\cap",
    "⊆": "\\subseteq",
    "\\circ": "\\o",
    "\\equiv": "<=>",
    "⇔": "<=>",
    "∈": "\\in",
    "∉": "\\notin",
    "\\lnot": "~",
    "\\neg": "~",
    "¬": "~",
  };
  const addApplyPositions = (
    entries: readonly { spelling: string; arity: number }[],
  ) => {
    if (positions.some((p) => p.constructor === "Apply")) return;
    for (const { spelling, arity } of entries)
      for (let i = 0; i < arity; i++)
        add(
          "Apply",
          spelling,
          `args[${i}]`,
          (p) => {
            const canonical = aliases[spelling] ?? spelling;
            if (canonical === "~>")
              return {
                ...direct(
                  i === 0 ? `(${p}) ~> TRUE` : `TRUE ~> (${p})`,
                  "always",
                ),
                property: true,
              };
            if (spelling === "user call representative")
              return {
                ...direct(
                  `User(${i === 0 ? p : "TRUE"},${i === 1 ? p : "TRUE"})`,
                  "call",
                ),
                declarations: "User(a,b) == a /\\ b",
              };
            if (canonical === "\\in" || canonical === "\\notin")
              return pair(
                `${i === 0 ? num(p) : "9"} ${spelling} ${i === 1 ? set(p) : "{9}"}`,
                canonical === "\\in" ? "TRUE" : "FALSE",
                canonical === "\\in" ? "in" : "not",
              );
            if (canonical === "Seq")
              return {
                node: `Seq(${set(p)})`,
                safe: `<<9>> \\in Seq(${set(p)})`,
                prefix: "args[1].",
                expectedOp: "sequence-in",
              };
            if (canonical === "=")
              return direct(
                `${i === 0 ? num(p) : "9"} = ${i === 1 ? num(p) : "9"}`,
                "eq",
              );
            if (arity === 1) {
              if (canonical === "~")
                return pair(`${spelling} (${p})`, "FALSE", "not");
              if (canonical === "-") return pair(`-${num(p)}`, "-9", "neg");
              const unary: Record<string, [string, string, string]> = {
                DOMAIN: [
                  "domain",
                  `(IF ${p} THEN [a |-> 9] ELSE [a |-> 8])`,
                  '{"a"}',
                ],
                Cardinality: ["cardinality", set(p), "1"],
                Len: ["len", seq(p), "1"],
                Head: ["head", seq(p), "9"],
                Tail: ["tail", seq(p), "<<>>"],
                SUBSET: ["powerset", set(p), "{{},{9}}"],
                UNION: ["union", `(IF ${p} THEN {{9}} ELSE {{8}})`, "{9}"],
              };
              const u = unary[canonical];
              if (!u) throw new Error(`Missing unary ${canonical}`);
              return pair(`${spelling} (${u[1]})`, u[2], u[0]);
            }
            if (canonical === "..")
              return pair(
                `${i === 0 ? num(p) : "9"} .. ${i === 1 ? num(p) : "9"}`,
                "{9}",
                "map-set",
              );
            if (canonical === "Append")
              return pair(
                `Append(${i === 0 ? seq(p) : "<<9>>"}, ${i === 1 ? num(p) : "9"})`,
                "<<9,9>>",
                "append",
              );
            if (canonical === "SubSeq")
              return pair(
                `SubSeq(${i === 0 ? seq(p) : "<<9>>"}, ${i === 1 ? `(IF ${p} THEN 1 ELSE 0)` : "1"}, ${i === 2 ? `(IF ${p} THEN 1 ELSE 0)` : "1"})`,
                "<<9>>",
                "subseq",
              );
            if (canonical.startsWith("ApaFold"))
              return {
                ...pair(
                  `${spelling}(Fold, ${i === 1 ? num(p) : "9"}, ${i === 2 ? (canonical === "ApaFoldSet" ? set(p) : seq(p)) : canonical === "ApaFoldSet" ? "{9}" : "<<9>>"})`,
                  "9",
                  "fold",
                ),
                declarations: `Fold(a,b) == ${i === 0 ? num(p) : "a"}`,
              };
            const row = binary.get(canonical);
            if (!row) throw new Error(`Missing binary ${spelling}`);
            const wrap =
              row[1] === "TRUE"
                ? (v: string) => `(${v})`
                : row[1] === "{9}"
                  ? set
                  : row[1] === "<<9>>"
                    ? seq
                    : num;
            return pair(
              `${i === 0 ? wrap(p) : row[1]} ${spelling} ${i === 1 ? wrap(p) : row[1]}`,
              row[2] ?? "TRUE",
              row[0] ?? "",
            );
          },
          arity,
        );
  };
  const analysisPositions = (): readonly AnalysisPosition[] => positions;
  const generateAnalysisCases = (): readonly AnalysisCase[] =>
    positions.flatMap((position) =>
      (["old", "binder", "target", "membership"] as const).flatMap((probe) =>
        [0, 1].flatMap((context) =>
          [false, true].map((control): AnalysisCase => {
            if (position.spelling === "~>") {
              const side = Number(position.position[5]);
              const formula = (value: string) =>
                side === 0 ? `(${value}) ~> TRUE` : `TRUE ~> (${value})`;
              let declarations = "";
              let definitions = "";
              const sourcePath = position.position;
              if (probe === "old") {
                definitions = `Safe == [<<9>> EXCEPT ![1] = LET Inspect${context === 0 ? "" : "(a)"} == ${formula(control ? "9 = 9" : "@ = 9")} IN TRUE][1]`;
              } else {
                let atom = "";
                let actual = "9";
                if (probe === "binder") {
                  declarations = "Hit(v) == v = 9";
                  atom = `(\\A n \\in {${context === 0 ? "9" : "p"}}: Hit(${control ? "9" : "n"})) = TRUE`;
                } else if (probe === "target") {
                  declarations = "Can(v) == ENABLED (UNCHANGED v)";
                  actual = "x";
                  atom = control
                    ? `ENABLED (UNCHANGED ${context === 0 ? "x" : "p"})`
                    : `Can(${context === 0 ? "x" : "p"})`;
                } else {
                  actual = control ? "{0}" : "Nat";
                  declarations = context === 0 ? "Member(p) == x \\in p" : "";
                  atom = context === 0 ? `Member(${actual})` : "x \\in p";
                }
                definitions =
                  context === 0
                    ? `Safe == ${formula(atom)}`
                    : `Inspect(p) == ${formula(atom)}\nSafe == Inspect(${actual})`;
              }
              return {
                id: `${position.id}/${probe}/${context}/${control ? "control" : "probe"}`,
                position,
                probe,
                context,
                control,
                input: {
                  source: `---- MODULE AnalysisParity ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\n${declarations}\n${definitions}\n====`,
                  config: `INIT Init\nNEXT Next\n${probe === "old" ? "INVARIANT" : "PROPERTY"} Safe\nCHECK_DEADLOCK FALSE`,
                  irVersion: 2,
                },
                sourcePath,
                expectedOp: "always",
                refusal:
                  position.constructor === "Label" && probe === "old"
                    ? "Unsupported TLA input: labels inside EXCEPT replacements are not supported by TLC"
                    : null,
              };
            }
            const build = builders.get(position.id);
            if (!build) throw new Error(position.id);
            let p =
              probe === "old"
                ? control
                  ? "9 = 9"
                  : "@ = 9"
                : probe === "binder"
                  ? `Hit(${control ? "9" : "n"})`
                  : probe === "target"
                    ? control
                      ? "ENABLED (UNCHANGED p)"
                      : "Can(p)"
                    : "0 \\in p";
            let recipe = build(p);
            if (recipe.property && probe === "binder") {
              p = `(\\A n \\in {9}: Hit(${control ? "9" : "n"})) = TRUE`;
              recipe = build(p);
            }
            const boundOld =
              position.constructor === "Except" &&
              position.position.endsWith(".value");
            const declarations =
              probe === "binder"
                ? "Hit(v) == (\\A n \\in {4}: v = 9)"
                : probe === "target"
                  ? "Can(v) == ENABLED (UNCHANGED v)"
                  : "";
            let inspect = recipe.safe;
            let extra = recipe.declarations ?? "";
            if (
              extra.includes(p) &&
              position.spelling.startsWith("ApaFold") &&
              position.position === "args[0]"
            ) {
              inspect = `LET ${extra} IN ${inspect}`;
              extra = "";
            }
            const unused = probe === "old" && !boundOld;
            const formal = unused ? (context === 0 ? "" : "(p)") : "(p)";
            let definitions: string;
            if (unused)
              definitions = `Safe == [<<9>> EXCEPT ![1] = LET Inspect${formal} == ${inspect} IN TRUE][1]`;
            else {
              const actual =
                probe === "target"
                  ? "x"
                  : probe === "membership"
                    ? control
                      ? "{0}"
                      : "Nat"
                    : "9";
              const call =
                probe === "binder" && !recipe.property
                  ? `(\\A n \\in {9}: Inspect(n)) = TRUE`
                  : `Inspect(${actual})`;
              const scoped =
                probe === "binder" && !recipe.property
                  ? inspect.replaceAll("Hit(n)", "Hit(p)")
                  : inspect;
              definitions =
                context === 0
                  ? `Inspect(p) == ${scoped}\nSafe == ${call}`
                  : `Outer(p) == LET Inspect(p) == ${scoped} IN ${probe === "binder" && !recipe.property ? "(\\A n \\in {9}: Inspect(n)) = TRUE" : "Inspect(p)"}\nSafe == Outer(${actual})`;
            }
            const source = `---- MODULE AnalysisParity ----\nEXTENDS Naturals, Integers, Sequences, FiniteSets, Apalache\nVARIABLE x\nInit == x = 0\nNext == UNCHANGED x\n${declarations}\n${extra}\n${definitions}\n====`;
            return {
              id: `${position.id}/${probe}/${context}/${control ? "control" : "probe"}`,
              position,
              probe,
              context,
              control,
              input: {
                source,
                config: `INIT Init\nNEXT Next\n${recipe.property && !unused ? "PROPERTY" : "INVARIANT"} Safe\nCHECK_DEADLOCK FALSE`,
                irVersion: 2,
              },
              sourcePath:
                (position.spelling.startsWith("ApaFold") &&
                position.position === "args[0]"
                  ? "body."
                  : "") +
                recipe.prefix +
                position.position.replaceAll(".Index", ""),
              expectedOp: recipe.expectedOp,
              refusal:
                position.constructor === "Label" && probe === "old"
                  ? "Unsupported TLA input: labels inside EXCEPT replacements are not supported by TLC"
                  : null,
            };
          }),
        ),
      ),
    );

  addApplyPositions(entries);
  return { positions: analysisPositions(), cases: generateAnalysisCases() };
};

export const sourceObjects = (
  value: unknown,
): readonly Record<string, unknown>[] => {
  const pending = [value];
  const found: Record<string, unknown>[] = [];
  while (pending.length) {
    const current = pending.pop();
    if (Array.isArray(current)) pending.push(...current);
    else if (isRecord(current)) {
      found.push(current);
      for (const [key, child] of Object.entries(current))
        if (key !== "location") pending.push(child);
    }
  }
  return found;
};
export const sourceReach = (entry: AnalysisCase) => {
  const module = parseModule(entry.input.source);
  const inspect = sourceObjects(module).find(
    (node) =>
      node.name ===
        (entry.position.spelling === "~>" &&
        entry.context === 0 &&
        entry.probe !== "old"
          ? "Safe"
          : "Inspect") && "body" in node,
  );
  if (!inspect) throw new Error(`Missing source anchor ${entry.id}`);
  let value: unknown = inspect.body;
  for (const part of entry.sourcePath
    .replaceAll("[", ".")
    .replaceAll("]", "")
    .split(".")
    .filter(Boolean)) {
    if (Array.isArray(value)) value = value[Number(part)];
    else if (isRecord(value)) value = value[part];
    else throw new Error(`Unreached source position ${entry.id}: ${part}`);
  }
  if (
    entry.position.spelling.startsWith("ApaFold") &&
    entry.position.position === "args[0]"
  ) {
    value = sourceObjects(inspect.body).find(
      (node) => node.name === "Fold" && "body" in node,
    )?.body;
  }
  const nodes = sourceObjects(value);
  const sentinel = nodes.filter((node) =>
    entry.probe === "old"
      ? entry.control
        ? node.kind === "Apply" &&
          node.operator === "=" &&
          Array.isArray(node.args) &&
          node.args.every(
            (arg) => isRecord(arg) && arg.kind === "Number" && arg.value === 9,
          )
        : node.kind === "ExceptAt"
      : entry.probe === "binder"
        ? node.kind === "Apply" && node.operator === "Hit"
        : entry.probe === "target"
          ? entry.control
            ? node.kind === "Enabled"
            : node.kind === "Apply" && node.operator === "Can"
          : node.kind === "Apply" &&
            (node.operator === "\\in" ||
              (entry.position.spelling === "~>" &&
                entry.context === 0 &&
                node.operator === "Member")),
  );
  return { module, inspect, value, sentinel };
};
