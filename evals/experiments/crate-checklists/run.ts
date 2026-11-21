import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  appendFile,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import {
  type DocumentInput,
  documentExitCode,
  parseDocumentReport,
  prepareDocuments,
  scanDocuments,
} from "jev-lint/documents";
import { manifestFor, sourceView } from "../../../scripts/check-support.js";
import { digest } from "../usable/provenance.js";

const cases = [
  {
    id: "smt-clean",
    crate: "rusmt",
    expected: "supports",
    remove: null,
    addition: null,
  },
  {
    id: "smt-real",
    crate: "rusmt",
    expected: "contradicts",
    remove: "- [ ] Real/LRA、Real/Int混合。",
    addition: "- [x] Real/LRAの一般的な式とReal/Int混合を求解できる。",
  },
  {
    id: "testing-clean",
    crate: "idealis-connect",
    expected: "supports",
    remove: null,
    addition: null,
  },
  {
    id: "testing-scalar",
    crate: "idealis-connect",
    expected: "contradicts",
    remove:
      "- [ ] scalar値の縮小、任意の高階Strategy、Proptest/fast-checkのStrategyやseedとの互換性。",
    addition: "- [x] 整数・文字列などのscalar値も縮小する。",
  },
];
const directory = process.argv[2];
assert.ok(directory);
if (process.argv[3] === "--replay") {
  const result: unknown = JSON.parse(
    await readFile(join(directory, "result.json"), "utf8"),
  );
  assert.ok(
    result &&
      typeof result === "object" &&
      "status" in result &&
      result.status === "complete" &&
      "files" in result &&
      Array.isArray(result.files),
  );
  assert.deepEqual(
    result.files.map((file) => file.path).sort(),
    [
      "manifest.json",
      ...cases.flatMap((item) => [
        `${item.id}.input.json`,
        `${item.id}.record.json`,
      ]),
    ].sort(),
  );
  for (const file of result.files) {
    assert.ok(
      file &&
        typeof file.path === "string" &&
        typeof file.sha256 === "string" &&
        !file.path.includes("/"),
    );
    assert.equal(
      digest(await readFile(join(directory, file.path))),
      file.sha256,
    );
  }
  for (const item of cases) {
    const input = JSON.parse(
      await readFile(join(directory, `${item.id}.input.json`), "utf8"),
    ) as DocumentInput;
    const report = parseDocumentReport(
      JSON.parse(
        await readFile(join(directory, `${item.id}.record.json`), "utf8"),
      ),
      input,
    );
    assert.equal(report.candidates.length, 1);
    assert.equal(report.candidates[0]?.judgment?.choice, item.expected);
    assert.equal(
      documentExitCode(report),
      item.expected === "supports" ? 0 : 1,
    );
  }
  console.log("4 labelled Jev comparisons replayed without model requests");
} else {
  await mkdir(dirname(directory), { recursive: true });
  await mkdir(directory);
  const files: { path: string; sha256: string }[] = [];
  const results = [];
  const save = async (path: string, value: unknown) => {
    const content = JSON.stringify(value, null, 2);
    await writeFile(join(directory, path), content, { flag: "wx" });
    files.push({ path, sha256: digest(content) });
  };
  await save("manifest.json", {
    plan: "0045-crate-checklists",
    cases,
    commit: execFileSync("git", ["rev-parse", "HEAD"]).toString().trim(),
    dirty: execFileSync("git", ["status", "--porcelain"]).toString(),
    model: "jev-1.14.0",
  });
  for (const item of cases) {
    const temporary = await mkdtemp(join(tmpdir(), "idealis-support-control-"));
    try {
      const full = await manifestFor(
        sourceView(process.cwd(), false),
        item.crate,
      );
      const manifest = {
        ...full,
        comparisons: full.comparisons.filter(
          (comparison) => comparison.id === `${item.crate}-readme`,
        ),
      };
      assert.equal(manifest.comparisons.length, 1);
      for (const source of Object.values(manifest.files)) {
        let content = await readFile(source.path, "utf8");
        if (
          source.path === `crates/${item.crate}/SUPPORT.md` &&
          item.addition
        ) {
          assert.ok(item.remove && content.includes(item.remove));
          content = content.replace(item.remove, "");
          const heading =
            item.crate === "rusmt" ? "## 対応済みの基礎" : "## 対応済み";
          assert.ok(content.includes(heading));
          content = content.replace(heading, `${heading}\n\n${item.addition}`);
        }
        await mkdir(dirname(join(temporary, source.path)), { recursive: true });
        await writeFile(join(temporary, source.path), content, { flag: "wx" });
      }
      const input = prepareDocuments(manifest, { cwd: temporary });
      assert.deepEqual(input.issues, []);
      const report = await scanDocuments(input, {
        clientOptions: { model: "jev-1.14.0" },
        snapshotCheck: () =>
          prepareDocuments(manifest, { cwd: temporary }).snapshotDigest ===
          input.snapshotDigest,
      });
      await save(`${item.id}.input.json`, input);
      await save(`${item.id}.record.json`, report);
      results.push({
        id: item.id,
        expected: item.expected,
        actual: report.candidates[0]?.judgment?.choice ?? null,
        exit: documentExitCode(report),
        usage: report.usage,
      });
      console.log(JSON.stringify(results.at(-1)));
    } finally {
      await rm(temporary, { recursive: true, force: true });
    }
  }
  const matched = results.every(
    (result) =>
      result.actual === result.expected &&
      result.exit === (result.expected === "supports" ? 0 : 1),
  );
  await writeFile(
    join(directory, "result.json"),
    JSON.stringify(
      { status: matched ? "complete" : "controls-failed", results, files },
      null,
      2,
    ),
    { flag: "wx" },
  );
  await appendFile(
    "evals/runs/ledger.jsonl",
    `${JSON.stringify({ runId: basename(directory), kind: "crate-checklists", status: matched ? "complete" : "controls-failed" })}\n`,
  );
  assert.ok(
    matched,
    "Jev checklist controls did not match the predeclared labels",
  );
}
