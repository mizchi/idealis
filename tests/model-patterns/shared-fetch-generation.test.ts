import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Generation = "Old" | "Renewed" | "Unexpected";
type Command =
  | {
      readonly kind: "start" | "abort" | "finish" | "cleanup";
      readonly generation: Exclude<Generation, "Unexpected">;
    }
  | { readonly kind: "attach" };
type Entry = {
  generation: Generation;
  controller: AbortController;
  releaseCleanup(): void;
  outcome: Promise<string | null>;
  cleaned: Promise<void>;
};
const checker = "crates/idealis-check/target/debug/idealis-check";
const model =
  "examples/model-patterns/cancellation/shared-fetch-generation.ideal";

test(
  "a stale fetch finalizer cannot delete the new HTTP request for its key",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "ReplacementRegisteredBeforeOldCleanup",
    );
    assert.deepEqual(base.commands, [
      { kind: "start", generation: "Old" },
      { kind: "abort", generation: "Old" },
      { kind: "start", generation: "Renewed" },
    ]);
    const commands: Command[] = [
      ...base.commands,
      { kind: "cleanup", generation: "Old" },
      { kind: "attach" },
      { kind: "finish", generation: "Renewed" },
      { kind: "cleanup", generation: "Renewed" },
    ];
    for (const deleteByKey of [false, true]) {
      await t.test(
        deleteByKey ? "delete by key" : "compare owner",
        async (t) => {
          const responses = new Map<Generation, ServerResponse>();
          const closed = new Map<Generation, Promise<unknown>>();
          const requested = new EventEmitter();
          let requests = 0;
          const server = createServer((request, response) => {
            assert.equal(request.url, "/shared");
            const generation = request.headers[
              "x-test-generation"
            ] as Generation;
            requests += 1;
            responses.set(generation, response);
            closed.set(generation, once(response, "close"));
            requested.emit(generation);
          });
          server.listen(0, "127.0.0.1");
          await once(server, "listening");
          t.after(async () => {
            server.closeAllConnections();
            await new Promise<void>((resolve) => server.close(() => resolve()));
          });
          const address = server.address();
          assert.ok(address && typeof address !== "string");
          const url = `http://127.0.0.1:${address.port}/shared`;
          const entries = new Map<Generation, Entry>();
          const registry = new Map<string, Entry>();
          const settled = new Set<Generation>();
          const pendingCleanup = new Set<Generation>();
          let attached: Entry | undefined;
          const acquire = async (generation: Generation): Promise<Entry> => {
            const existing = registry.get(url);
            if (existing) return existing;
            const ready = once(requested, generation);
            const controller = new AbortController();
            let releaseCleanup!: () => void;
            const cleanup = new Promise<void>((resolve) => {
              releaseCleanup = resolve;
            });
            const outcome = fetch(url, {
              signal: controller.signal,
              headers: { "x-test-generation": generation },
            }).then(
              async (response) => {
                assert.equal(response.status, 200);
                return await response.text();
              },
              (error: unknown) => {
                assert.ok(
                  error instanceof DOMException && error.name === "AbortError",
                );
                return null;
              },
            );
            const cleaned = outcome
              .finally(async () => {
                settled.add(generation);
                pendingCleanup.add(generation);
                await cleanup;
                if (deleteByKey || registry.get(url) === entry)
                  registry.delete(url);
                pendingCleanup.delete(generation);
              })
              .then(() => {});
            const entry: Entry = {
              generation,
              controller,
              releaseCleanup,
              outcome,
              cleaned,
            };
            void cleaned.catch(() => {});
            entries.set(generation, entry);
            registry.set(url, entry);
            await Promise.race([ready, outcome]);
            return entry;
          };
          const adapter = {
            async apply(command: Command) {
              if (command.kind === "attach") {
                attached = await acquire("Unexpected");
                return;
              }
              const generation = command.generation;
              if (command.kind === "start") {
                await acquire(generation);
                return;
              }
              const entry = entries.get(generation);
              assert.ok(entry);
              if (command.kind === "abort") {
                assert.equal(registry.get(url), entry);
                registry.delete(url);
                entry.controller.abort();
                assert.equal(await entry.outcome, null);
                await closed.get(generation);
              } else if (command.kind === "finish") {
                const response = responses.get(generation);
                assert.ok(response);
                response.end(`body:${generation}`);
                assert.equal(await entry.outcome, `body:${generation}`);
              } else {
                entry.releaseCleanup();
                await entry.cleaned;
              }
            },
            async observe() {
              return {
                registered: registry.get(url)?.generation ?? null,
                settled: [...settled].sort(),
                pendingCleanup: [...pendingCleanup].sort(),
                attached: attached?.generation ?? null,
                requests,
              };
            },
            project: ({
              registered,
              settled,
              pendingCleanup,
              attached,
              started,
            }: Readonly<Record<string, unknown>>) => ({
              registered,
              settled,
              pendingCleanup,
              attached,
              requests: (started as unknown[]).length,
            }),
          };

          const result = await recordReplay<Command>(
            checker,
            { ...base, commands },
            adapter,
          );
          if (deleteByKey) {
            assert.equal(result.verdict, "mismatch");
            assert.equal(result.difference.step, 4);
            assert.deepEqual(result.difference.observed, {
              registered: null,
              settled: ["Old"],
              pendingCleanup: [],
              attached: null,
              requests: 2,
            });
            await adapter.apply({ kind: "attach" });
            assert.equal((await adapter.observe()).requests, 3);
          } else {
            assert.equal(result.verdict, "matched");
            assert.equal(await attached?.outcome, "body:Renewed");
            assert.deepEqual(result.recording.observations.at(-1), {
              registered: null,
              settled: ["Old", "Renewed"],
              pendingCleanup: [],
              attached: "Renewed",
              requests: 2,
            });
          }
        },
      );
    }
  },
);
