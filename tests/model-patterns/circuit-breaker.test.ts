import assert from "node:assert/strict";
import test from "node:test";
import {
  recordReplay,
  transitionScenario,
  type Scenario,
} from "../../examples/model-patterns/replay.js";

const checker = "crates/rcheck/target/debug/rcheck";
const model = "examples/model-patterns/circuit-breaker/half-open.model";
const requests = ["First", "Second", "Third"] as const;
const generations = ["Initial", "Retry", "Final"] as const;
type Request = (typeof requests)[number];
type Phase = "Closed" | "Open" | "HalfOpen";
type Outcome = "Succeeded" | "Failed";
type Fault = "multiple-probes" | "stale-success";
type Command =
  | { readonly kind: "attempt"; readonly request: Request }
  | { readonly kind: "elapsed" }
  | {
      readonly kind: "complete";
      readonly request: Request;
      readonly outcome: Outcome;
    };

function breaker(fault?: Fault) {
  let phase: Phase = "Closed";
  let generation = 0;
  let probe: Request | null = null;
  const admission = new Map<Request, "Admitted" | "Rejected">();
  const calls = new Map<
    Request,
    {
      generation: number;
      respond: (result: Outcome) => void;
      settled: Promise<void>;
    }
  >();

  function attempt(request: Request) {
    const canSend =
      phase === "Closed" ||
      (phase === "HalfOpen" && (probe === null || fault === "multiple-probes"));
    admission.set(request, canSend ? "Admitted" : "Rejected");
    if (!canSend) return;
    if (phase === "HalfOpen") probe = request;

    const sentIn = generation;
    let respond!: (outcome: Outcome) => void;
    const reply = new Promise<Outcome>((resolve) => {
      respond = resolve;
    });
    const settled = reply.then((outcome) => {
      calls.delete(request);
      const acceptsStaleSuccess =
        fault === "stale-success" &&
        outcome === "Succeeded" &&
        phase === "HalfOpen";
      if (sentIn !== generation && !acceptsStaleSuccess) return;
      if (outcome === "Failed") {
        phase = "Open";
        generation++;
        probe = null;
      } else if (probe === request || acceptsStaleSuccess) {
        phase = "Closed";
        probe = null;
      }
    });
    calls.set(request, { generation: sentIn, respond, settled });
  }

  return {
    async apply(command: Command) {
      switch (command.kind) {
        case "elapsed":
          assert.equal(phase, "Open");
          phase = "HalfOpen";
          break;
        case "attempt":
          attempt(command.request);
          break;
        case "complete": {
          const call = calls.get(command.request);
          assert.ok(call);
          call.respond(command.outcome);
          await call.settled;
          break;
        }
      }
    },
    async observe() {
      return {
        phase,
        generation: generations[generation],
        probe,
        pending: Object.fromEntries(
          requests.map((request) => {
            const call = calls.get(request);
            return [
              request,
              call === undefined ? null : generations[call.generation],
            ];
          }),
        ),
        admission: Object.fromEntries(
          requests.map((request) => [request, admission.get(request) ?? null]),
        ),
      };
    },
    project: (state: Readonly<Record<string, unknown>>) => state,
  };
}

async function replayWithFault(scenario: Scenario<Command>, fault: Fault) {
  const normal = await recordReplay(checker, scenario, breaker());
  assert.equal(normal.verdict, "matched");
  const failed = await recordReplay(checker, scenario, breaker(fault));
  assert.equal(failed.verdict, "mismatch");
  assert.equal(failed.difference.step, scenario.commands.length);
  assert.deepEqual(failed.difference.command, scenario.commands.at(-1));
  return failed.difference;
}

test("a waiting probe causes the next admission attempt to be rejected", async () => {
  const scenario = await transitionScenario<Command>(
    checker,
    model,
    "attempt",
    "before(phase) = HalfOpen and some before(probe)",
  );
  assert.equal(scenario.commands.length, 5);
  const last = scenario.commands.at(-1);
  assert.ok(last?.kind === "attempt");
  const difference = await replayWithFault(scenario, "multiple-probes");
  const expected = difference.expected as {
    admission: Record<Request, string>;
  };
  const observed = difference.observed as {
    admission: Record<Request, string>;
  };
  assert.equal(expected.admission[last.request], "Rejected");
  assert.equal(observed.admission[last.request], "Admitted");
});

test("a successful old request cannot close a breaker testing the current generation", async () => {
  const scenario = await transitionScenario<Command>(
    checker,
    model,
    "complete",
    "before(phase) = HalfOpen and some before(probe) and before(request.pending) != before(generation) and outcome = Succeeded",
  );
  assert.equal(scenario.commands.length, 6);
  const difference = await replayWithFault(scenario, "stale-success");
  const expected = difference.expected as { phase: Phase; probe: Request };
  const observed = difference.observed as {
    phase: Phase;
    probe: Request | null;
  };
  assert.equal(expected.phase, "HalfOpen");
  assert.equal(observed.phase, "Closed");
  assert.ok(expected.probe);
  assert.equal(observed.probe, null);

  const scenarioWithProbe = {
    ...scenario,
    commands: [
      ...scenario.commands,
      { kind: "complete", request: expected.probe, outcome: "Succeeded" },
    ] as const,
  };
  const recovered = await recordReplay(checker, scenarioWithProbe, breaker());
  assert.equal(recovered.verdict, "matched");
  assert.equal(
    (recovered.recording.observations.at(-1) as { phase: Phase }).phase,
    "Closed",
  );

  const failedProbe = await recordReplay(
    checker,
    {
      ...scenario,
      commands: [
        ...scenario.commands,
        { kind: "complete", request: expected.probe, outcome: "Failed" },
      ],
    },
    breaker(),
  );
  assert.equal(failedProbe.verdict, "matched");
  const reopened = failedProbe.recording.observations.at(-1) as {
    phase: Phase;
    generation: string;
    probe: Request | null;
  };
  assert.equal(reopened.phase, "Open");
  assert.equal(reopened.generation, "Final");
  assert.equal(reopened.probe, null);
});
