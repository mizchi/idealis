import assert from "node:assert/strict";
import { bindings } from "../smt-subset/clients.js";
import type {
  Assumption,
  CheckCache,
  Reply,
  Request,
  Solution,
} from "../smt-subset/wire.js";

/** One request at a time, so a reply can feed the next request. */
export type Client = (request: Request) => Promise<Reply>;
type Operation =
  | { readonly kind: "assert"; readonly expression: string }
  | { readonly kind: "push"; readonly count: number }
  | { readonly kind: "pop"; readonly count: number }
  | {
      readonly kind: "check";
      readonly assumptions: readonly Assumption[];
      readonly maxWork: number;
    };
export interface ContextSequence {
  readonly name: string;
  readonly declarations: string;
  readonly operations: readonly Operation[];
}

const declarations =
  "(declare-const x Int)(declare-const y Int)(declare-const z Int)(declare-const p Bool)(declare-const q Bool)";
const numeral = (value: number) =>
  value < 0 ? `(- ${-value})` : String(value);
// Later integer equalities rewrite earlier assertions; rejected and ill-typed
// expressions must leave the context unchanged.
const templates: readonly ((c: string) => string)[] = [
  (c) => `(<= (- x y) ${c})`,
  () => "(< y z)",
  (c) => `(= y ${c})`,
  (c) => `(= z ${c})`,
  (c) => `(= x ${c})`,
  (c) => `(= x (+ y ${c}))`,
  (c) => `(and (= y ${c}) (<= x 9))`,
  () => "(or p (<= x y))",
  () => "(=> q (> z x))",
  (c) => `(= (ite p x y) (+ z ${c}))`,
  (c) => `(let ((w (- z ${c}))) (>= w y))`,
  () => "(xor p q)",
  () => "(distinct x y z)",
  (c) => `(<= (* 2 (- x y)) ${c})`,
  (c) => `(<= (+ x y) ${c})`,
  (c) => `(= (* x y) ${c})`,
  (c) => `(not (= y ${c}))`,
  () => "false",
  () => "(+ x 1)",
  () => "(< x undeclared)",
];
const symbols = ["p", "q", "x", "missing"];
export const maxWork = 2_000_000;

export const sequences = (
  seed: number,
  count: number,
): readonly ContextSequence[] => {
  let state = seed >>> 0;
  const next = (bound: number) => {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return (state >>> 16) % bound;
  };
  return Array.from({ length: count }, (_, index) => ({
    name: `context-${seed}-${index}`,
    declarations,
    operations: Array.from({ length: 24 }, (): Operation => {
      const choice = next(6);
      if (choice < 2)
        return {
          kind: "assert",
          expression: templates[next(templates.length)]!(numeral(next(19) - 9)),
        };
      if (choice === 2) return { kind: "push", count: next(3) };
      if (choice === 3) return { kind: "pop", count: next(3) };
      return {
        kind: "check",
        assumptions: Array.from({ length: next(3) }, () => ({
          symbol: symbols[next(symbols.length)]!,
          positive: next(2) === 0,
        })),
        maxWork: [0, 7, 40, maxWork][next(4)]!,
      };
    }),
  }));
};

const request = (handle: number, operation: Operation): Request =>
  operation.kind === "assert"
    ? { kind: "assert", handle, expression: operation.expression }
    : operation.kind === "check"
      ? {
          kind: "check",
          handle,
          maxWork: operation.maxWork,
          assumptions: operation.assumptions,
        }
      : { kind: operation.kind, handle, count: operation.count };

const prepared = async (client: Client, sequence: ContextSequence) => {
  const reply = await client({
    kind: "prepare",
    source: sequence.declarations,
  });
  if (reply.kind !== "prepared") throw new Error(JSON.stringify(reply));
  return reply.handle;
};

/** Replies of one sequence, with the engine-specific handle number removed. */
const transcript = async (client: Client, sequence: ContextSequence) => {
  const handle = await prepared(client, sequence);
  const replies: Reply[] = [];
  for (const operation of sequence.operations)
    replies.push(await client(request(handle, operation)));
  assert.equal((await client({ kind: "release", handle })).kind, "released");
  return replies;
};

type Z3 = { readonly script: (source: string) => Promise<string> };
const literal = ({ symbol, positive }: Assumption) =>
  positive ? symbol : `(not ${symbol})`;

/**
 * Compares every engine on generated context sequences, checks complete
 * answers and SAT models with Z3, then replays the sequences through the
 * check cache of each cache client.
 */
