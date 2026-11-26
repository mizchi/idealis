const [S, W] = process.argv.slice(2);
const { startNative } = await import(
  `${W}/dist/evals/experiments/usable/native.js`
);
const { splitProbeText, loadCases } = await import(
  `${W}/dist/evals/experiments/call-depth/paths-0120.js`
);
const { bounds } = await import(
  `${W}/dist/evals/experiments/nesting-chains/outcome.js`
);

const files = [
  "nesting-chains/cases.json",
  "nesting-chains/cases-0075.json",
  "call-depth/cases-0119.json",
  "call-depth/cases-0120.json",
].map((f) => `${W}/fixtures/${f}`);
const { cases } = await loadCases(files, `${W}/fixtures/nesting-chains/quint`);
const pick = [
  "hop-invariant-40",
  "hop-invariant-168",
  "violation-120",
  "division-by-zero-120",
  "hop-124",
];
const nat = startNative(`${S}/snapshots/oracle/idealis-core`);
const natB = startNative(`${S}/snapshots/before/idealis-core`);
for (const name of pick) {
  const c = cases.find((x) => x.name === name);
  if (!c) {
    console.log("missing", name);
    continue;
  }
  const comp = JSON.parse(
    await nat.requestText({
      version: 1,
      request: {
        kind: "compile-tla",
        source: c.source,
        config: c.config,
        irVersion: 2,
      },
    }),
  );
  if (comp.kind !== "compiled") {
    console.log(name, "compile reject");
    continue;
  }
  const cmd = {
    version: 1,
    request: { kind: "check-safety", program: comp.program, bounds },
  };
  const t = await nat.requestText(cmd);
  const tb = await natB.requestText(cmd);
  const s = splitProbeText(t);
  const idx = t.indexOf('"probe"');
  console.log(
    "==",
    name,
    "len",
    t.length,
    "probeAt",
    idx,
    "lenBefore",
    tb.length,
  );
  console.log(
    "tail:",
    t.slice(Math.max(0, idx - 60), idx + 260).replace(/\n/g, "\\n"),
  );
  console.log(
    "without===before:",
    s.without === tb,
    "withoutLen",
    s.without.length,
    "published:",
    s.published === null ? null : s.published.slice(0, 160),
  );
  if (s.without !== tb) {
    let i = 0;
    while (i < tb.length && s.without[i] === tb[i]) i++;
    console.log(
      "firstdiff",
      i,
      JSON.stringify(s.without.slice(i - 30, i + 60)),
      JSON.stringify(tb.slice(i - 30, i + 60)),
    );
  }
}
await nat.close();
await natB.close();
