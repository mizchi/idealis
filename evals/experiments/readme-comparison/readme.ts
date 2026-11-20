import assert from "node:assert/strict";
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { cases } from "../single-model/cases.js";
import { renderComparison, type Summary } from "./protocol.js";

const directory = process.argv[2];
assert.ok(directory);
const manifest = JSON.parse(
  await readFile(join(directory, "manifest.json"), "utf8"),
);
assert.equal(
  manifest.saved,
  null,
  "Replay timings are not the large-model benchmark",
);
assert.equal(manifest.plan, "0022-readme-comparison");
assert.deepEqual(
  JSON.parse(await readFile(join(directory, "cases.json"), "utf8")),
  cases("large"),
);
const result: {
  readonly status: string;
  readonly summaries: readonly Summary[];
} = JSON.parse(await readFile(join(directory, "result.json"), "utf8"));
const tables = renderComparison(result);
const start = "<!-- checker-comparison:start -->";
const end = "<!-- checker-comparison:end -->";
const section = `${start}

## モデル検査の速度比較

同じTLA+ source/configを各回別processで解析し、完全探索した起動込み時間。単位は秒、各条件1 warmup後3回の中央値。Apple M5 arm64、32 GiB、Node 24.22.2、tla-ts 0.0.10、TLC v1.7.4 / Temurin 21.0.13+9で測った。

${tables.combined}

Wasm列はNode上の実行で、ブラウザの速度ではない。nativeの幅1モデルはpoolを起動しない。tla-tsは今回1 workerのみを測っており、その並列性能との比較はしていない。TLCとNodeのheap設定は8 GiBで、共通の厳密なRSS上限ではない。idealisは対応する有限fragmentを検査し、TLA+全体の互換性やモデル全般の速度を保証する結果ではない。

[全sample・最小/最大・条件と再実行](evals/iterations/0022-readme-comparison.md)を保存した。表は同じrunから生成し、過去の値を混在させていない。常駐APIの比較は[別の0016測定](evals/iterations/0016-checker-benchmark.md#apiを再利用した全探索)を参照。上の起動込み時間と同じ指標として扱わない。

${end}

`;
const readme = await readFile("README.md", "utf8");
const begin = readme.indexOf(start);
const finish = readme.indexOf(end);
assert.equal(begin >= 0, finish >= 0);
const updated =
  begin >= 0
    ? readme.slice(0, begin) +
      section +
      readme.slice(finish + end.length).trimStart()
    : readme.replace(
        "## モデルと実装の比較",
        `${section}## モデルと実装の比較`,
      );
assert.ok(updated.includes(start) && updated.includes(end));
await writeFile("README.md", updated);
