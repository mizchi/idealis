import assert from "node:assert/strict";
import { execFileSync, spawnSync } from "node:child_process";
import {
  mkdir,
  mkdtemp,
  readdir,
  readFile,
  rm,
  writeFile,
} from "node:fs/promises";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { test } from "node:test";
import { Jev } from "jev-lint/client";
import { documentExitCode } from "jev-lint/documents";
import {
  checkSupport,
  compactSupport,
  installSupportHook,
  reviewSupport,
  sourceView,
} from "./check-support.js";

const fixture = async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-checklist-test-"));
  execFileSync("git", ["init", "--quiet"], { cwd: root });
  execFileSync("git", ["config", "core.hooksPath", ".git/hooks"], {
    cwd: root,
  });
  await mkdir(join(root, "crates/demo/src"), { recursive: true });
  await writeFile(
    join(root, "crates/demo/Cargo.toml"),
    '[package]\nname = "demo"\nversion = "0.1.0"\n',
  );
  await writeFile(
    join(root, "crates/demo/src/lib.rs"),
    "pub fn supported() {}\n",
  );
  await writeFile(
    join(root, "crates/demo/SUPPORT.md"),
    "# demo\n\n- [x] supported\n- [ ] unimplemented\n\n[source](src/lib.rs)\n",
  );
  execFileSync("git", ["add", "."], { cwd: root });
  return root;
};

