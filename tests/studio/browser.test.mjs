import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { cp, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import test from "node:test";
import { chromium, expect } from "@playwright/test";

const example = "examples/model-patterns/worker-microblog";
const server = resolve("tools/idealis-studio/server.mjs");
const checker = resolve("crates/idealis-check/target/debug/idealis-check");

test(
  "Studio opens linked rules and refreshes the diff and evidence after a save",
  { timeout: 30_000 },
  async (t) => {
    const root = await mkdtemp(join(tmpdir(), "idealis-studio-browser-"));
    t.after(() => rm(root, { recursive: true, force: true }));
    await cp(example, join(root, example), { recursive: true });
    const child = spawn(process.execPath, [server], {
      cwd: root,
      env: {
        ...process.env,
        IDEALIS_STUDIO_PORT: "0",
        IDEALIS_CHECKER: checker,
      },
      stdio: ["ignore", "pipe", "inherit"],
    });
    const stopped = once(child, "exit");
    t.after(async () => {
      child.kill();
      await stopped;
    });
    const address = await Promise.race([
      once(child.stdout, "data").then(
        ([chunk]) => chunk.toString().match(/http:\/\/127\.0\.0\.1:\d+/)?.[0],
      ),
      stopped.then(([code]) => {
        throw new Error("Studio exited: " + code);
      }),
    ]);
    assert.ok(address);
    const browser = await chromium.launch();
    t.after(() => browser.close());
    const page = await browser.newPage({
      viewport: { width: 1440, height: 950 },
    });
    const errors = [];
    page.on("pageerror", (error) => errors.push(error.message));
    await page.goto(address);
    await expect(
      page.getByRole("heading", { name: "WorkerMicroblog" }),
    ).toBeVisible();
    const baseline = await fetch(address + "/api/workspace").then((response) =>
      response.json(),
    );
    assert.equal(baseline.preview.files.length, 0);

    const graph = page.getByRole("region", { name: "構成モデルの関係図" });
    await expect(graph.getByRole("region", { name: "配信" })).toBeVisible();
    await expect(
      graph.getByRole("region", { name: "タイムライン" }),
    ).toBeVisible();
    await expect(
      graph.getByRole("button", {
        name: "relay → queue: 配信依頼",
        exact: true,
      }),
    ).not.toHaveClass(/is-cross-domain/);
    await expect(
      graph.getByRole("button", {
        name: "posts → relay: 投稿者の記録",
        exact: true,
      }),
    ).toHaveClass(/is-cross-domain/);

    await expect(graph).toBeVisible();
    const canvasFrame = await graph.boundingBox();
    assert.ok(canvasFrame.width > 1440 * 0.9);
    assert.ok(canvasFrame.height > 950 * 0.7);
    await expect(
      page.getByRole("navigation", { name: "ワークスペース" }),
    ).toBeHidden();
    await expect(
      page.getByRole("region", { name: "モデル間の接続" }),
    ).toHaveCount(0);

    await expect(
      graph.getByRole("button", { name: "queue", exact: true }),
    ).toBeVisible();
    await expect(
      graph.getByText("初回配信・再配信・ACK を管理する。", { exact: true }),
    ).toBeVisible();
    await graph.getByLabel("説明の言語").selectOption("en");
    await expect(
      graph.getByText(
        "Manages first delivery, retries, and acknowledgements.",
        { exact: true },
      ),
    ).toBeVisible();
    await expect(
      graph.getByRole("button", {
        name: "users → timeline: follow relationship",
        exact: true,
      }),
    ).toBeVisible();
    await graph.getByLabel("説明の言語").selectOption("ja");
    const stage = graph.locator(".graph-stage");
    const beforeZoom = await stage.boundingBox();
    assert.ok(beforeZoom);
    const canvasViewport = graph.locator(".graph-scroll");
    const viewportBox = await canvasViewport.boundingBox();
    assert.ok(viewportBox);
    const startDrag = { x: viewportBox.x + 102, y: viewportBox.y + 24 };
    await page.mouse.move(startDrag.x, startDrag.y);
    await page.mouse.down();
    await page.mouse.move(startDrag.x - 70, startDrag.y + 54, { steps: 5 });
    await page.mouse.up();
    const moved = await stage.boundingBox();
    assert.ok(Math.abs(moved.x - beforeZoom.x + 70) < 3);
    assert.ok(Math.abs(moved.y - beforeZoom.y - 54) < 3);
    await graph.getByRole("button", { name: "ズームをリセット" }).click();
    const reset = await stage.boundingBox();
    assert.ok(Math.abs(reset.x - beforeZoom.x) < 3);
    assert.ok(Math.abs(reset.y - beforeZoom.y) < 3);
    const cursor = {
      x: beforeZoom.x + beforeZoom.width / 2,
      y: beforeZoom.y + beforeZoom.height / 2,
    };
    await page.mouse.move(cursor.x, cursor.y);
    await page.mouse.wheel(0, -300);
    await expect
      .poll(async () => (await stage.boundingBox()).width)
      .toBeGreaterThan(beforeZoom.width);
    const enlarged = await stage.boundingBox();
    assert.ok(Math.abs(enlarged.x + enlarged.width / 2 - cursor.x) < 3);
    assert.ok(Math.abs(enlarged.y + enlarged.height / 2 - cursor.y) < 3);
    await page.mouse.wheel(0, 300);
    await expect
      .poll(async () =>
        Math.abs((await stage.boundingBox()).width - beforeZoom.width),
      )
      .toBeLessThan(3);
    const followsEdge = graph.getByRole("button", {
      name: "users → timeline: フォロー関係",
      exact: true,
    });
    await expect(followsEdge).toBeVisible();
    await followsEdge.click();
    const followConnection = page.getByRole("region", {
      name: "users → timeline の接続",
    });
    await expect(
      followConnection.getByText("follows", { exact: true }),
    ).toBeVisible();
    await expect(
      graph.getByRole("button", {
        name: "consumer → queue: ACK 要求",
        exact: true,
      }),
    ).toHaveCount(0);
    await graph.getByRole("button", { name: "絞り込みを解除" }).click();
    await expect(
      graph.getByRole("button", {
        name: "queue → relay: 受付済み",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      graph.getByRole("button", {
        name: "relay → queue: 配信依頼",
        exact: true,
      }),
    ).toBeVisible();
    await graph.getByRole("button", { name: "queue", exact: true }).click();
    const connections = page.getByRole("region", { name: "queue の接続" });
    await expect(
      connections.getByText("queue → timeline", { exact: true }),
    ).toBeVisible();
    await expect(
      connections.getByText("consumer → queue", { exact: true }),
    ).toBeVisible();
    await expect(
      connections.getByRole("button", { name: "ackRequests", exact: true }),
    ).toBeVisible();
    await expect(
      connections.getByText("posts → timeline", { exact: true }),
    ).toHaveCount(0);
    await page.getByLabel("共有状態で絞り込む").selectOption("ackRequests");
    await expect(
      connections.getByText("consumer → queue", { exact: true }),
    ).toBeVisible();
    await expect(
      connections.getByText("queue → timeline", { exact: true }),
    ).toHaveCount(0);
    await graph.getByRole("button", { name: "絞り込みを解除" }).click();
    await graph.getByRole("button", { name: "timeline", exact: true }).click();
    const timelinePanel = page.getByRole("region", { name: "timeline の接続" });
    await expect(
      timelinePanel.getByRole("heading", { name: "内部状態", exact: true }),
    ).toBeVisible();
    await expect(
      timelinePanel.getByRole("button", {
        name: "appliedOnInitial",
        exact: true,
      }),
    ).toBeVisible();
    await expect(
      timelinePanel.getByText("set Post", { exact: true }).first(),
    ).toBeVisible();
    await timelinePanel
      .getByRole("button", { name: "inFlight", exact: true })
      .click();
    await expect(graph.getByLabel("共有状態で絞り込む")).toHaveValue(
      "inFlight",
    );
    await expect(graph.locator(".graph-wire-label")).toHaveCount(2);
    const inFlight = timelinePanel.locator(".graph-state-list li").filter({
      has: page.getByRole("button", { name: "inFlight", exact: true }),
    });
    await expect(
      inFlight.getByText("書く queue", { exact: true }),
    ).toBeVisible();
    await expect(
      inFlight.getByText("読む consumer", { exact: true }),
    ).toBeVisible();
    await graph.getByRole("button", { name: "絞り込みを解除" }).click();
    await page
      .getByRole("button", { name: "モデルと仕様", exact: true })
      .click();

    const main = page.getByRole("main");
    await main
      .getByRole("button", { name: "AckAfterWrite", exact: true })
      .click();
    await expect(
      main.getByRole("heading", { name: "AckAfterWrite", exact: true }),
    ).toBeVisible();
    await main
      .getByRole("button", { name: "MICROBLOG-ACK-AFTER-WRITE", exact: true })
      .click();
    await expect(
      main.getByRole("heading", { name: /反映処理を済ませて/ }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "差分プレビュー", exact: true })
      .click();

    const file = join(root, example, "consumer.ideal");
    const source = await readFile(file, "utf8");
    await writeFile(file, source.replace("    p in @applied,\n", ""));
    await expect(
      page.getByRole("button", { name: "差分プレビュー (1)", exact: true }),
    ).toBeVisible();
    await expect(
      main.getByText("consumer_requestAck", { exact: true }),
    ).toBeVisible();
    await expect(
      main.locator(".minus").filter({ hasText: "p in @applied" }),
    ).toBeVisible();
    await page
      .getByRole("button", { name: "モデルと仕様", exact: true })
      .click();
    await main
      .getByRole("button", { name: "AckAfterWrite", exact: true })
      .click();
    await main.getByRole("button", { name: "現在のモデルを検査" }).click();
    await expect(main.getByText("違反", { exact: true })).toBeVisible();
    await expect(main.getByText(/queue_ack/)).toBeVisible();

    await page.setViewportSize({ width: 390, height: 840 });
    await page
      .getByRole("button", { name: "モデル関係図", exact: true })
      .click();
    await expect(
      graph.getByRole("button", { name: "consumer、保存済みの変更あり" }),
    ).toBeVisible();
    assert.equal(
      await page.evaluate(() => document.documentElement.scrollWidth),
      390,
    );
    await writeFile(file, source);
    await expect(
      graph.getByRole("button", { name: "consumer", exact: true }),
    ).toBeVisible();
    await expect(main.getByText("違反", { exact: true })).toHaveCount(0);
    await page
      .getByRole("button", { name: "差分プレビュー", exact: true })
      .click();
    await expect(main.getByText("保存済みの変更はありません。")).toBeVisible();
    assert.equal(
      await page.evaluate(() => document.documentElement.scrollWidth),
      390,
    );
    assert.deepEqual(errors, []);
  },
);
