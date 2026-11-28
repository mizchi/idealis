import assert from "node:assert/strict";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { createWorkspace } from "../../tools/idealis-studio/workspace.mjs";

const example = "examples/model-patterns/worker-microblog";
const model = join(example, "model.project.json");
const spec = join(example, "spec.json");
const checker = resolve("crates/idealis-check/target/debug/idealis-check");

test("Studio links rules to checks and previews saved edits against the startup snapshot", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "idealis-studio-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await cp(example, join(root, example), { recursive: true });
  const workspace = await createWorkspace({ root, model, spec, checker });
  const start = await workspace.refresh();
  assert.deepEqual(start.preview.files, []);
  assert.equal(start.current.name, "WorkerMicroblog");
  const ack = start.current.rules.find(
    (rule) => rule.id === "MICROBLOG-ACK-AFTER-WRITE",
  );
  assert.deepEqual(ack.claims, [
    { model, check: "AckAfterWrite", expect: "holds", resolved: true },
  ]);
  assert.ok(
    start.current.components
      .find(({ name }) => name === "queue")
      .ports.some(({ state }) => state === "ackRequests"),
  );

  const docs = join(root, example, "timeline.ideal");
  const documented = await readFile(docs, "utf8");
  await writeFile(
    docs,
    documented.replace(
      "Checks current follows on read.",
      "Rereads the follow relation.",
    ),
  );
  const descriptionEdit = await workspace.refresh();
  const timeline = descriptionEdit.current.components.find(
    ({ name }) => name === "timeline",
  );
  assert.equal(
    timeline.description.en,
    "Creates timeline rows on first delivery.\nRereads the follow relation.",
  );
  assert.deepEqual(descriptionEdit.preview.symbols, [
    { kind: "component", name: "timeline", status: "modified" },
  ]);
  assert.deepEqual(descriptionEdit.current.checks, start.current.checks);
  await writeFile(docs, documented);

  const source = await readFile(join(root, example, "consumer.ideal"), "utf8");
  await writeFile(
    join(root, example, "consumer.ideal"),
    source.replace("    p in @applied,\n", ""),
  );
  const changed = await workspace.refresh();
  assert.ok(
    changed.preview.symbols.some(
      ({ kind, name }) => kind === "action" && name === "consumer_requestAck",
    ),
  );
  assert.match(changed.preview.files[0].patch, /-    p in @applied,/);
  const evidence = await workspace.witness("AckAfterWrite", changed.revision);
  assert.equal(evidence.verdict, "violated");
  assert.equal(evidence.trace.commands.at(-1).kind, "queue_ack");

  await writeFile(join(root, example, "consumer.ideal"), "broken syntax");
  const invalid = await workspace.refresh();
  assert.ok(invalid.current.diagnostics.some(({ area }) => area === "model"));
  assert.equal(invalid.preview.symbols, null);
  assert.match(invalid.preview.files[0].patch, /\+broken syntax/);
  await assert.rejects(
    workspace.witness("AckAfterWrite", changed.revision),
    /変更/,
  );

  await writeFile(join(root, example, "consumer.ideal"), source);
  const specSource = await readFile(join(root, spec), "utf8");
  await writeFile(
    join(root, spec),
    specSource.replace('"check": "AckAfterWrite"', '"check": "MissingCheck"'),
  );
  const unlinked = await workspace.refresh();
  assert.ok(
    unlinked.preview.symbols.some(
      ({ kind, name }) => kind === "rule" && name === ack.id,
    ),
  );
  assert.equal(
    unlinked.current.rules.find(({ id }) => id === ack.id).claims[0].resolved,
    false,
  );
  assert.equal(
    unlinked.current.checks.find(({ name }) => name === "AckAfterWrite").rules
      .length,
    0,
  );
});

test("Studio resolves descriptor ports independently for instances of the same component", async () => {
  const project =
    "examples/model-patterns/offline-merge/two-stores/model.project.json";
  const workspace = await createWorkspace({
    root: process.cwd(),
    model: project,
    checker,
  });
  const { current } = await workspace.refresh();
  assert.deepEqual(current.diagnostics, []);
  const { connections } =
    await import("../../tools/idealis-studio/topology.js");
  assert.deepEqual(connections(current.components), [
    { from: "east", to: "eastDelivery", states: ["eastReceipt"] },
    { from: "west", to: "westDelivery", states: ["westReceipt"] },
  ]);
});

test("Studio previews domain membership changes and reports invalid assignments", async (t) => {
  const root = await mkdtemp(join(tmpdir(), "idealis-domain-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  await cp(example, join(root, example), { recursive: true });
  const workspace = await createWorkspace({ root, model, spec, checker });
  const start = await workspace.refresh();
  assert.equal(start.current.graph.domains.length, 4);
  assert.ok(
    start.current.graph.width > start.current.graph.height,
    "grouped models flow horizontally",
  );
  const states = new Map(
    start.current.stateCatalog.map((state) => [state.name, state]),
  );
  assert.equal(states.get("timeline_appliedOnInitial").owner, "timeline");
  assert.equal(
    states.get("timeline_appliedOnInitial").localName,
    "appliedOnInitial",
  );
  assert.equal(states.get("timeline_appliedOnInitial").kind, "internal");
  assert.equal(states.get("inFlight").kind, "shared");
  assert.deepEqual(
    states.get("inFlight").ports.map(({ model, access }) => [model, access]),
    [
      ["queue", "write"],
      ["timeline", "read"],
      ["consumer", "read"],
    ],
  );
  assert.equal(states.get("pending").kind, "project");

  const graph = start.current.graph;
  for (const member of graph.nodes) {
    const boundary = graph.domains.find(({ id }) => id === member.domain);
    assert.ok(boundary);
    assert.ok(member.x >= boundary.x && member.y >= boundary.y);
    assert.ok(member.x + member.width <= boundary.x + boundary.width);
    assert.ok(member.y + member.height <= boundary.y + boundary.height);
  }

  const path = join(root, model);
  const manifest = JSON.parse(await readFile(path, "utf8"));
  manifest.domains.find(({ id }) => id === "delivery").models = [
    "queue",
    "consumer",
  ];
  manifest.domains.find(({ id }) => id === "publishing").models.push("relay");
  await writeFile(path, JSON.stringify(manifest));
  const preview = await workspace.refresh();
  assert.ok(
    preview.preview.symbols.some(
      ({ kind, name }) => kind === "domain" && name === "publishing",
    ),
  );
  assert.equal(
    preview.current.graph.edges.find(
      ({ from, to }) => from === "posts" && to === "relay",
    ).crossDomain,
    false,
  );
  manifest.domains[0].models.push("relay");
  await writeFile(path, JSON.stringify(manifest));
  const invalid = await workspace.refresh();
  assert.ok(
    invalid.current.diagnostics.some(({ message }) =>
      /relay.*more than one domain/.test(message),
    ),
  );
  assert.equal(invalid.preview.symbols, null);
});