test("sync without a Jev key warns and skips before reading or writing the repository", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-support-no-key-"));
  try {
    for (const args of [["--sync"], ["--sync", "--staged"]]) {
      const result = spawnSync(
        process.execPath,
        [
          "--experimental-strip-types",
          resolve("scripts/check-support.ts"),
          ...args,
        ],
        {
          cwd: root,
          encoding: "utf8",
          env: { ...process.env, TYPESAFE_API_KEY: "", TYPESAFEAI_API_KEY: "" },
        },
      );
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stderr, /warning:.*Jev.*key.*skipping/i);
      assert.deepEqual(await readdir(root), []);
    }
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("a real index is authoritative over unstaged checklist edits", async () => {
  const root = await fixture();
  try {
    const original = await readFile(join(root, "crates/demo/SUPPORT.md"));
    await writeFile(
      join(root, "crates/demo/SUPPORT.md"),
      "unstaged incomplete document\n",
    );
    await checkSupport(sourceView(root, true), ["demo"]);
    await assert.rejects(
      checkSupport(sourceView(root, false), ["demo"]),
      /supported checklist/,
    );
    assert.match(
      (await sourceView(root, true).read("crates/demo/SUPPORT.md")).toString(),
      /\[x\] supported/,
    );
    execFileSync("git", ["add", "."], { cwd: root });
    await writeFile(join(root, "crates/demo/SUPPORT.md"), original);
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /supported checklist/,
    );
    await checkSupport(sourceView(root, false), ["demo"]);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("missing checklists, new crates and dangling staged evidence are rejected", async () => {
  const root = await fixture();
  try {
    await checkSupport(sourceView(root, true), ["demo"]);
    execFileSync(
      "git",
      ["rm", "--cached", "--quiet", "crates/demo/SUPPORT.md"],
      {
        cwd: root,
      },
    );
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /Missing.*staged source/,
    );
    await writeFile(
      join(root, "crates/demo/SUPPORT.md"),
      "# demo\n- [x] supported\n- [ ] not supported\n[source](absent.rs)\n",
    );
    execFileSync("git", ["add", "."], { cwd: root });
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /link absent/,
    );
    await mkdir(join(root, "crates/extra"));
    await writeFile(
      join(root, "crates/extra/Cargo.toml"),
      '[package]\nname="extra"\n',
    );
    execFileSync("git", ["add", "."], { cwd: root });
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /inventory changed/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("staged symlink sources are not followed", async () => {
  const root = await fixture();
  try {
    const blob = execFileSync("git", ["hash-object", "-w", "--stdin"], {
      cwd: root,
      input: "/etc/passwd",
    })
      .toString()
      .trim();
    execFileSync(
      "git",
      [
        "update-index",
        "--add",
        "--cacheinfo",
        `120000,${blob},crates/demo/SUPPORT.md`,
      ],
      { cwd: root },
    );
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /non-regular staged source/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("installing a hook is repeatable and preserves another tool's hook", async () => {
  const root = await fixture();
  try {
    await installSupportHook(root);
    await installSupportHook(root);
    const path = join(root, ".git/hooks/pre-commit");
    await writeFile(path, "#!/bin/sh\nexit 7\n");
    await assert.rejects(
      installSupportHook(root),
      /Existing pre-commit hook kept/,
    );
    assert.equal(await readFile(path, "utf8"), "#!/bin/sh\nexit 7\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("a checklist made only of examples in a code fence is not a support declaration", async () => {
  const root = await fixture();
  try {
    await writeFile(
      join(root, "crates/demo/SUPPORT.md"),
      "# demo\n\n```md\n- [x] sample\n- [ ] sample\n```\n",
    );
    execFileSync("git", ["add", "."], { cwd: root });
    await assert.rejects(
      checkSupport(sourceView(root, true), ["demo"]),
      /supported checklist/,
    );
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("CI and staged checks run without Jev evidence and ignore a broken ledger", async () => {
  const project = process.cwd();
  const root = await mkdtemp(join(tmpdir(), "idealis-support-advisory-"));
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root });
    const view = sourceView(project, false);
    const supportFiles = view.paths.filter((path) =>
      /^crates\/[^/]+\/SUPPORT\.md$/.test(path),
    );
    const paths = new Set([".gitignore"]);
    for (const path of supportFiles) {
      paths.add(path);
      paths.add(join(dirname(path), "Cargo.toml"));
      const content = (await view.read(path)).toString();
      for (const match of content.matchAll(/\]\(([^)]+)\)/g)) {
        const target = match[1]?.split("#")[0];
        if (target && !/^https?:/.test(target))
          paths.add(join(dirname(path), target));
      }
    }
    for (const path of paths) {
      await mkdir(dirname(join(root, path)), { recursive: true });
      await writeFile(join(root, path), await view.read(path));
    }
    const check = (...args: string[]) =>
      spawnSync(
        process.execPath,
        [
          "--experimental-strip-types",
          resolve(project, "scripts/check-support.ts"),
          ...args,
        ],
        { cwd: root, encoding: "utf8" },
      );
    execFileSync("git", ["add", "."], { cwd: root });
    const fresh = check();
    assert.equal(fresh.status, 0, fresh.stderr);
    const local =
      ".jev-lint/checklists/rtla/20260101T000000000Z-00000000.jsonl";
    await mkdir(dirname(join(root, local)), { recursive: true });
    await writeFile(join(root, local), "invalid JSON");
    const staged = check("--staged");
    assert.equal(staged.status, 0, staged.stderr);
    await writeFile(join(root, "crates/rtla/SUPPORT.md"), "# incomplete");
    assert.notEqual(check().status, 0);
    assert.equal(check("--staged").status, 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

const answering = (onAsk: () => void = () => {}) =>
  new Jev({
    apiKey: "fixture",
    model: "jev-1.14.0",
    fetch: async () => {
      onAsk();
      return Response.json({
        model: "jev-1.14.0",
        answers: {
          q0000: {
            type: "choice",
            choice: "supports",
            confidence: 1,
            probabilities: {
              supports: 1,
              contradicts: 0,
              unrelated: 0,
              insufficient_context: 0,
            },
          },
        },
      });
    },
  });

test("Jev review replays from the crate ledger, reuses recorded answers and never passes unrecorded comparisons", async () => {
  const view = sourceView(process.cwd(), false);
  const ledgerDir = await mkdtemp(join(tmpdir(), "idealis-support-ledger-"));
  try {
    const empty = await reviewSupport(view, "rtla", { ledgerDir, sync: false });
    assert.equal(documentExitCode(empty), 3);
    assert.ok(empty.errors.every((error) => error.code === "unrecorded"));

    let asked = 0;
    const synced = await reviewSupport(view, "rtla", {
      ledgerDir,
      sync: true,
      client: answering(() => asked++),
    });
    assert.equal(synced.status, "complete");
    assert.equal(asked, synced.coverage.expected);
    assert.equal((await readdir(ledgerDir)).length, 1);

    const again = await reviewSupport(view, "rtla", {
      ledgerDir,
      sync: true,
      client: answering(() =>
        assert.fail("a recorded comparison is asked again"),
      ),
    });
    assert.equal(again.coverage.reused, again.coverage.expected);
    assert.equal(
      (await readdir(ledgerDir)).length,
      1,
      "a run that learns nothing writes nothing",
    );

    const replayed = await reviewSupport(view, "rtla", {
      ledgerDir,
      sync: false,
    });
    assert.equal(replayed.status, "complete");
    assert.equal(replayed.usage.requests, 0);
  } finally {
    await rm(ledgerDir, { recursive: true, force: true });
  }
});

test("compaction keeps only the answers the current checklist still asks", async () => {
  const view = sourceView(process.cwd(), false);
  const ledgerDir = await mkdtemp(join(tmpdir(), "idealis-support-ledger-"));
  try {
    await reviewSupport(view, "rtla", {
      ledgerDir,
      sync: true,
      client: answering(),
    });
    await writeFile(
      join(ledgerDir, "19700101T000000000Z-00000000.jsonl"),
      `${JSON.stringify({ schema: "jev-lint-ledger-1", namespace: "jev-lint-documents-judgments-1" })}\n${JSON.stringify(
        {
          key: "0".repeat(64),
          at: "1970-01-01T00:00:00.000Z",
          value: {
            judgment: {
              choice: "supports",
              confidence: 1,
              probabilities: {
                supports: 1,
                contradicts: 0,
                unrelated: 0,
                insufficient_context: 0,
              },
            },
            comparisonId: "gone",
            target: "x",
          },
        },
      )}\n`,
    );
    const result = await compactSupport(view, "rtla", { ledgerDir });
    assert.equal(result.dropped, 1);
    assert.equal((await readdir(ledgerDir)).length, 1);
    assert.equal(
      (await reviewSupport(view, "rtla", { ledgerDir, sync: false })).status,
      "complete",
    );
  } finally {
    await rm(ledgerDir, { recursive: true, force: true });
  }
});
