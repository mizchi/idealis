import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer, type ServerResponse } from "node:http";
import test from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";
import { readPinnedDelivery } from "../../examples/model-patterns/subscriptions/delivery-stream.js";

type Attempt = "Primary" | "Hedge";
type Answer = "Retry" | "Expired" | "Delivered" | "Missing";
type Command =
  | { kind: "connect"; attempt: Attempt }
  | { kind: "finish"; attempt: Attempt; answer: Answer }
  | { kind: "restart" };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/subscriptions/delivery-generation.model";
const id = "notice-1";

test(
  "only responses owned by the active observation can settle it",
  { timeout: 30_000 },
  async (t) => {
    const expired = await reachableScenario<Command>(
      checker,
      model,
      "PrunedVersionReopened",
    );
    const sameVersion = await reachableScenario<Command>(
      checker,
      model,
      "SameVersionReopened",
    );
    const lateRetry: Command = {
      kind: "finish",
      attempt: "Primary",
      answer: "Retry",
    };
    const lateNotice: Command = {
      kind: "finish",
      attempt: "Primary",
      answer: "Delivered",
    };
    const fresh: Command = {
      kind: "finish",
      attempt: "Hedge",
      answer: "Missing",
    };
    const freshNotice: Command = {
      kind: "finish",
      attempt: "Hedge",
      answer: "Delivered",
    };
    const cases = [
      {
        name: "old retry before fresh result",
        prefix: expired,
        tail: [lateRetry, fresh],
        fault: null,
        revision: 2,
      },
      {
        name: "old retry after fresh result",
        prefix: expired,
        tail: [fresh, lateRetry],
        fault: null,
        revision: 2,
      },
      {
        name: "old notice at the same version",
        prefix: sameVersion,
        tail: [lateNotice, freshNotice],
        fault: null,
        revision: 1,
      },
      {
        name: "old retry without an ownership check",
        prefix: expired,
        tail: [lateRetry, fresh],
        fault: "ignore-owner",
        revision: 2,
      },
      {
        name: "old notice with a version check alone",
        prefix: sameVersion,
        tail: [lateNotice, freshNotice],
        fault: "version-only",
        revision: 1,
      },
    ] as const;

    for (const { name, prefix, tail, fault, revision } of cases) {
      await t.test(name, async (t) => {
        type Observation = {
          pinned: number;
          generation: "Original" | "Renewed";
          result: "Waiting" | Answer;
        };
        let current: Observation = {
          pinned: 1,
          generation: "Original",
          result: "Waiting",
        };
        const writers = new Map<
          string,
          { response: ServerResponse; revision: number }
        >();
        const requests = new Map<
          Attempt,
          {
            owner: Observation;
            response: Promise<Response>;
          }
        >();
        const requested: number[] = [];
        const server = createServer((request, response) => {
          const params = new URL(request.url ?? "/", "http://localhost")
            .searchParams;
          const attempt = params.get("attempt");
          if (!attempt || writers.has(attempt) || params.get("id") !== id) {
            response.writeHead(400).end();
            return;
          }
          const revision = Number(params.get("revision"));
          requested.push(revision);
          writers.set(attempt, { response, revision });
        });
        server.listen(0, "127.0.0.1");
        await once(server, "listening");
        t.after(async () => {
          const closed = new Promise<void>((resolve, reject) =>
            server.close((error) => (error ? reject(error) : resolve())),
          );
          server.closeAllConnections();
          await closed;
        });
        const address = server.address();
        assert.ok(address && typeof address !== "string");

        const result = await recordReplay(
          checker,
          {
            ...prefix,
            commands: [...prefix.commands, ...tail],
          },
          {
            async apply(command) {
              if (command.kind === "restart") {
                current = {
                  pinned: current.result === "Expired" ? 2 : current.pinned,
                  generation: "Renewed",
                  result: "Waiting",
                };
                return;
              }
              const { attempt } = command;
              if (command.kind === "connect") {
                const entered = once(server, "request");
                const response = fetch(
                  `http://127.0.0.1:${address.port}/?id=${id}&attempt=${attempt}&revision=${current.pinned}`,
                  { signal: AbortSignal.timeout(10_000) },
                );
                // The old response may outlive its observation. Cleanup can abort it.
                void response.catch(() => {});
                requests.set(attempt, { owner: current, response });
                await Promise.race([entered, response]);
                assert.ok(writers.has(attempt));
                return;
              }
              const request = requests.get(attempt);
              const sent = writers.get(attempt);
              assert.ok(request && sent);
              const writer = sent.response;
              if (command.answer === "Expired" || command.answer === "Retry") {
                writer
                  .writeHead(command.answer === "Expired" ? 410 : 503)
                  .end();
              } else {
                writer.writeHead(200, {
                  "Content-Type": "application/x-ndjson",
                  "X-Delivery-Revision": String(sent.revision),
                });
                const frames =
                  command.answer === "Delivered" ? ["notice", "end"] : ["end"];
                writer.end(
                  frames
                    .map((kind) => JSON.stringify({ kind, id }) + "\n")
                    .join(""),
                );
              }
              const answer = await readPinnedDelivery(
                await request.response,
                id,
                request.owner.pinned,
              );
              assert.ok(answer !== "WrongRevision");
              requests.delete(attempt);
              writers.delete(attempt);
              const belongs =
                fault === "ignore-owner" ||
                (fault === "version-only"
                  ? request.owner.pinned === current.pinned
                  : request.owner === current);
              if (
                belongs &&
                (current.result === "Waiting" || current.result === "Retry")
              ) {
                current.result = answer;
              }
            },
            async observe() {
              return {
                pinned: current.pinned,
                generation: current.generation,
                result: current.result,
                outstanding: {
                  Primary: requests.get("Primary")?.owner.generation ?? null,
                  Hedge: requests.get("Hedge")?.owner.generation ?? null,
                },
              };
            },
            project: ({ pinned, generation, result, outstanding }) => ({
              pinned,
              generation,
              result,
              outstanding,
            }),
          },
        );
        assert.deepEqual(requested, revision === 1 ? [1, 1] : [1, 1, 2]);
        if (fault) {
          assert.equal(result.verdict, "mismatch");
          assert.equal(result.difference.step, prefix.commands.length + 1);
          assert.deepEqual(result.difference.command, tail[0]);
          assert.equal(
            (result.difference.expected as Observation).result,
            "Waiting",
          );
          assert.equal(
            (result.difference.observed as Observation).result,
            tail[0].answer,
          );
        } else {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations.at(-1), {
            pinned: revision,
            generation: "Renewed",
            result: revision === 1 ? "Delivered" : "Missing",
            outstanding: { Primary: null, Hedge: null },
          });
        }
      });
    }
  },
);