export const verifyContexts = async (
  inputs: readonly ContextSequence[],
  clients: Readonly<Record<string, Client>>,
  cacheClients: Readonly<Record<string, Client>>,
  z3: Z3,
  buildDigest: string,
  onRecord: (record: unknown) => void,
) => {
  const counts = {
    contexts: inputs.length,
    contextRequests: 0,
    contextChecks: 0,
    z3Statuses: 0,
    z3Models: 0,
    contextErrors: 0,
    contextUnknown: 0,
    contextCacheHits: 0,
    contextCacheMisses: 0,
  };
  const caches: Record<string, CheckCache | undefined> = {};
  for (const sequence of inputs) {
    const replies: Record<string, Reply[]> = {};
    for (const [name, client] of Object.entries(clients))
      replies[name] = await transcript(client, sequence);
    const [reference, ...others] = Object.entries(replies);
    if (!reference) throw new Error("No context clients");
    for (const [name, other] of others)
      assert.deepEqual(other, reference[1], `${sequence.name} ${name}`);
    onRecord({ context: sequence, replies: reference[1] });
    counts.contextRequests += sequence.operations.length;

    // Z3 sees the assertions the engines accepted, scoped like the context.
    let asserted: string[] = [];
    const frames: string[][] = [];
    for (const [index, operation] of sequence.operations.entries()) {
      const reply: Reply = reference[1][index]!;
      if (reply.kind === "error") {
        counts.contextErrors++;
        continue;
      }
      if (operation.kind === "assert")
        asserted = [...asserted, operation.expression];
      else if (operation.kind === "push")
        for (let step = 0; step < operation.count; step++)
          frames.push(asserted);
      else if (operation.kind === "pop") {
        if (operation.count > 0) {
          asserted = frames[frames.length - operation.count]!;
          frames.length -= operation.count;
        }
      } else {
        counts.contextChecks++;
        if (reply.kind !== "solution") throw new Error(JSON.stringify(reply));
        if (reply.result.status === "unknown") {
          counts.contextUnknown++;
          continue;
        }
        const body =
          sequence.declarations +
          asserted.map((expression) => `(assert ${expression})`).join("");
        const query = `${body}(check-sat-assuming (${operation.assumptions.map(literal).join(" ")}))`;
        const status = (await z3.script(query)).trim();
        assert.equal(status, reply.result.status, `${sequence.name} ${index}`);
        counts.z3Statuses++;
        let validation: string | undefined;
        if (reply.result.status === "sat") {
          validation = (
            await z3.script(
              `${body}${operation.assumptions.map((value) => `(assert ${literal(value)})`).join("")}${bindings(reply.result)}(check-sat)`,
            )
          ).trim();
          assert.equal(validation, "sat", `${sequence.name} ${index} model`);
          counts.z3Models++;
        }
        onRecord({ oracle: sequence.name, index, query, status, validation });
      }
    }

    // The cache must not change misses and must return fresh answers on hits.
    const cached: Record<string, unknown[]> = {};
    for (const [name, client] of Object.entries(cacheClients)) {
      const handle = await prepared(client, sequence);
      const log: unknown[] = [];
      for (const [index, operation] of sequence.operations.entries()) {
        const plain: Reply = reference[1][index]!;
        if (operation.kind !== "check") {
          log.push(await client(request(handle, operation)));
          continue;
        }
        const reply: Reply = await client({
          kind: "check-incremental",
          handle,
          maxWork: operation.maxWork,
          assumptions: operation.assumptions,
          enabled: true,
          buildDigest,
          maxEntries: 4096,
          ...(caches[name] ? { cache: caches[name] } : {}),
        });
        log.push(reply);
        if (reply.kind === "error") {
          assert.deepEqual(reply, plain, `${sequence.name} ${index} cache`);
          continue;
        }
        if (reply.kind !== "incremental-solution" || plain.kind !== "solution")
          throw new Error(JSON.stringify(reply));
        caches[name] = reply.cache;
        if (reply.stats.hits === 1) {
          const fresh = await client(
            request(handle, { ...operation, maxWork }),
          );
          if (fresh.kind !== "solution") throw new Error(JSON.stringify(fresh));
          const { stats, ...answer } = reply.result;
          const { stats: _, ...expected } = fresh.result as Solution;
          assert.deepEqual(answer, expected, `${sequence.name} ${index} hit`);
          assert.deepEqual(stats, { work: 0, branches: 0, relaxations: 0 });
          if (name === Object.keys(cacheClients)[0]) counts.contextCacheHits++;
        } else {
          assert.deepEqual(
            reply.result,
            plain.result,
            `${sequence.name} ${index} miss`,
          );
          if (name === Object.keys(cacheClients)[0])
            counts.contextCacheMisses++;
        }
      }
      assert.equal(
        (await client({ kind: "release", handle })).kind,
        "released",
      );
      cached[name] = log;
    }
    const [first, ...rest] = Object.values(cached);
    for (const log of rest) assert.deepEqual(log, first, sequence.name);
    onRecord({ contextCache: sequence.name, replies: first });
  }
  return counts;
};
