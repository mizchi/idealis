import assert from "node:assert/strict";
import { fork, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import test from "node:test";
import { fileURLToPath } from "node:url";
import { replay, type Scenario } from "../../examples/model-patterns/replay.js";

type Step = "begin" | "stage" | "commit" | "crash" | "restart" | "respond";
type Command = { readonly kind: Step };
type Observation = {
  phase: string;
  reply?: "Posted" | null;
};

async function message(child: ChildProcess, step?: Step): Promise<Observation> {
  const controller = new AbortController();
  const options = {
    signal: AbortSignal.any([controller.signal, AbortSignal.timeout(5_000)]),
  };
  try {
    const response = once(child, "message", options).then(
      ([value]) => value as Observation,
    );
    const exited = once(child, "exit", options).then(([code, signal]) => {
      throw new Error(
        `transaction child exited before reporting ${step ?? "startup"} (${code ?? signal})`,
      );
    });
    if (step) child.send(step);
    return await Promise.race([response, exited]);
  } finally {
    controller.abort();
  }
}

async function stop(child: ChildProcess) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const exited = once(child, "exit");
  assert.equal(child.kill("SIGKILL"), true);
  const [code, signal] = await exited;
  assert.deepEqual([code, signal], [null, "SIGKILL"]);
}

test(
  "SQLite recovers partial and committed writes across a process crash",
  { timeout: 30_000 },
  async () => {
    const pathRoot = await mkdtemp(join(tmpdir(), "idealis-crash-"));
    try {
      for (const boundary of ["Prepared", "Committed"] as const) {
        for (const split of [false, true]) {
          const path = join(pathRoot, `${boundary}-${split}.sqlite`);
          const setup = new DatabaseSync(path);
          setup.exec(
            "CREATE TABLE ledger (id INTEGER PRIMARY KEY); CREATE TABLE receipt (id INTEGER PRIMARY KEY)",
          );
          setup.close();
          function launch() {
            return fork(
              fileURLToPath(
                new URL("./helpers/transaction-child.js", import.meta.url),
              ),
              [path, split ? "split" : "atomic"],
              { stdio: ["ignore", "ignore", "inherit", "ipc"] },
            );
          }

          const steps: Step[] =
            boundary === "Prepared"
              ? [
                  "begin",
                  "stage",
                  "crash",
                  "restart",
                  "begin",
                  "stage",
                  "commit",
                  "respond",
                ]
              : ["begin", "stage", "commit", "crash", "restart", "respond"];
          const scenario: Scenario<Command> = {
            model: "examples/model-patterns/crash-recovery/transaction.ideal",
            config: {},
            initial: {
              phase: "Idle",
              ledger: null,
              receipt: null,
              reply: null,
            },
            commands: steps.map((kind) => ({ kind })),
          };

          let child = launch();
          try {
            let state = await message(child);
            const run = replay(
              "crates/idealis-check/target/debug/idealis-check",
              scenario,
              {
                async apply({ kind }) {
                  if (kind === "crash") {
                    await stop(child);
                    state = { phase: "Offline" };
                  } else if (kind === "restart") {
                    child = launch();
                    state = await message(child);
                  } else {
                    state = await message(child, kind);
                  }
                },
                async observe() {
                  if (
                    state.phase === "Offline" ||
                    state.phase === "OpenTxn" ||
                    state.phase === "Prepared"
                  ) {
                    return { phase: state.phase };
                  }
                  const db = new DatabaseSync(path, { readOnly: true });
                  try {
                    function marker(table: "ledger" | "receipt") {
                      const count = db
                        .prepare(`SELECT COUNT(*) AS n FROM ${table}`)
                        .get()?.n;
                      return count === 0
                        ? null
                        : count === 1
                          ? "Posted"
                          : "Duplicate";
                    }
                    return {
                      phase: state.phase,
                      ledger: marker("ledger"),
                      receipt: marker("receipt"),
                      reply: state.reply ?? null,
                    };
                  } finally {
                    db.close();
                  }
                },
                project(state) {
                  if (
                    state.phase === "Offline" ||
                    state.phase === "OpenTxn" ||
                    state.phase === "Prepared"
                  ) {
                    return { phase: state.phase };
                  }
                  return {
                    phase: state.phase,
                    ledger: state.ledger,
                    receipt: state.receipt,
                    reply: state.reply,
                  };
                },
              },
            );
            if (split && boundary === "Prepared") {
              await assert.rejects(
                run,
                /"step":4.*"kind":"restart".*"observed":\{"phase":"Idle","ledger":"Posted","receipt":null/,
              );
            } else {
              await run;
            }
          } finally {
            await stop(child);
          }
        }
      }
    } finally {
      await rm(pathRoot, { recursive: true, force: true });
    }
  },
);
