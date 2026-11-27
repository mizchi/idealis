import assert from "node:assert/strict";
import { EventEmitter, once } from "node:events";
import { createServer } from "node:http";
import test, { type TestContext } from "node:test";
import {
  reachableScenario,
  recordReplay,
} from "../../examples/model-patterns/replay.js";

type Host = "Primary" | "Backup";
type Stamp = { readonly endpoint: Host; readonly instance: number };
type Image = {
  readonly owner: Stamp;
  readonly offset: number;
  readonly contents: "Seed" | "OldData" | "NewData" | "BackupData";
};
type Command = {
  readonly kind:
    | "capture"
    | "moveToBackup"
    | "returnToPrimary"
    | "publish"
    | "read";
};

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/lease/resync-reused-source.model";

async function startSource(t: TestContext, endpoint: Host) {
  const events = new EventEmitter();
  let instance = 0;
  let send: (() => void) | undefined;
  const server = createServer((_request, response) => {
    const image: Image = {
      owner: { endpoint, instance },
      offset: 1,
      contents:
        endpoint === "Backup"
          ? "BackupData"
          : instance === 0
            ? "OldData"
            : "NewData",
    };
    send = () => response.end(JSON.stringify(image));
    events.emit("captured");
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  t.after(async () => {
    server.closeAllConnections();
    await new Promise<void>((resolve) => server.close(() => resolve()));
  });
  const address = server.address();
  assert.ok(address && typeof address !== "string");
  const url = `http://127.0.0.1:${address.port}/snapshot`;
  return {
    rotate() {
      instance += 1;
    },
    async capture() {
      const ready = once(events, "captured");
      const reply = fetch(url).then(async (response) => {
        assert.equal(response.status, 200);
        return (await response.json()) as Image;
      });
      void reply.catch(() => {});
      await Promise.race([ready, reply]);
      return async () => {
        assert.ok(send);
        send();
        send = undefined;
        return await reply;
      };
    },
  };
}

test(
  "responses and reads use the incarnation even when the primary URL is reused",
  { timeout: 30_000 },
  async (t) => {
    const base = await reachableScenario<Command>(
      checker,
      model,
      "OldImageBufferedWhenPrimaryReturns",
    );
    assert.deepEqual(
      base.commands.map(({ kind }) => kind),
      ["capture", "moveToBackup", "returnToPrimary"],
    );
    const commands: Command[] = [
      ...base.commands,
      { kind: "read" },
      { kind: "publish" },
      { kind: "read" },
      { kind: "capture" },
      { kind: "publish" },
      { kind: "read" },
    ];
    const seed: Image = {
      owner: { endpoint: "Primary", instance: 0 },
      offset: 0,
      contents: "Seed",
    };
    const fresh: Image = {
      owner: { endpoint: "Primary", instance: 1 },
      offset: 1,
      contents: "NewData",
    };
    for (const fault of ["none", "publish-by-name", "read-by-name"] as const) {
      await t.test(fault, async (t) => {
        const primary = await startSource(t, "Primary");
        const backup = await startSource(t, "Backup");
        let active = seed.owner;
        let saved = seed;
        let observed: Image | null = null;
        let release: (() => Promise<Image>) | undefined;
        const matches = (image: Image, compareNamesOnly: boolean) =>
          image.owner.endpoint === active.endpoint &&
          (compareNamesOnly || image.owner.instance === active.instance);
        const result = await recordReplay<Command>(
          checker,
          { ...base, commands },
          {
            async apply({ kind }) {
              switch (kind) {
                case "capture":
                  assert.equal(release, undefined);
                  release = await (
                    active.endpoint === "Primary" ? primary : backup
                  ).capture();
                  break;
                case "moveToBackup":
                  active = { endpoint: "Backup", instance: 0 };
                  break;
                case "returnToPrimary":
                  primary.rotate();
                  active = fresh.owner;
                  break;
                case "publish": {
                  assert.ok(release);
                  const image = await release();
                  release = undefined;
                  if (matches(image, fault === "publish-by-name"))
                    saved = image;
                  break;
                }
                case "read":
                  observed = matches(saved, fault === "read-by-name")
                    ? saved
                    : null;
                  break;
              }
            },
            async observe() {
              return { saved, observed };
            },
            project: ({ saved, observed }) => ({ saved, observed }),
          },
        );
        if (fault === "none") {
          assert.equal(result.verdict, "matched");
          assert.deepEqual(result.recording.observations[5], {
            saved: seed,
            observed: null,
          });
          assert.deepEqual(result.recording.observations.at(-1), {
            saved: fresh,
            observed: fresh,
          });
        } else {
          assert.equal(result.verdict, "mismatch");
          assert.deepEqual(
            result.difference,
            fault === "publish-by-name"
              ? {
                  step: 5,
                  command: { kind: "publish" },
                  expected: { saved: seed, observed: null },
                  observed: {
                    saved: { ...seed, offset: 1, contents: "OldData" },
                    observed: null,
                  },
                }
              : {
                  step: 4,
                  command: { kind: "read" },
                  expected: { saved: seed, observed: null },
                  observed: { saved: seed, observed: seed },
                },
          );
        }
      });
    }
  },
);
