import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test, type TestContext } from "node:test";

const checkerPath = "crates/idealis-check/target/debug/idealis-check";
const example = "examples/model-patterns/worker-microblog";
const manifest = "model.project.json";
const budget = ["--max-states", "200000"];

interface Command {
  readonly kind: string;
  readonly p?: string;
  readonly delivery?: string;
}

async function changeFragment(
  t: TestContext,
  fragment: string,
  before: string,
  after: string,
) {
  const directory = await mkdtemp(join(tmpdir(), "idealis-worker-microblog-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  await cp(example, directory, { recursive: true });
  const file = join(directory, fragment);
  const source = await readFile(file, "utf8");
  assert.equal(source.split(before).length, 2);
  await writeFile(file, source.replace(before, after));
  return directory;
}

function witness(directory: string, question: string) {
  return JSON.parse(
    execFileSync(
      checkerPath,
      ["witness", join(directory, manifest), question, ...budget],
      { encoding: "utf8" },
    ),
  ) as {
    verdict: string;
    complete: boolean;
    trace: { commands: Command[] };
  };
}

test("Workers observation types match the current Idealis model", async () => {
  const generated = execFileSync(
    checkerPath,
    ["generate", join(example, manifest), "typescript-types"],
    { encoding: "utf8" },
  );
  assert.equal(
    await readFile(join(example, "idealis-generated.d.ts"), "utf8"),
    generated,
  );
});

test("Workers feed properties hold with explicit queue and owner boundaries", () => {
  const reports = JSON.parse(
    execFileSync(
      checkerPath,
      [join(example, manifest), "--checker", "reference", ...budget],
      { encoding: "utf8" },
    ),
  ) as {
    question: string;
    answers: { verdict: string; coverage: { kind: string } }[];
  }[];
  for (const { question, answers } of reports) {
    assert.equal(answers[0]?.coverage.kind, "complete", question);
  }
  assert.deepEqual(
    Object.fromEntries(
      reports.map(({ question, answers }) => [question, answers[0]?.verdict]),
    ),
    {
      AckAfterWrite: "holds",
      IndexedPostsAreCommitted: "holds",
      CommittedBeforeEnqueue: "reachable",
      LaterAppliedFirst: "reachable",
      RedeliveredAfterWrite: "reachable",
      AckedAfterRedelivery: "reachable",
      DelayedInitialAckRequest: "reachable",
      StaleIndexRow: "reachable",
      AckMatchesDelivery: "holds",
      ReplayKeepsRows: "holds",
      ReadRespectsFollow: "holds",
      Declarations: "holds",
    },
  );

  const outOfOrder = witness(example, "LaterAppliedFirst");
  assert.equal(outOfOrder.verdict, "reachable");
  assert.deepEqual(
    outOfOrder.trace.commands
      .filter(({ kind }) => kind === "queue_enqueue")
      .map(({ p }) => p),
    ["Earlier", "Later"],
  );
  assert.deepEqual(
    outOfOrder.trace.commands
      .filter(({ kind }) => kind === "queue_deliver")
      .map(({ p }) => p),
    ["Later"],
  );

  const redelivery = witness(example, "RedeliveredAfterWrite");
  assert.equal(redelivery.verdict, "reachable");
  assert.deepEqual(
    redelivery.trace.commands.map(({ kind }) => kind),
    [
      "posts_publish",
      "relay_offer",
      "queue_enqueue",
      "queue_deliver",
      "timeline_apply",
      "queue_retry",
      "queue_deliver",
      "timeline_apply",
    ],
  );

  const retryAck = witness(example, "AckedAfterRedelivery");
  assert.equal(retryAck.verdict, "reachable");
  assert.deepEqual(
    retryAck.trace.commands
      .filter(({ kind }) => kind === "consumer_requestAck")
      .map(({ delivery }) => delivery),
    ["Initial", "Retry"],
  );
  assert.deepEqual(retryAck.trace.commands.slice(-2), [
    { kind: "consumer_requestAck", p: "Earlier", delivery: "Retry" },
    { kind: "queue_ack", p: "Earlier", delivery: "Retry" },
  ]);
});

const faults = [
  {
    file: "consumer.ideal",
    before: "    p in @applied,\n",
    after: "",
    question: "AckAfterWrite",
    last: "queue_ack",
  },
  {
    file: "queue.ideal",
    before: "(p -> delivery) in @ackRequests",
    after: "some p.@ackRequests",
    question: "AckMatchesDelivery",
    last: "queue_ack",
  },
  {
    file: "timeline.ideal",
    before: "@replayed = @replayed + p",
    after:
      "@replayed = @replayed + p\n        @entries = @entries + (@follows.author -> p)",
    question: "ReplayKeepsRows",
    last: "timeline_apply",
  },
  {
    file: "timeline.ideal",
    before: ", author in reader.@follows",
    after: "",
    question: "ReadRespectsFollow",
    last: "timeline_read",
  },
];

for (const fault of faults) {
  test(`Workers feed detects a broken ${fault.question} across models`, async (t) => {
    const directory = await changeFragment(
      t,
      fault.file,
      fault.before,
      fault.after,
    );
    const result = witness(directory, fault.question);
    assert.equal(result.verdict, "violated");
    assert.equal(result.complete, true);
    assert.equal(result.trace.commands.at(-1)?.kind, fault.last);
    if (fault.question === "ReplayKeepsRows") {
      assert.ok(
        result.trace.commands.some(({ kind }) => kind === "queue_retry"),
      );
    }
    if (fault.question === "AckMatchesDelivery") {
      assert.deepEqual(
        result.trace.commands
          .filter(({ kind }) => kind === "consumer_requestAck")
          .map(({ delivery }) => delivery),
        ["Initial"],
      );
      assert.equal(result.trace.commands.at(-1)?.delivery, "Retry");
    }
  });
}

test("the publisher cannot forge a consumer ACK request", async (t) => {
  const directory = await changeFragment(
    t,
    "relay.ideal",
    "update { @offered = @offered + p }",
    "update { @offered = @offered + p\n    ackRequests = ackRequests + (p -> Initial) }",
  );
  const result = spawnSync(checkerPath, [join(directory, manifest)], {
    encoding: "utf8",
  });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /relay_offer cannot write ackRequests/);
});
