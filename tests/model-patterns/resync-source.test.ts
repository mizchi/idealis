import assert from "node:assert/strict";
import { once } from "node:events";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Origin = "Primary" | "Replacement";
type Ticket = "First" | "Second";
type Image = "OldData" | "NewData" | "LaterData";
type Snapshot = {
  readonly origin: Origin;
  readonly offset: number;
  readonly image: Image;
};
type Command =
  | { readonly kind: "switchSource" | "advance" }
  | { readonly kind: "request" | "publish"; readonly ticket: Ticket }
  | {
      readonly kind: "receive";
      readonly ticket: Ticket;
      readonly source: Origin;
    };

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/resync-source.model";

async function endpoint(t: TestContext, initial: Snapshot, next?: Snapshot) {
  const server = createServer((request, response) => {
    const url = new URL(request.url ?? "/", "http://localhost");
    if (url.pathname === "/snapshot") response.end(JSON.stringify(initial));
    else if (
      url.pathname === "/next" &&
      next &&
      url.searchParams.get("after") === String(initial.offset)
    )
      response.end(JSON.stringify(next));
    else response.writeHead(404).end();
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(async () => {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  return `http://127.0.0.1:${address.port}`;
}

async function readSnapshot(url: string): Promise<Snapshot> {
  const response = await fetch(url);
  assert.equal(response.status, 200);
  return (await response.json()) as Snapshot;
}

test("a replica compares offsets only within the same source generation", async (t) => {
  const primary: Snapshot = { origin: "Primary", offset: 2, image: "OldData" };
  const replacement: Snapshot = {
    origin: "Replacement",
    offset: 1,
    image: "NewData",
  };
  const later: Snapshot = {
    origin: "Replacement",
    offset: 2,
    image: "LaterData",
  };
  const urls = {
    Primary: await endpoint(t, primary),
    Replacement: await endpoint(t, replacement, later),
  };
  const short = await reachableScenario<Command>(
    checker,
    model,
    "ReplacementReadyToPublishEarlierOffset",
  );
  const sameOffset = await reachableScenario<Command>(
    checker,
    model,
    "WrongOriginAtSameOffset",
  );
  const scenarios = [
    {
      base: short,
      ticket: "First",
      fault: "compare-across-origins",
      expected: replacement,
      observed: null,
    },
    {
      base: sameOffset,
      ticket: "Second",
      fault: "ignore-origin",
      expected: later,
      observed: null,
    },
  ] as const;

  for (const scenario of scenarios) {
    for (const fault of ["none", scenario.fault] as const) {
      let active: Origin = "Primary";
      let saved = await readSnapshot(`${urls.Primary}/snapshot`);
      let latest: Ticket | undefined;
      const inbox = new Map<Ticket, Snapshot>();
      const visible = () => (saved.origin === active ? saved : null);
      const command = { kind: "publish", ticket: scenario.ticket } as const;
      const commands = [...scenario.base.commands, command];
      const result = await recordReplay<Command>(
        checker,
        { ...scenario.base, commands },
        {
          async apply(input) {
            switch (input.kind) {
              case "switchSource":
                active = "Replacement";
                break;
              case "request":
                latest = input.ticket;
                break;
              case "receive":
                inbox.set(
                  input.ticket,
                  await readSnapshot(`${urls[input.source]}/snapshot`),
                );
                break;
              case "publish": {
                const image = inbox.get(input.ticket);
                assert.ok(image);
                inbox.delete(input.ticket);
                if (
                  input.ticket !== latest ||
                  (fault !== "ignore-origin" && image.origin !== active)
                )
                  break;
                if (
                  (saved.origin !== image.origin &&
                    fault !== "compare-across-origins") ||
                  image.offset >= saved.offset
                )
                  saved = image;
                break;
              }
              case "advance": {
                const current = visible();
                assert.ok(current);
                saved = await readSnapshot(
                  `${urls[active]}/next?after=${current.offset}`,
                );
                break;
              }
            }
          },
          async observe() {
            return visible();
          },
          project: ({ active, saved }) => {
            const snapshot = saved as Snapshot;
            return snapshot.origin === active ? snapshot : null;
          },
        },
      );
      if (fault === "none") {
        assert.equal(result.verdict, "matched");
        assert.equal(
          result.recording.observations[
            commands.findIndex(({ kind }) => kind === "switchSource") + 1
          ],
          null,
        );
        assert.deepEqual(
          result.recording.observations.at(-1),
          scenario.expected,
        );
      } else {
        assert.equal(result.verdict, "mismatch");
        assert.deepEqual(result.difference, {
          step: commands.length,
          command,
          expected: scenario.expected,
          observed: scenario.observed,
        });
      }
    }
  }
});
