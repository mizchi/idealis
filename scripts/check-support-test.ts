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
import { parseDocumentManifest } from "jev-lint/documents";
import {
  checkSupport,
  installSupportHook,
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

test("pre-commit skips without either key and continues when either alias is set", async () => {
  const root = await mkdtemp(join(tmpdir(), "idealis-support-hook-key-"));
  try {
    const hook = resolve(".jev-lint/hooks/pre-commit");
    const environment = {
      ...process.env,
      TYPESAFE_API_KEY: "",
      TYPESAFEAI_API_KEY: "",
    };
    const skipped = spawnSync("sh", [hook], {
      cwd: root,
      encoding: "utf8",
      env: environment,
    });
    assert.equal(skipped.status, 0, skipped.stderr);
    assert.match(skipped.stderr, /warning:.*Jev.*key.*skipping/i);
    for (const key of ["TYPESAFE_API_KEY", "TYPESAFEAI_API_KEY"]) {
      const continued = spawnSync("sh", [hook], {
        cwd: root,
        encoding: "utf8",
        env: { ...environment, [key]: "offline-hook-test" },
      });
      assert.notEqual(
        continued.status,
        0,
        "the hook should reach Git when a key is present",
      );
      assert.doesNotMatch(continued.stderr, /skipping/i);
      assert.match(continued.stderr, /not a git repository/i);
    }
    assert.deepEqual(await readdir(root), []);
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

test("the real offline gate rejects stale, missing and corrupted staged Jev evidence", async () => {
  const project = process.cwd();
  const view = sourceView(project, false);
  const root = await mkdtemp(join(tmpdir(), "idealis-support-gate-"));
  try {
    execFileSync("git", ["init", "--quiet"], { cwd: root });
    execFileSync("git", ["config", "core.hooksPath", ".git/hooks"], {
      cwd: root,
    });
    const paths = new Set(
      view.paths.filter(
        (path) =>
          /^(?:docs\/|evals\/iterations\/).*\.md$/.test(path) ||
          path === "evals/README.md",
      ),
    );
    const manifests = view.paths.filter((path) =>
      /^\.jev-lint\/checklists\/[^/]+\.manifest.json$/.test(path),
    );
    assert.equal(manifests.length, 8);
    for (const path of manifests) {
      const manifest = parseDocumentManifest(
        JSON.parse((await view.read(path)).toString()),
      );
      paths.add(path);
      paths.add(path.replace(".manifest.json", ".record.json"));
      for (const source of Object.values(manifest.files))
        paths.add(source.path);
    }
    for (const path of paths) {
      await mkdir(dirname(join(root, path)), { recursive: true });
      await writeFile(join(root, path), await view.read(path));
    }
    const stage = () => execFileSync("git", ["add", "."], { cwd: root });
    const check = (staged: boolean) =>
      spawnSync(
        process.execPath,
        [
          "--experimental-strip-types",
          resolve(project, "scripts/check-support.ts"),
          ...(staged ? ["--staged"] : []),
        ],
        {
          cwd: root,
          encoding: "utf8",
          env: { ...process.env, TYPESAFE_API_KEY: "", TYPESAFEAI_API_KEY: "" },
        },
      );
    stage();
    const clean = check(true);
    assert.equal(clean.status, 0, clean.stderr);
    const linkedPath = "docs/safety-ir-v2.md";
    const linked = await readFile(join(root, linkedPath));
    await writeFile(
      join(root, linkedPath),
      Buffer.concat([linked, Buffer.from("\n")]),
    );
    stage();
    assert.notEqual(
      check(true).status,
      0,
      "linked support evidence must invalidate saved records",
    );
    await writeFile(join(root, linkedPath), linked);
    stage();
    assert.equal(check(true).status, 0);
    const fixturePath = "crates/rtla-compiler/tests/finite-support.json";
    const fixture = await readFile(join(root, fixturePath));
    await writeFile(
      join(root, fixturePath),
      Buffer.concat([fixture, Buffer.from("\n")]),
    );
    stage();
    assert.notEqual(
      check(true).status,
      0,
      "compiled fixtures must invalidate saved evidence",
    );
    await writeFile(join(root, fixturePath), fixture);
    stage();
    assert.equal(check(true).status, 0);
    const sourcePath = "crates/rtla/src/error.rs";
    const original = await readFile(join(root, sourcePath));
    await writeFile(
      join(root, sourcePath),
      Buffer.concat([
        original,
        Buffer.from("\n// changed implementation evidence\n"),
      ]),
    );
    assert.equal(
      check(true).status,
      0,
      "unstaged implementation must not affect the commit",
    );
    assert.notEqual(
      check(false).status,
      0,
      "working-tree review must detect the changed source",
    );
    stage();
    const checklistPath = "crates/rtla/SUPPORT.md";
    const checklist = await readFile(join(root, checklistPath));
    await writeFile(
      join(root, checklistPath),
      Buffer.concat([checklist, Buffer.from("\nunstaged correction\n")]),
    );
    assert.notEqual(
      check(true).status,
      0,
      "unstaged text cannot repair a stale staged snapshot",
    );
    await writeFile(join(root, sourcePath), original);
    await writeFile(join(root, checklistPath), checklist);
    stage();
    assert.equal(check(true).status, 0);
    const recordPath = "./.jev-lint/checklists/rtla.record.json";
    const record = await readFile(join(root, recordPath));
    execFileSync("git", ["update-index", "--force-remove", recordPath], {
      cwd: root,
    });
    assert.notEqual(
      check(true).status,
      0,
      "unstaged record cannot replace a missing staged record",
    );
    stage();
    await writeFile(join(root, recordPath), "{}\n");
    stage();
    assert.notEqual(
      check(true).status,
      0,
      "corrupted records cannot approve a commit",
    );
    await writeFile(join(root, recordPath), record);
    stage();
    assert.equal(check(true).status, 0);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
