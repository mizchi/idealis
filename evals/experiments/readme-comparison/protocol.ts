import assert from "node:assert/strict";
import { cases } from "../single-model/cases.js";

export { normalizeTla } from "../single-model/tla.js";

export const routes = [
  "tla-ts",
  "tlc-1",
  "tlc-8",
  "wasm",
  "native-1",
  "native-8",
] as const;
export type Route = (typeof routes)[number];

export interface Summary {
  readonly input: string;
  readonly route: string;
  readonly status: string;
  readonly wall: {
    readonly median: number;
    readonly min: number | undefined;
    readonly max: number | undefined;
  } | null;
}
export const renderComparison = (result: {
  readonly status: string;
  readonly summaries: readonly Summary[];
}) => {
  assert.equal(result.status, "complete");
  assert.equal(result.summaries.length, 24);
  const get = (input: string, route: Route) => {
    const rows = result.summaries.filter(
      (summary) => summary.input === input && summary.route === route,
    );
    assert.equal(rows.length, 1);
    const summary = rows[0];
    assert.ok(summary && summary.status === "complete" && summary.wall);
    assert.ok(
      typeof summary.wall.min === "number" &&
        typeof summary.wall.max === "number",
    );
    assert.ok(
      [summary.wall.min, summary.wall.median, summary.wall.max].every(
        (value) => Number.isFinite(value) && value >= 0,
      ),
    );
    assert.ok(
      summary.wall.min <= summary.wall.median &&
        summary.wall.median <= summary.wall.max,
    );
    return `${(summary.wall.median / 1000).toFixed(3)}`;
  };
  const names = ["カウンタ群", "Boolean関数", "チャネル群", "幅1のring"];
  const rows = cases("large");
  for (const input of rows) for (const route of routes) get(input.name, route);
  return {
    combined: [
      "| モデル | 状態数 | tla-ts 1 worker | TLC 1 worker | TLC 8 workers | idealis Wasm 1 worker | idealis native 1 worker | idealis native 8 workers |",
      "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |",
      ...rows.map(
        (input, index) =>
          `| ${names[index]} | ${input.states.toLocaleString("en-US")} | ${routes.map((route) => get(input.name, route)).join(" | ")} |`,
      ),
    ].join("\n"),
    sequential: [
      "| モデル | 状態数 | tla-ts 1 worker | TLC 1 worker | idealis Wasm 1 worker | idealis native 1 worker |",
      "| --- | ---: | ---: | ---: | ---: | ---: |",
      ...rows.map(
        (input, index) =>
          `| ${names[index]} | ${input.states.toLocaleString("en-US")} | ${(["tla-ts", "tlc-1", "wasm", "native-1"] as const).map((route) => get(input.name, route)).join(" | ")} |`,
      ),
    ].join("\n"),
    parallel: [
      "| モデル | TLC 1 worker | TLC 8 workers | idealis native 1 worker | idealis native 8 workers |",
      "| --- | ---: | ---: | ---: | ---: |",
      ...rows.map(
        (input, index) =>
          `| ${names[index]} | ${(["tlc-1", "tlc-8", "native-1", "native-8"] as const).map((route) => get(input.name, route)).join(" | ")} |`,
      ),
    ].join("\n"),
  };
};
