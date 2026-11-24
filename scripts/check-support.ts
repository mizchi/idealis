import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import {
  chmod,
  lstat,
  mkdir,
  mkdtemp,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, posix, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import type { AskClient } from "jev-lint/client";
import {
  type DocumentManifest,
  type DocumentReport,
  documentExitCode,
  documentJudgmentKey,
  loadDocumentLedger,
  parseDocumentManifest,
  prepareDocuments,
  replayDocuments,
  scanDocuments,
} from "jev-lint/documents";

interface Evidence {
  readonly path: string;
  readonly topic: string;
  readonly contains?: string;
  readonly targetContains?: string;
  readonly context?: readonly string[];
  readonly contextContains?: string;
}
interface CrateScope {
  readonly name: string;
  readonly evidence: readonly Evidence[];
}
export interface SourceView {
  readonly paths: readonly string[];
  readonly read: (path: string) => Promise<Buffer>;
}

const scopes: readonly CrateScope[] = [
  {
    name: "rtla",
    evidence: [
      {
        path: "src/lib.rs",
        topic: "公開parser API、構文と評価の分離、整数と深さの上限",
      },
      {
        path: "src/parser.rs",
        contains: "fn expression(",
        topic: "infixとprefixの結合、CASEとselectorの構文、資源上限の分類",
      },
      {
        path: "src/parser.rs",
        contains: "fn module_inner(",
        topic: "nested module、RECURSIVE、宣言とproofの構造保持",
      },
      {
        path: "src/parser.rs",
        contains: "fn instance(",
        topic: "引数付きINSTANCE、記号WITHとLET内INSTANCEの構文",
        context: ["tests/modules.rs", "tests/instances.rs"],
      },
      {
        path: "src/parser/proof.rs",
        topic: "ASSUME/PROVE、proof step階層と参照の保持、証明検証との区別",
      },
      {
        path: "src/lexer.rs",
        topic: "ASCIIとUnicodeのoperator・構造alias、字句処理と位置",
      },
      {
        path: "tests/versions.rs",
        topic:
          "1.7.4/1.8.0の優先順位選択、既定と共通文法、廃止文法や旧版の不具合との区別",
      },
    ],
  },
  {
    name: "rtla-compiler",
    evidence: [
      {
        path: "src/symmetry.rs",
        topic:
          "限定したSYMMETRY設定のlowering、untyped model valueとflat moduleの条件、対称性証明との区別",
      },
      {
        path: "benches/quint_generated.rs",
        contains: "fn measurements(",
        targetContains: "同じ6生成物の解析",
        topic:
          "固定した6つのQuint生成物で解析・v1/v2のcompile・prepared check・全体の42測定を分け、TLCの正常判定・状態数・生成数の検査後に測る",
      },
      {
        path: "src/link.rs",
        topic:
          "明示したmodule集合のEXTENDS/INSTANCE、LOCALとscope、設定置換の対応範囲・拒否と展開上限",
        contains: "fn instantiate(",
      },
      {
        path: "src/rewrite.rs",
        topic:
          "字句名とoperator引数のhygienicな置換、selectorとmodule引数の解決",
      },
      {
        path: "src/link.rs",
        contains: "pub(crate) fn resolve(",
        topic:
          "moduleと設定の上書き、値とoperatorのarity、欠落・衝突・上限の検証",
      },
      {
        path: "src/lib.rs",
        contains: "pub fn compile(",
        topic: "受理するmodule/configと、実行しない・拒否する構文の境界",
      },
      {
        path: "src/lib.rs",
        contains: "const TARGET_EXPANSION_LIMIT:",
        targetContains: "524,288ノード",
        topic:
          "tuple targetのcompile時node上限の実際の定数値とチェックリストの数値が一致する",
      },
      {
        path: "src/lib.rs",
        contains: "fn resolve_target_within(",
        targetContains: "v2のtuple target解決と共有IR",
        context: ["src/lib.rs"],
        contextContains: "const TARGET_EXPANSION_LIMIT:",
        topic:
          "v2のtuple target解決で共通node上限と既存のdepth/leaf制限を維持し、空tupleでも展開を有界にする",
      },
      {
        path: "src/typed_expr.rs",
        contains: "pub(super) fn variable_reads_ir(",
        targetContains: "v2のtuple target解決と共有IR",
        context: ["src/lib.rs"],
        contextContains: "const TARGET_EXPANSION_LIMIT:",
        topic:
          "共有IRの変数読取り展開で共通node上限を適用し、caller frame/actualと既存depthの意味を維持する",
      },
      {
        path: "src/typed_lower.rs",
        contains: "fn frame_actuals(",
        targetContains: "frame_actualsは",
        context: ["src/typed.rs"],
        contextContains: "pub(crate) fn check_depth_from(",
        topic:
          "frame_actualsはargumentsとcapturesの各Expressionを初期depth 1でcheck_depth_fromに渡し、全件の検査後にVecをcloneする",
      },
      {
        path: "src/typed.rs",
        contains: "pub(crate) fn check_depth_from(",
        targetContains: "時間論理IRの構造的な深さ",
        context: ["src/typed_lower.rs"],
        contextContains: "fn frame_formula(",
        topic:
          "v2は式深さと別に時間論理IRの全Temporal nodeを128段まで数え、frame compaction前にprotocol拒否する",
      },
      {
        path: "src/lib.rs",
        contains: "fn apply(",
        targetContains: "signed 32-bit整数",
        topic:
          "有限式、action、CASE、Apalache folds、限定したVariants、SelectSeq/SortSeq/Assertのlowering",
      },
      {
        path: "src/lib.rs",
        contains: "fn parts(",
        topic:
          "SPECIFICATIONの分解、定数有限domain上の量化fairness、wrapperと対象外の区別",
      },
      {
        path: "src/lib.rs",
        contains: "fn shared_body_ir(",
        topic:
          "共有expression/action/temporal定義の登録、phase・用途・target signature・body kindの区別と遅延引数",
      },
      {
        path: "src/typed_expr.rs",
        topic:
          "明示v2の型付きexpression lowering、共有callとframe-localなbinder、SelectSeq/SortSeq/Assertと再帰callback",
      },
      {
        path: "src/typed_lower.rs",
        topic:
          "明示v2のprogramとaction/temporal frame、caller環境とtarget解決、module連結、frameが必要な非再帰action/temporalと再帰actionの共有call、零引数inlineと展開上限",
      },
      {
        path: "src/elaborate.rs",
        topic:
          "高階operator・LAMBDAの静的特殊化、上限と公開action名、stateへ格納するoperator値との区別",
      },
      {
        path: "src/select.rs",
        topic: "label・binder引数と一般subexpression selectorの解決と対象外",
      },
      {
        path: "src/levels.rs",
        topic:
          "未使用定義を含むsource level制約、formal/capture/LET/再帰への固定点伝播、引数の上限と結果の依存の区別。全SANY名前・型・proof検査は対象外",
      },
      {
        path: "src/functions.rs",
        topic:
          "有限domainの暗黙再帰関数、直接keyとDOMAINのlowering、eagerな関数値との区別",
      },
      {
        path: "src/old.rs",
        topic:
          "引数なしLETの自由なEXCEPT @を宣言側へ保持し、引数あり定義の適用側oldと区別する",
      },
      {
        path: "src/labels.rs",
        topic:
          "labelのbound parameter・重複・EXCEPT境界の検査と、完全なSANY意味解析との区別",
      },
      {
        path: "src/link/local.rs",
        topic: "LET内の引数付きINSTANCEと字句scopeの保持",
      },
      {
        path: "src/link/nested.rs",
        topic: "nested moduleの宣言位置のscope、外側の宣言と内側WITH対象の区別",
      },
    ],
  },
  {
    name: "rtlc",
    evidence: [
      {
        path: "src/safety_values.rs",
        contains: "pub fn portable_bounded(",
        targetContains: "状態keyとsuccessor列のportable変換は1展開64 KiB",
        context: ["src/safety.rs"],
        contextContains: "fn expand_cached(",
        topic:
          "cache保存時の1展開64 KiBの保守的な変換予算、共有値の複製前のサイズ計算、超過時と保存容量不足時のstreaming fallbackと不完全な展開の保存拒否",
      },
      {
        path: "src/expansions.rs",
        contains: "pub struct ExpansionOptions",
        targetContains: "ExpansionOptions.enabledは既定false",
        context: ["src/safety.rs"],
        topic:
          "enabledの既定falseと、無効時は通常checkへ委譲してcacheを参照・更新せず計数を0とする契約",
      },
      {
        path: "src/expansions.rs",
        contains: "impl SafetyExpansionCache",
        targetContains: "任意のcheck_incremental",
        context: ["src/safety.rs"],
        topic:
          "同一build・全Next/definition・portable状態の完全展開だけを保持し、SHA-256、保持状態数上限とmerge競合拒否を扱う。property/constraintsの再検査と逐次探索の範囲を維持する",
      },
      {
        path: "src/ir.rs",
        topic:
          "IRの値と式・action定義・Step call・予算・結果のwire契約。準備済み所有型の実装は別比較",
        context: ["src/lib.rs"],
        targetContains: "IRのwire構造は",
      },
      {
        path: "src/safety.rs",
        contains: "pub fn prepare(self)",
        topic:
          "所有するPreparedSafetyProgramの静的準備、値やASSUMEを先に評価しない契約、Data索引の再利用",
        context: ["src/safety_prepared.rs"],
      },
      {
        path: "src/safety.rs",
        topic:
          "有限BFS、invariantとdeadlock、結果と探索予算、opt-inの逐次action coverageとworkの内訳",
        context: ["src/safety_coverage.rs"],
      },
      {
        path: "src/temporal.rs",
        topic: "temporal/fairnessの検査とlasso、保証範囲の区別",
      },
      {
        path: "src/safety_symmetry.rs",
        topic:
          "単一のuntypedモデル値群、scalar/flat recordのnode関数とnode部分集合、同じnode行のopt-in代表展開、equivarianceの利用者前提、具体traceと計数、対象外",
        context: ["src/safety_parallel.rs"],
      },
      {
        path: "src/safety_values.rs",
        contains: "pub fn scalar_record_shape(",
        topic:
          "symmetryのflat record payloadのfield名・Int/Bool型の検証とnested/node参照の拒否",
      },
      {
        path: "src/safety_partial_order.rs",
        topic:
          "opt-inの限定POR、不可視で独立な単調Bool flagの条件、他branchによるresetの禁止、fallback、一般PORとの区別",
        context: ["src/safety.rs"],
      },
      {
        path: "src/safety_prepared.rs",
        topic:
          "immutableなData式のslot登録、未評価分岐や失敗を先に評価しない定数の準備。状態依存式と型別命令VMは対象外",
      },
      {
        path: "src/safety_parallel.rs",
        topic: "native内部並列化の対象、逐次fallback、workerとworkの契約",
      },
      {
        path: "src/safety_values.rs",
        contains: "impl<'scope> Arena<'scope>",
        topic:
          "immutable domain共有、Boolと整数の別bit表現、membershipの論理work、worker参照とremap、hash衝突の完全比較",
        context: ["src/safety_functions.rs"],
      },
      {
        path: "src/safety_functions.rs",
        contains: "pub fn position(",
        targetContains: "Int/Ref domainのlookupは",
        topic:
          "Int/Refの添字候補を完全key一致のときだけ使用し、疎なdomain・Ref順が異なる場合・欠落keyを線形検索へ戻す",
      },
      {
        path: "src/safety_lazy.rs",
        topic:
          "v2の選択とDOMAINでalias/actual/record/tupleを通る有限関数の遅延観測、所有snapshotと要求key、完全観測の実体化。全観測位置のTLC lazy互換とmemoizationは対象外",
      },
      {
        path: "src/safety_definitions.rs",
        topic:
          "v2の定義graph、expression/action/temporal kind・ID/arity/capture/phaseとframe-local slot、明示recursiveの強連結成分と固定点によるmetadata検証",
      },
      {
        path: "src/safety_expressions.rs",
        contains: "fn eval_reference<T>(",
        topic:
          "call-by-nameの要求時評価、callerのlexical環境とphase、callと引数のwork課金。値のmemoizationとの区別",
      },
      {
        path: "src/safety_expressions.rs",
        contains: "pub(crate) fn visit_branch(",
        topic:
          "共有action呼出しの代入生成、分岐の巻き戻しとcaller継続、再帰フラグとwork境界",
      },
      {
        path: "src/safety_expressions.rs",
        contains: "impl CallFrame {",
        topic:
          "actual/capture間でのimmutableな親frameの共有、caller locals/oldと要求時評価、値cacheを持たないこと",
      },
      {
        path: "src/bounded_drop.rs",
        topic:
          "深いIR/Valueの破棄を32段の再帰とworklistで制限する。評価・compileの既存深さ上限とは区別する",
      },
    ],
  },
  {
    name: "idealis-core",
    evidence: [
      {
        path: "src/lib.rs",
        contains: "enabled: bool",
        targetContains: "enabledは省略時false",
        topic:
          "check-safety-incrementalのenabledはserde defaultでfalseとなり、明示したflagをrtlcのExpansionOptionsへ渡す",
      },
      {
        path: "src/lib.rs",
        contains: "Request::CheckSafetyIncremental",
        targetContains: "check-safety-incremental / incremental-safety",
        topic:
          "rtlcの状態別Next cacheをnative JSONLとWasmで入出力し、同一namespaceのsnapshot mergeへ委譲する。分散BFSやqueue再開とは区別する",
      },
      {
        path: "src/lib.rs",
        topic: "Model/Driver effect、照合protocol、TLA検査への委譲",
      },
      {
        path: "src/history.rs",
        topic: "並行履歴の確定・未確定イベントと有限探索予算",
      },
      {
        path: "src/host.rs",
        topic: "明示した時計・通信・永続化とHostTapeでありOS障害ではないこと",
      },
    ],
  },
  {
    name: "rusmt",
    evidence: [
      {
        path: "src/parser.rs",
        topic: "受理するSMT-LIB式・型・制約と拒否する範囲",
      },
      {
        path: "src/solver.rs",
        topic: "assertion scope、一時assumptions、求解結果の契約",
      },
      {
        path: "src/script.rs",
        topic: "対応scriptコマンド、checkの順序、共有workと空の結果列",
      },
    ],
  },
  {
    name: "ralloy",
    evidence: [
      {
        path: "src/parser.rs",
        topic: "静的なAlloyの受理する構文と有限scope、未対応構文",
        // The parser and evaluator outgrew the 20-excerpt context budget once
        // ralloy covered more of Alloy (plans 0079-0082); the syntax tree
        // alone states what is accepted.
        context: ["src/syntax.rs"],
      },
      {
        path: "src/lib.rs",
        topic: "公開APIと結果status、解析・固定instance検査の区別",
      },
      {
        path: "src/check.rs",
        topic: "固定した実装instanceを検査し、invalidやunknownを区別する契約",
        context: ["src/instance.rs", "src/syntax.rs"],
      },
    ],
  },
  {
    name: "rspec",
    evidence: [
      {
        path: "src/lib.rs",
        topic:
          "Pklの仕様（用語・rule・claim）の静的な結びつきの検査、生成するMarkdownとjev-lintのmanifest",
      },
    ],
  },
  {
    name: "rcheck",
    evidence: [
      {
        path: "src/lib.rs",
        topic:
          "検査器の宣言（complete・bounded・対応しない）、答えの語、autoとallの選び方と食い違い",
        context: ["src/checkers.rs"],
      },
    ],
  },
  {
    name: "rmodel",
    evidence: [
      {
        path: "src/syntax.rs",
        topic: "モデル言語の構文・型・actionの形の検査と拒否する入力",
      },
      {
        path: "src/emit.rs",
        topic: "AlloyとTLA+への2つの写し方、暗黙のframeと多重度の意味",
      },
    ],
  },
  {
    name: "rquint",
    evidence: [
      {
        path: "src/lib.rs",
        topic: "受理するQuint IRのopcode・宣言、TLA+への写し方、拒否する構文",
      },
    ],
  },
  {
    name: "idealis-cli",
    evidence: [
      {
        path: "src/bin/idealis/tlc.rs",
        topic: "有限TLCの引数、終了値、予算切れと検査完了の区別",
        targetContains: "`idealis tlc` は",
        context: ["src/bin/idealis/modules.rs", "tests/tla_cli.rs"],
      },
      {
        path: "src/bin/idealis/tla.rs",
        topic: "SANY互換の限定した解析オプションと診断",
        targetContains: "`idealis tla` は",
        context: ["src/bin/idealis/modules.rs", "tests/tla_cli.rs"],
      },
      {
        path: "src/main.rs",
        topic:
          "JSONLの各行を同じEngineへ渡し、応答を行単位でflushするnative実行口",
      },
    ],
  },
  {
    name: "idealis-connect",
    evidence: [
      {
        path: "src/tla.rs",
        topic:
          "任意のtla featureでの制約適用後の有限モデル射影と実装照合、invalidとmismatchの区別",
        context: ["tests/tla.rs", "examples/tla_store.rs"],
      },
      {
        path: "src/session.rs",
        topic: "PBT effect、失敗signature、縮小と再生、未確定と成功の区別",
      },
      {
        path: "src/shrink.rs",
        topic: "操作列の削除による縮小でありscalarの縮小ではないこと",
      },
      {
        path: "src/schedule.rs",
        topic: "明示したactor操作順と構造上限、自動thread計装ではないこと",
      },
      {
        path: "src/compare.rs",
        topic: "許容観測集合との比較とerror/unknown/invalidの保持",
      },
    ],
  },
  {
    name: "roracle",
    evidence: [
      {
        path: "src/wire.rs",
        topic:
          "version付き観測batchをOracleGroupへ通すJSON境界、件数と入力size、CLIとWasm ABI",
        context: [
          "src/wasm.rs",
          "src/main.rs",
          "src/wire_observation.rs",
          "tests/json.rs",
          "tests/bytes.rs",
          "tests/value.rs",
          "benches/wire.rs",
        ],
      },
      {
        path: "src/typescript.rs",
        topic:
          "開発用typescript featureのRust契約からのTS型生成、既定依存と実行時serde制約との分離",
        context: [
          "Cargo.toml",
          "examples/typescript.rs",
          "src/report.rs",
          "src/wire.rs",
        ],
      },
      {
        path: "src/report.rs",
        topic:
          "typed observation比較とmatched/mismatch/invalid/unknown/error、集約の優先順位",
      },
      {
        path: "src/group.rs",
        topic:
          "groupとcase登録、入力付きreport、保存oracleを呼ばないreplay、構造検査",
      },
      {
        path: "src/bench.rs",
        topic:
          "preflightと全呼出照合、予算、clock、順序、raw samplesと速度比、部分失敗",
        context: ["tests/oracle.rs"],
      },
    ],
  },
];
const shared = [
  "scripts/check-support.ts",
  "justfile",
  "package.json",
  "packages/idealis/package.json",
  "pnpm-workspace.yaml",
  "pnpm-lock.yaml",
];
/** The model every checklist judgment is asked of and keyed on. */
const MODEL = "jev-1.14.0";
/**
 * One judgment ledger per crate (jev-lint 0.10): append-only files keyed on
 * the compared excerpts, committed and merged as a union. The manifest is
 * rebuilt from the catalog on every run and is not stored.
 */
export const supportLedger = (root: string, name: string) =>
  join(root, ".jev-lint/checklists", name);
const git = (root: string, args: readonly string[]) =>
  execFileSync("git", [...args], { cwd: root, maxBuffer: 16 * 1024 * 1024 });

export const sourceView = (root: string, staged: boolean): SourceView => {
  if (staged) {
    const entries = new Map(
      git(root, ["ls-files", "--stage", "-z"])
        .toString()
        .split("\0")
        .filter(Boolean)
        .map((entry) => {
          const match = /^(\d+) ([a-f0-9]+) (\d)\t([\s\S]+)$/.exec(entry);
          assert.ok(
            match && match[3] === "0",
            "Unmerged index entries cannot be reviewed",
          );
          return [
            match[4] ?? "",
            { mode: match[1], oid: match[2] ?? "" },
          ] as const;
        }),
    );
    return {
      paths: [...entries.keys()].sort(),
      read: async (path) => {
        const entry = entries.get(path);
        assert.ok(
          entry && ["100644", "100755"].includes(entry.mode ?? ""),
          `Missing or non-regular staged source: ${path}`,
        );
        return git(root, ["cat-file", "blob", entry.oid]);
      },
    };
  }
  return {
    paths: git(root, [
      "ls-files",
      "-z",
      "--cached",
      "--others",
      "--exclude-standard",
    ])
      .toString()
      .split("\0")
      .filter(Boolean)
      .sort(),
    read: async (path) => {
      assert.ok(!posix.isAbsolute(path) && !path.split("/").includes(".."));
      for (const part of path
        .split("/")
        .map((_, index, parts) => parts.slice(0, index + 1).join("/")))
        assert.ok(
          !(await lstat(join(root, part))).isSymbolicLink(),
          `Symlink source: ${path}`,
        );
      assert.ok(
        (await lstat(join(root, path))).isFile(),
        `Not a regular file: ${path}`,
      );
      return readFile(join(root, path));
    },
  };
};

export const checkSupport = async (
  view: SourceView,
  crateNames: readonly string[],
): Promise<void> => {
  const actual = view.paths
    .flatMap(
      (path) => /^crates\/([^/]+)\/Cargo.toml$/.exec(path)?.slice(1) ?? [],
    )
    .sort();
  assert.deepEqual(
    actual,
    [...crateNames].sort(),
    "Crate inventory changed; update the checklist catalog",
  );
  for (const name of actual) {
    const path = `crates/${name}/SUPPORT.md`;
    const content = (await view.read(path)).toString();
    const visible: string[] = [];
    let fence: { marker: string; length: number } | undefined;
    for (const line of content.split("\n")) {
      const marker = /^ {0,3}(`{3,}|~{3,})/.exec(line)?.[1];
      if (marker) {
        if (!fence) fence = { marker: marker[0] ?? "", length: marker.length };
        else if (marker[0] === fence.marker && marker.length >= fence.length)
          fence = undefined;
        continue;
      }
      if (!fence) visible.push(line);
    }
    assert.match(
      visible.join("\n"),
      /^\s*[-*] \[[xX]\] .+/m,
      `${path}: supported checklist is missing`,
    );
    assert.match(
      visible.join("\n"),
      /^\s*[-*] \[ \] .+/m,
      `${path}: unsupported checklist is missing`,
    );
    for (const link of content.matchAll(/\]\(([^)]+)\)/g)) {
      const target = link[1] ?? "";
      if (/^(?:https?:|#)/.test(target)) continue;
      const destination = posix.normalize(
        posix.join(posix.dirname(path), target.split("#")[0] ?? ""),
      );
      assert.ok(
        view.paths.includes(destination),
        `${path}: link absent from selected tree: ${target}`,
      );
      await view.read(destination);
    }
  }
};

export const manifestFor = async (
  view: SourceView,
  name: string,
): Promise<DocumentManifest> => {
  const scope = scopes.find((candidate) => candidate.name === name);
  assert.ok(scope, `Unknown support scope: ${name}`);
  const prefix = `crates/${scope.name}/`;
  const support = (await view.read(`${prefix}SUPPORT.md`)).toString();
  const linked = [...support.matchAll(/\]\(([^)]+)\)/g)].flatMap((match) => {
    const target = match[1] ?? "";
    return /^(?:https?:|#)/.test(target)
      ? []
      : [posix.normalize(posix.join(prefix, target.split("#")[0] ?? ""))];
  });
  const paths = [
    ...new Set([
      ...shared,
      ...linked,
      ...view.paths.filter(
        (path) =>
          path.startsWith(prefix) &&
          (/\.(?:rs|toml|lock|json|tla|cfg)$/.test(path) ||
            ["README.md", "SUPPORT.md"].includes(path.slice(prefix.length))),
      ),
    ]),
  ].sort();
  const files = Object.fromEntries(
    paths.map((path, index) => [
      `source${index}`,
      { path, role: path === `${prefix}SUPPORT.md` ? "document" : "reference" },
    ]),
  );
  const idFor = (path: string) => {
    const entry = Object.entries(files).find(([, file]) => file.path === path);
    assert.ok(entry, `Unregistered support source: ${path}`);
    return entry[0];
  };
  return parseDocumentManifest({
    schema: "jev-lint-documents-manifest-1",
    roots: [`${prefix}SUPPORT.md`],
    files,
    comparisons: [
      {
        id: `${scope.name}-readme`,
        topic: "記載条件内の対応機能、未対応・対象外、数値や結果の制約",
        mode: "semantic",
        reference: { source: idFor(`${prefix}README.md`) },
        target: { source: idFor(`${prefix}SUPPORT.md`) },
      },
      ...scope.evidence.map((evidence, index) => ({
        id: `${scope.name}-implementation-${index}`,
        topic: evidence.topic,
        mode: "semantic",
        reference: {
          source: idFor(prefix + evidence.path),
          ...(evidence.contains ? { contains: evidence.contains } : {}),
        },
        target: {
          source: idFor(`${prefix}SUPPORT.md`),
          ...(evidence.targetContains
            ? { contains: evidence.targetContains }
            : {}),
        },
        context: (evidence.context ?? []).map((path) => ({
          source: idFor(prefix + path),
          ...(evidence.contextContains
            ? { contains: evidence.contextContains }
            : {}),
        })),
      })),
    ],
  });
};

const snapshot = async (
  view: SourceView,
  manifest: DocumentManifest,
  directory: string,
) => {
  for (const source of Object.values(manifest.files)) {
    await mkdir(dirname(join(directory, source.path)), { recursive: true });
    await writeFile(join(directory, source.path), await view.read(source.path));
  }
  const input = prepareDocuments(manifest, { cwd: directory });
  assert.deepEqual(input.issues, [], JSON.stringify(input.issues));
  assert.equal(input.deferred.length, 0);
  assert.equal(input.unboundDocuments.length, 0);
  assert.equal(input.comparisons.length, input.expectedComparisons);
  return input;
};

/** Build the crate's manifest and snapshot from `view` in a temporary directory. */
const withInput = async <T>(
  view: SourceView,
  name: string,
  use: (
    input: ReturnType<typeof prepareDocuments>,
    manifest: DocumentManifest,
    directory: string,
  ) => Promise<T>,
): Promise<T> => {
  const directory = await mkdtemp(join(tmpdir(), "idealis-support-"));
  try {
    const manifest = await manifestFor(view, name);
    return await use(
      await snapshot(view, manifest, directory),
      manifest,
      directory,
    );
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
};

/**
 * Sync asks Jev only what the crate's ledger cannot answer and appends the new
 * answers; replay reports from the ledger alone. An unrecorded comparison is
 * reported as such and never counts as agreement.
 */
export const reviewSupport = async (
  view: SourceView,
  name: string,
  {
    ledgerDir,
    sync,
    client,
    currentView,
  }: {
    ledgerDir: string;
    sync: boolean;
    client?: AskClient;
    /** The tree as it is now; sync is stale unless it still matches the reviewed snapshot. */
    currentView?: () => SourceView;
  },
): Promise<DocumentReport> =>
  withInput(view, name, async (input, manifest, directory) => {
    const ledger = loadDocumentLedger(ledgerDir);
    for (const error of ledger.loadErrors) console.warn(`${name}: ${error}`);
    if (!sync)
      return replayDocuments(input, ledger, { model: client?.model ?? MODEL });
    const report = await scanDocuments(input, {
      ...(client ? { client } : { clientOptions: { model: MODEL } }),
      ...(currentView
        ? {
            // Compare with the bytes copied into the snapshot, not another
            // read of the working tree, which would follow an edit.
            snapshotCheck: async () => {
              const current = currentView();
              assert.deepEqual(await manifestFor(current, name), manifest);
              for (const source of Object.values(manifest.files))
                assert.deepEqual(
                  await current.read(source.path),
                  await readFile(join(directory, source.path)),
                );
              return true;
            },
          }
        : {}),
      ledger,
    });
    ledger.flush({ model: report.model });
    return report;
  });

/** Fold the crate's ledger into one file holding only the answers its checklist still asks. */
export const compactSupport = async (
  view: SourceView,
  name: string,
  { ledgerDir, dryRun = false }: { ledgerDir: string; dryRun?: boolean },
) =>
  withInput(view, name, async (input) => {
    const ledger = loadDocumentLedger(ledgerDir);
    // The question digest the scan keyed on, as the replay reports it.
    const { questionDigest } = replayDocuments(input, ledger, { model: MODEL });
    const live = new Set(
      input.comparisons.map((c) =>
        documentJudgmentKey(c, questionDigest, MODEL),
      ),
    );
    const all = ledger.keys();
    if (dryRun) {
      const kept = all.filter((key) => live.has(key)).length;
      return { kept, dropped: all.length - kept };
    }
    const { kept, dropped } = ledger.compact({ live, model: MODEL });
    return { kept, dropped };
  });

export const installSupportHook = async (root: string): Promise<void> => {
  const path = resolve(
    root,
    git(root, ["rev-parse", "--git-path", "hooks/pre-commit"])
      .toString()
      .trim(),
  );
  const shim =
    '#!/bin/sh\nhook="$(git rev-parse --show-toplevel)/.jev-lint/hooks/pre-commit" || exit 2\nif [ -x "$hook" ]; then exec "$hook" "$@"; fi\n';
  await mkdir(dirname(path), { recursive: true });
  const present = await lstat(path).catch((error: NodeJS.ErrnoException) => {
    if (error.code !== "ENOENT") throw error;
    return null;
  });
  if (present)
    assert.ok(
      present.isFile() && (await readFile(path, "utf8")) === shim,
      "Existing pre-commit hook kept; invoke .jev-lint/hooks/pre-commit from it",
    );
  else await writeFile(path, shim, { flag: "wx" });
  await chmod(path, 0o755);
};

const run = async () => {
  const args = process.argv.slice(2);
  assert.ok(
    args.every((arg) =>
      [
        "--staged",
        "--sync",
        "--replay",
        "--dry-run",
        "--compact",
        "--install-hook",
      ].includes(arg),
    ),
    "Unknown support-check option",
  );
  const sync = args.includes("--sync");
  assert.ok(
    args.filter((arg) =>
      ["--sync", "--replay", "--dry-run", "--compact"].includes(arg),
    ).length <= 1,
    "Select one Jev review mode",
  );
  if (
    sync &&
    !args.includes("--install-hook") &&
    !process.env.TYPESAFE_API_KEY &&
    !process.env.TYPESAFEAI_API_KEY
  ) {
    console.warn(
      "warning: Jev API key is not set (TYPESAFE_API_KEY / TYPESAFEAI_API_KEY); skipping live checklist review.",
    );
    return;
  }
  const root = git(process.cwd(), ["rev-parse", "--show-toplevel"])
    .toString()
    .trim();
  if (args.includes("--install-hook")) {
    await installSupportHook(root);
    return;
  }
  const staged = args.includes("--staged");
  const view = sourceView(root, staged);
  await checkSupport(
    view,
    scopes.map((scope) => scope.name),
  );
  if (
    !sync &&
    !["--dry-run", "--replay", "--compact"].some((mode) => args.includes(mode))
  ) {
    console.log(
      "Support checklist structure and links checked. Jev review is optional.",
    );
    return;
  }
  let code = 0;
  for (const scope of scopes) {
    const ledgerDir = supportLedger(root, scope.name);
    if (args.includes("--dry-run")) {
      await withInput(view, scope.name, async (input) => {
        console.log(
          `${scope.name}: ${input.comparisons.length} comparisons, ${input.sources.length} sources, ${input.comparisons.reduce((sum, item) => sum + JSON.stringify(item).length, 0)} chars`,
        );
      });
      continue;
    }
    if (args.includes("--compact")) {
      assert.ok(!staged, "Compact the working tree on main, not a staged view");
      const { kept, dropped } = await compactSupport(view, scope.name, {
        ledgerDir,
      });
      console.log(`${scope.name}: kept ${kept}, dropped ${dropped}`);
      continue;
    }
    const report = await reviewSupport(view, scope.name, {
      ledgerDir,
      sync,
      currentView: () => sourceView(root, staged),
    });
    const exit = documentExitCode(report);
    console.log(
      `${scope.name}: ${report.status}, ${report.coverage.answered}/${report.coverage.expected} answered (${report.coverage.reused} from the ledger), ${report.coverage.contradictions} contradictions, ${report.coverage.unassessed} unassessed${sync ? `, $${report.usage.usd.toFixed(6)}` : " (offline)"}`,
    );
    for (const candidate of report.candidates)
      if (candidate.judgment?.choice !== "supports")
        console.log(
          `${candidate.id}: ${candidate.judgment?.choice ?? "unrecorded"}`,
        );
    if (exit) code = exit;
  }
  process.exitCode = code;
};

if (
  process.argv[1] &&
  resolve(process.argv[1]) === fileURLToPath(import.meta.url)
)
  await run().catch((error: unknown) => {
    console.error(error instanceof Error ? error.message : String(error));

    process.exitCode = 3;
  });
