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
const canon = (v) =>
  Array.isArray(v)
    ? `[${v.map(canon).join(",")}]`
    : v && typeof v === "object"
      ? `{${Object.keys(v)
          .sort()
          .map((k) => JSON.stringify(k) + ":" + canon(v[k]))
          .join(",")}}`
      : JSON.stringify(v);
const ors = startNative(`${S}/snapshots/oracle/idealis-core`),
  orsA = startNative(`${S}/snapshots/oracle-after/idealis-core`);
const bef = startNative(`${S}/snapshots/before/idealis-core`),
  aft = startNative(`${S}/snapshots/after/idealis-core`);
const rows = [];
let tot = 0,
  eqExact = 0,
  eqCanon = 0,
  bad = [];
for (const c of cases) {
  const comp = JSON.parse(
    await ors.requestText({
      version: 1,
      request: {
        kind: "compile-tla",
        source: c.source,
        config: c.config,
        irVersion: 2,
      },
    }),
  );
  if (comp.kind !== "compiled") continue;
  const cmd = {
    version: 1,
    request: { kind: "check-safety", program: comp.program, bounds },
  };
  for (const [o, r, tag] of [
    [ors, bef, "oracle"],
    [orsA, aft, "oracle-after"],
  ]) {
    const t = await o.requestText(cmd);
    const s = splitProbeText(t);
    if (s.published === null || s.published === "null") continue;
    const real = await r.requestText(cmd);
    tot++;
    if (s.published === real) eqExact++;
    else if (canon(JSON.parse(s.published)) === canon(JSON.parse(real))) {
      eqCanon++;
      if (rows.length < 3)
        rows.push([c.name, tag, s.published.slice(0, 200), real.slice(0, 200)]);
    } else bad.push(c.name + " " + tag);
  }
}
console.log({
  tot,
  eqExact,
  eqCanon,
  bad: bad.length,
  badNames: bad.slice(0, 20),
});
console.log(rows);
for (const x of [ors, orsA, bef, aft]) await x.close();
