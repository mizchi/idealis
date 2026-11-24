export interface Transition<State, Observation> {
  readonly state: State;
  readonly observation: Observation;
}

export interface Model<State, Action, Observation> {
  readonly name: string;
  readonly initial: () => readonly Transition<State, Observation>[];
  readonly actions: (state: State) => readonly Action[];
  readonly step: (
    state: State,
    action: Action,
  ) => readonly Transition<State, Observation>[];
  readonly key: (state: State) => string;
  readonly invariants?: readonly {
    readonly name: string;
    readonly check: (state: State) => boolean;
  }[];
}

export interface Driver<Action, Observation> {
  readonly reset: () => Observation | Promise<Observation>;
  readonly step: (action: Action) => Observation | Promise<Observation>;
  readonly dispose?: () => void | Promise<void>;
}

export interface Trace<Action, Observation> {
  readonly version: 1;
  readonly model: string;
  readonly initial: Observation;
  readonly steps: readonly {
    readonly action: Action;
    readonly observation: Observation;
  }[];
}

export interface ConformanceResult<Action, Observation> {
  readonly status: "matched" | "mismatch" | "invalid";
  readonly checkedSteps: number;
  readonly actual: Observation;
  readonly expected: readonly Observation[];
  readonly trace: Trace<Action, Observation>;
}

export interface Comparison<Observation> {
  readonly equal?: (expected: Observation, actual: Observation) => boolean;
}

export type PortableValue =
  | { readonly type: "null" }
  | { readonly type: "bool"; readonly value: boolean }
  | {
      readonly type: "int" | "string" | "bytes" | "model-value";
      readonly value: string;
    }
  | { readonly type: "list" | "set"; readonly value: readonly PortableValue[] }
  | {
      readonly type: "record";
      readonly value: readonly (readonly [string, PortableValue])[];
    }
  | {
      readonly type: "map";
      readonly value: readonly (readonly [PortableValue, PortableValue])[];
    };

export interface PortableTrace {
  readonly format: "modelt.portable-trace";
  readonly version: 1;
  readonly model: string;
  readonly initial: PortableValue;
  readonly steps: readonly {
    readonly action: PortableValue;
    readonly observation: PortableValue;
  }[];
}

export interface PortableResult {
  readonly status: "matched" | "mismatch" | "invalid";
  readonly checkedSteps: number;
  readonly actual: PortableValue;
  readonly expected: readonly PortableValue[];
  readonly trace: PortableTrace;
}

export interface PortableState {
  readonly key: string;
  readonly state: PortableValue;
}

export interface PortableTransition extends PortableState {
  readonly observation: PortableValue;
}

export type PortableResponse =
  | {
      readonly kind: "transitions";
      readonly transitions: readonly PortableTransition[];
    }
  | { readonly kind: "observation"; readonly observation: PortableValue };

export type PortableEffect =
  | { readonly kind: "model-initial" | "driver-reset"; readonly id: number }
  | {
      readonly kind: "model-step";
      readonly id: number;
      readonly states: readonly PortableState[];
      readonly action: PortableValue;
    }
  | {
      readonly kind: "driver-step";
      readonly id: number;
      readonly action: PortableValue;
    }
  | { readonly kind: "done"; readonly result: PortableResult };

export interface TlaSourceOptions {
  readonly source: string;
  readonly config: string;
  readonly irVersion?: 1 | 2;
  readonly syntaxVersion?: "1.7.4" | "1.8.0";
}

export interface TlaSourceInput extends TlaSourceOptions {
  readonly modules?: Readonly<Record<string, string>>;
}

export interface PortableCommand {
  readonly version: 1;
  readonly request:
    | {
        readonly kind: "compile-tla";
        readonly source: string;
        readonly config: string;
        readonly modules?: Readonly<Record<string, string>>;
        readonly irVersion?: 1 | 2;
        readonly syntaxVersion?: "1.7.4" | "1.8.0";
      }
    | {
        readonly kind: "check-tla";
        readonly source: string;
        readonly config: string;
        readonly modules?: Readonly<Record<string, string>>;
        readonly irVersion?: 1 | 2;
        readonly syntaxVersion?: "1.7.4" | "1.8.0";
        readonly bounds: SafetyBounds;
      }
    | {
        readonly kind: "check-safety-parallel";
        readonly program: SafetyProgram;
        readonly bounds: SafetyBounds;
        readonly workers: number;
      }
    | {
        readonly kind: "check-safety";
        readonly program: SafetyProgram;
        readonly bounds: SafetyBounds;
        readonly options?: SafetyOptions;
      }
    | {
        readonly kind: "check-safety-coverage";
        readonly program: SafetyProgram;
        readonly bounds: SafetyBounds;
      }
    | { readonly kind: "prepare-safety"; readonly program: SafetyProgram }
    | {
        readonly kind: "check-prepared-safety";
        readonly handle: number;
        readonly bounds: SafetyBounds;
        readonly options?: SafetyOptions;
      }
    | {
        readonly kind: "check-prepared-safety-batch";
        readonly handle: number;
        readonly jobs: readonly SafetyCheck[];
      }
    | { readonly kind: "release-safety"; readonly handle: number }
    | {
        readonly kind: "check-safety-incremental";
        readonly enabled?: boolean;
        readonly program: SafetyProgram;
        readonly bounds: SafetyBounds;
        readonly cache: SafetyExpansionCache;
        readonly buildDigest: string;
        readonly maxEntries: number;
      }
    | {
        readonly kind: "merge-safety-expansions";
        readonly left: SafetyExpansionCache;
        readonly right: SafetyExpansionCache;
        readonly maxEntries: number;
      }
    | { readonly kind: "load-model"; readonly model: FiniteModel }
    | { readonly kind: "release-model"; readonly handle: number }
    | {
        readonly kind: "start-loaded";
        readonly handle: number;
        readonly actions: readonly PortableValue[];
      }
    | {
        readonly kind: "check-loaded";
        readonly handle: number;
        readonly traces: readonly PortableTrace[];
      }
    | {
        readonly kind: "check-history";
        readonly handle: number;
        readonly history: ConcurrentHistory;
        readonly maxNodes: number;
      }
    | { readonly kind: "canonicalize"; readonly value: PortableValue }
    | { readonly kind: "replay-host"; readonly tape: HostTape }
    | {
        readonly kind: "start-finite";
        readonly model: FiniteModel;
        readonly actions: readonly PortableValue[];
      }
    | {
        readonly kind: "check-finite";
        readonly model: FiniteModel;
        readonly traces: readonly PortableTrace[];
      }
    | { readonly kind: "shrink-candidates"; readonly length: number }
    | {
        readonly kind: "start";
        readonly model: string;
        readonly actions: readonly PortableValue[];
      }
    | { readonly kind: "replay"; readonly trace: PortableTrace }
    | { readonly kind: "cancel"; readonly id: number }
    | {
        readonly kind: "resume";
        readonly id: number;
        readonly response: PortableResponse;
      };
}

export type PortableOutput =
  | { readonly kind: "compiled"; readonly program: SafetyProgram }
  | { readonly kind: "safety"; readonly result: SafetyResult }
  | { readonly kind: "safety-coverage"; readonly report: SafetyCoverageReport }
  | {
      readonly kind: "incremental-safety";
      readonly result: SafetyResult;
      readonly cache: SafetyExpansionCache;
      readonly stats: ExpansionStats;
    }
  | { readonly kind: "safety-expansions"; readonly cache: SafetyExpansionCache }
  | { readonly kind: "loaded"; readonly handle: number }
  | { readonly kind: "safety-prepared"; readonly handle: number }
  | {
      readonly kind: "safety-batch";
      readonly results: readonly SafetyBatchResult[];
    }
  | { readonly kind: "released" }
  | { readonly kind: "history"; readonly result: ConcurrentResult }
  | { readonly kind: "host"; readonly snapshot: PortableValue }
  | { readonly kind: "results"; readonly results: readonly PortableResult[] }
  | { readonly kind: "candidates"; readonly ranges: readonly DeletionRange[] }
  | { readonly kind: "cancelled" }
  | { readonly kind: "value"; readonly value: PortableValue }
  | { readonly kind: "effect"; readonly effect: PortableEffect };

export interface SafetyExpansionCache {
  readonly format: "idealis.safety-expansions";
  readonly version: 1;
  readonly buildDigest: string;
  readonly transitionIdentity: string;
  readonly entries: Readonly<
    Record<
      string,
      {
        readonly digest: string;
        readonly successors: readonly {
          readonly action: number;
          readonly values: readonly PortableValue[];
        }[];
      }
    >
  >;
}
export interface ExpansionStats {
  readonly hits: number;
  readonly misses: number;
  readonly writes: number;
  readonly invalidated: boolean;
}
export interface IncrementalSafetyResult {
  readonly result: SafetyResult;
  readonly cache: SafetyExpansionCache;
  readonly stats: ExpansionStats;
}

export interface PortableCore {
  readonly request: (command: PortableCommand) => PortableOutput;
}

export interface SafetyCheck {
  readonly bounds: SafetyBounds;
  readonly options?: SafetyOptions;
}

export type SafetyBatchResult =
  | { readonly kind: "safety"; readonly result: SafetyResult }
  | {
      readonly kind: "error";
      readonly code: "protocol" | "value";
      readonly message: string;
    };

export interface PreparedSafetyProgram {
  readonly check: (
    bounds: SafetyBounds,
    options?: SafetyOptions,
  ) => SafetyResult;
  readonly checkBatch: (
    jobs: readonly SafetyCheck[],
  ) => readonly SafetyBatchResult[];
  readonly release: () => void;
}

export interface BackendIdentity {
  readonly id: string;
  readonly version: string;
}

export interface BackendScope {
  readonly modelId: string;
  readonly propertyId: string;
  readonly semantics: string;
  readonly assumptions: readonly string[];
}

export type BackendQuery =
  | { readonly kind: "explicit"; readonly bounds: SafetyBounds }
  | { readonly kind: "bounded"; readonly maxDepth: number }
  | {
      readonly kind: "induction";
      readonly invariantId: string;
      readonly obligation: "init" | "step" | "implication" | "all";
    };

export interface BackendRequest<Input = unknown> {
  readonly format: "modelt.backend-request";
  readonly version: 1;
  readonly scope: BackendScope;
  readonly query: BackendQuery;
  readonly input: Input;
}

export interface BackendBehaviorWitness {
  readonly kind: "behavior";
  readonly violation:
    | "invariant-violation"
    | "property-violation"
    | "liveness-violation"
    | "deadlock";
  readonly name: string;
  readonly variables: readonly string[];
  readonly trace: NonNullable<SafetyResult["violation"]>["trace"];
  readonly loopStart?: number;
  readonly loopAction?: string;
}

export type BackendOutcome =
  | { readonly kind: "explicit-complete"; readonly states: number }
  | { readonly kind: "bounded-no-counterexample"; readonly maxDepth: number }
  | { readonly kind: "obligation-holds" }
  | {
      readonly kind: "inductive";
      readonly obligations: readonly BackendResult[];
    }
  | {
      readonly kind: "counterexample";
      readonly witness:
        | BackendBehaviorWitness
        | { readonly kind: "obligation"; readonly assignment: PortableValue };
    }
  | { readonly kind: "empty" }
  | {
      readonly kind: "inconclusive";
      readonly reason:
        | "depth"
        | "states"
        | "work"
        | "temporal"
        | "solver-unknown"
        | "cancelled"
        | "unsupported"
        | "initialization-unknown"
        | "evaluation"
        | "obligations";
      readonly obligations?: readonly BackendResult[];
    }
  | { readonly kind: "error"; readonly code: string; readonly message: string };

export interface BackendResult {
  readonly format: "modelt.backend-result";
  readonly version: 1;
  readonly backend: BackendIdentity;
  readonly scope: BackendScope;
  readonly query: BackendQuery;
  readonly initialization: "nonempty" | "empty" | "unknown";
  readonly outcome: BackendOutcome;
  readonly progress?: {
    readonly states: number;
    readonly generated: number;
    readonly depth: number;
    readonly work: number;
  };
}

export interface BackendDescriptor extends BackendIdentity {
  readonly queries: readonly BackendQuery["kind"][];
  readonly semantics: readonly string[];
}

export interface VerificationBackend<Input = unknown> {
  readonly descriptor: BackendDescriptor;
  readonly check: (request: BackendRequest<Input>) => Promise<BackendResult>;
}

export interface CheckCacheJob {
  readonly buildDigest: string;
  readonly backend: BackendDescriptor;
  readonly request: BackendRequest;
}

export interface CheckCacheArtifact {
  readonly format: "idealis.check-cache-artifact";
  readonly version: 1;
  readonly identity: string;
  readonly owner: string;
  readonly runId: string;
  readonly completedAt: number;
  readonly result: BackendResult;
  readonly digest: string;
}

export interface CheckCacheCell {
  readonly version: 1;
  readonly revision: number;
  readonly identity: string;
  readonly fence: number;
  readonly phase: "idle" | "leased" | "ready";
  readonly owner: string;
  readonly expiresAt: number;
  readonly artifact: CheckCacheArtifact | null;
}

export interface CheckCacheStore {
  readonly read: (key: string) => Promise<CheckCacheCell | null>;
  readonly compareExchange: (
    key: string,
    expectedRevision: number | null,
    next: CheckCacheCell,
  ) => Promise<boolean>;
}

export interface CheckCacheLease {
  readonly key: string;
  readonly identity: string;
  readonly fence: number;
  readonly owner: string;
  readonly runId: string;
}

export interface CheckCacheReceipt {
  readonly identity: string;
  readonly fence: number;
  readonly owner: string;
  readonly runId: string;
  readonly result: BackendResult;
}

export type CheckCacheProbe =
  | { readonly kind: "miss"; readonly key: string }
  | { readonly kind: "busy"; readonly key: string; readonly expiresAt: number }
  | {
      readonly kind: "hit";
      readonly key: string;
      readonly artifact: CheckCacheArtifact;
    };

export type CheckCacheReservation =
  | CheckCacheProbe
  | { readonly kind: "lease"; readonly lease: CheckCacheLease };

export interface CheckCache {
  readonly probe: (job: CheckCacheJob, now: number) => Promise<CheckCacheProbe>;
  readonly acquire: (
    job: CheckCacheJob,
    options: {
      readonly owner: string;
      readonly runId: string;
      readonly now: number;
      readonly leaseMs: number;
    },
  ) => Promise<CheckCacheReservation>;
  readonly execute: <Input>(
    lease: CheckCacheLease,
    backend: VerificationBackend<Input>,
  ) => Promise<CheckCacheReceipt>;
  readonly publish: (
    lease: CheckCacheLease,
    receipt: CheckCacheReceipt,
    now: number,
  ) => Promise<"published" | "stale" | "uncacheable">;
  readonly renew: (
    lease: CheckCacheLease,
    now: number,
    leaseMs: number,
  ) => Promise<boolean>;
  readonly release: (lease: CheckCacheLease) => Promise<boolean>;
  readonly evict: (job: CheckCacheJob) => Promise<boolean>;
}

export interface CachedCheckSchedulingJob extends SchedulingJob {
  readonly check: CheckCacheJob;
}

export interface CachedCheckSchedule {
  readonly plan: SchedulePlan;
  readonly pending: readonly CachedCheckSchedulingJob[];
  readonly cached: readonly {
    readonly id: string;
    readonly result: {
      readonly backendResult: BackendResult;
      readonly origin: {
        readonly kind: "cache";
        readonly key: string;
        readonly producerRunId: string;
        readonly artifactDigest: string;
      };
    };
  }[];
  readonly deferred: readonly {
    readonly id: string;
    readonly key: string;
    readonly expiresAt: number;
  }[];
  readonly lookupMs: number;
}

export type CachedCheckExecution =
  | { readonly kind: "cached"; readonly artifact: CheckCacheArtifact }
  | {
      readonly kind: "deferred";
      readonly key: string;
      readonly expiresAt: number;
    }
  | {
      readonly kind: "executed";
      readonly result: BackendResult;
      readonly publication: "published" | "stale" | "uncacheable";
      readonly cleanupError?: unknown;
    };

export type SafetyScalar =
  | { readonly type: "int"; readonly value: string }
  | { readonly type: "bool"; readonly value: boolean };

interface SafetyFrameFor<Version extends 1 | 2> {
  readonly arguments: readonly SafetyExpressionFor<Version>[];
  readonly captures: readonly SafetyExpressionFor<Version>[];
  readonly parameterCount: number;
  readonly captureCount: number;
  readonly localScope: number;
}

type SafetyExpressionFor<Version extends 1 | 2> =
  | { readonly op: "data"; readonly value: PortableValue }
  | { readonly op: "bound"; readonly slot: number }
  | { readonly op: "old" }
  | { readonly op: "unmatched-case"; readonly location: string }
  | {
      readonly op: "set" | "tuple";
      readonly values: readonly SafetyExpressionFor<Version>[];
    }
  | {
      readonly op: "record" | "record-set";
      readonly fields: readonly (readonly [
        string,
        SafetyExpressionFor<Version>,
      ])[];
    }
  | {
      readonly op: "at";
      readonly target: SafetyExpressionFor<Version>;
      readonly key: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "update";
      readonly target: SafetyExpressionFor<Version>;
      readonly updates: readonly {
        readonly path: readonly SafetyExpressionFor<Version>[];
        readonly value: SafetyExpressionFor<Version>;
      }[];
    }
  | {
      readonly op: "forall" | "exists" | "function" | "filter" | "map-set";
      readonly bounds: readonly SafetyBinding<Version>[];
      readonly body: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "fold";
      readonly accumulator: number;
      readonly initial: SafetyExpressionFor<Version>;
      readonly collection: SafetyExpressionFor<Version>;
      readonly order: "set-members" | "sequence-left";
      readonly body: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "choose";
      readonly binding: SafetyBinding<Version>;
      readonly body: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "select-seq" | "sort-seq";
      readonly slot: number;
      readonly sequence: SafetyExpression;
      readonly body: SafetyExpression;
    }
  | {
      readonly op: "assert";
      readonly condition: SafetyExpression;
      readonly message: SafetyExpression;
    }
  | {
      readonly op:
        | "sequence-set"
        | "domain"
        | "variant-tag"
        | "cardinality"
        | "len"
        | "head"
        | "tail"
        | "powerset"
        | "union";
      readonly argument: SafetyExpressionFor<Version>;
    }
  | {
      readonly op:
        | "cup"
        | "cap"
        | "difference"
        | "subset-eq"
        | "append"
        | "concat"
        | "function-set"
        | "function-in"
        | "sequence-in";
      readonly left: SafetyExpressionFor<Version>;
      readonly right: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "subseq";
      readonly sequence: SafetyExpressionFor<Version>;
      readonly from: SafetyExpressionFor<Version>;
      readonly to: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "enabled";
      readonly branches: readonly SafetyBranch<Version>[];
      readonly subscript?:
        | SafetyExpressionFor<Version>
        | (Version extends 2 ? null : never);
    }
  | { readonly op: "literal"; readonly value: SafetyScalar }
  | { readonly op: "read"; readonly slot: number; readonly next: boolean }
  | {
      readonly op: "not" | "neg" | "prime";
      readonly argument: SafetyExpressionFor<Version>;
    }
  | {
      readonly op:
        | "add"
        | "sub"
        | "mul"
        | "div"
        | "mod"
        | "pow"
        | "eq"
        | "ne"
        | "lt"
        | "le"
        | "gt"
        | "ge"
        | "and"
        | "or"
        | "implies";
      readonly left: SafetyExpressionFor<Version>;
      readonly right: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "if";
      readonly condition: SafetyExpressionFor<Version>;
      readonly consequent: SafetyExpressionFor<Version>;
      readonly alternative: SafetyExpressionFor<Version>;
    }
  | {
      readonly op: "in";
      readonly value: SafetyExpressionFor<Version>;
      readonly domain: SafetyDomain<Version>;
    }
  | (Version extends 2
      ? | (SafetyFrameFor<Version> & {
            readonly op: "frame";
            readonly body: SafetyExpressionFor<Version>;
          })
        | {
            readonly op: "call";
            readonly definition: number;
            readonly arguments: readonly SafetyExpressionFor<Version>[];
            readonly captures: readonly SafetyExpressionFor<Version>[];
          }
        | { readonly op: "parameter" | "capture"; readonly slot: number }
      : never);

export type SafetyExpression = SafetyExpressionFor<1>;

export type SafetyExpressionV2 = SafetyExpressionFor<2>;

export type SafetyDomain<Version extends 1 | 2 = 1> =
  | { readonly kind: "value"; readonly value: SafetyExpressionFor<Version> }
  | {
      readonly kind: "range";
      readonly lower: SafetyExpressionFor<Version>;
      readonly upper: SafetyExpressionFor<Version>;
    }
  | {
      readonly kind: "set";
      readonly values: readonly SafetyExpressionFor<Version>[];
    };

export type SafetyStep<Version extends 1 | 2 = 1> =
  | {
      readonly kind: "bind";
      readonly slot: number;
      readonly domain: SafetyDomain<Version>;
      readonly tupleSize?: number;
    }
  | {
      readonly kind: "guard";
      readonly expression: SafetyExpressionFor<Version>;
    }
  | {
      readonly kind: "assign";
      readonly slot: number;
      readonly value: SafetyExpressionFor<Version>;
    }
  | {
      readonly kind: "choose";
      readonly slot: number;
      readonly domain: SafetyDomain<Version>;
    }
  | (Version extends 2
      ? | {
            readonly kind: "call";
            readonly definition: number;
            readonly arguments: readonly SafetyExpressionFor<Version>[];
            readonly captures: readonly SafetyExpressionFor<Version>[];
          }
        | (SafetyFrameFor<Version> & {
            readonly kind: "frame";
            readonly branches: readonly SafetyBranch<Version>[];
          })
      : never);

export interface SafetyBranch<Version extends 1 | 2 = 1> {
  readonly name: string;
  readonly steps: readonly SafetyStep<Version>[];
}

export interface SafetyBinding<Version extends 1 | 2 = 1> {
  readonly tupleSize?: number;
  readonly slot: number;
  readonly domain: SafetyDomain<Version>;
}

export type TemporalFormula<Version extends 1 | 2 = 1> =
  | { readonly kind: "atom"; readonly expression: SafetyExpressionFor<Version> }
  | {
      readonly kind: "forall" | "exists";
      readonly bounds: readonly SafetyBinding<Version>[];
      readonly formula: TemporalFormula<Version>;
    }
  | {
      readonly kind: "not" | "always" | "eventually";
      readonly formula: TemporalFormula<Version>;
    }
  | {
      readonly kind: "and" | "or";
      readonly left: TemporalFormula<Version>;
      readonly right: TemporalFormula<Version>;
    }
  | (Version extends 2
      ? | {
            readonly kind: "call";
            readonly definition: number;
            readonly arguments: readonly SafetyExpressionFor<Version>[];
            readonly captures: readonly SafetyExpressionFor<Version>[];
          }
        | (SafetyFrameFor<Version> & {
            readonly kind: "frame";
            readonly formula: TemporalFormula<Version>;
          })
      : never);

export interface TemporalProperty<Version extends 1 | 2 = 1> {
  readonly name: string;
  readonly formula: TemporalFormula<Version>;
}

export interface SafetyPredicate<Version extends 1 | 2 = 1> {
  readonly name: string;
  readonly expression: SafetyExpressionFor<Version>;
}

export interface SafetySymmetry {
  readonly modelValues: readonly string[];
}

export interface SafetyOptions {
  readonly workers?: number;
  readonly symmetryActions?: boolean;
  readonly partialOrder?: boolean;
}

interface SafetyProgramFields<Version extends 1 | 2> {
  readonly format: "modelt.safety-program";
  readonly version: Version;
  readonly name: string;
  readonly variables: readonly string[];
  readonly initial: readonly SafetyBranch<Version>[];
  readonly next: readonly SafetyBranch<Version>[];
  readonly invariants: readonly SafetyPredicate<Version>[];
  readonly assumptions: readonly SafetyPredicate<Version>[];
  readonly checkDeadlock: boolean;
  readonly symmetry?: SafetySymmetry;
  readonly initialProperties?: readonly SafetyPredicate<Version>[];
  readonly actionProperties?: readonly SafetyPredicate<Version>[];
  readonly constraints?: readonly SafetyPredicate<Version>[];
  readonly actionConstraints?: readonly SafetyPredicate<Version>[];
  readonly properties?: readonly TemporalProperty<Version>[];
  readonly fairness?: readonly TemporalFormula<Version>[];
}

interface SafetyDefinitionFields {
  readonly recursive?: boolean;
  readonly id: number;
  readonly parameterCount: number;
  readonly captureCount: number;
  readonly localScope: number;
}

export interface SafetyExpressionDefinition extends SafetyDefinitionFields {
  readonly bodyKind: "expression";
  readonly body: SafetyExpressionV2;
}

export interface SafetyActionDefinition extends SafetyDefinitionFields {
  readonly bodyKind: "action";
  readonly body: readonly SafetyBranch<2>[];
}

export interface SafetyTemporalDefinition extends SafetyDefinitionFields {
  readonly bodyKind: "temporal";
  readonly body: TemporalFormula<2>;
}

export type SafetyDefinition =
  | SafetyExpressionDefinition
  | SafetyActionDefinition
  | SafetyTemporalDefinition;

export type SafetyProgramV1 = SafetyProgramFields<1>;

export interface SafetyProgramV2 extends SafetyProgramFields<2> {
  readonly definitions: readonly SafetyDefinition[];
}

export type SafetyProgram = SafetyProgramV1 | SafetyProgramV2;

export interface SafetyBounds {
  readonly maxDepth: number;
  readonly maxStates: number;
  readonly maxWork: number;
  readonly maxTemporalNodes?: number;
}

export interface SafetyResult {
  readonly status:
    | "complete"
    | "bounded"
    | "empty"
    | "invariant-violation"
    | "property-violation"
    | "liveness-violation"
    | "deadlock"
    | "assumption-violation";
  readonly reason:
    | "depth"
    | "states"
    | "work"
    | "temporal"
    | "evaluation"
    | null;
  readonly states: number;
  readonly generated: number;
  readonly depth: number;
  readonly work: number;
  readonly violation: {
    readonly name: string;
    readonly trace: readonly {
      readonly action: string | null;
      readonly values: readonly PortableValue[];
    }[];
    readonly loopStart?: number;
    readonly loopAction?: string;
  } | null;
}

export interface SafetyActionCoverage {
  readonly name: string;
  readonly attempts: number;
  readonly enabled: number;
  readonly generated: number;
  readonly distinct: number;
  readonly work: number;
}

export interface SafetyCoverage {
  readonly initial: readonly SafetyActionCoverage[];
  readonly next: readonly SafetyActionCoverage[];
  readonly unattributedWork: number;
}

export interface SafetyCoverageReport {
  readonly result: SafetyResult;
  readonly coverage: SafetyCoverage;
}

export interface PortableEncoding<State, Action, Observation> {
  readonly state?: (state: State) => PortableValue;
  readonly action?: (action: Action) => PortableValue;
  readonly observation?: (observation: Observation) => PortableValue;
}

export interface FiniteModel {
  readonly format: "modelt.finite-model";
  readonly version: 1;
  readonly name: string;
  readonly initial: readonly {
    readonly key: string;
    readonly observation: PortableValue;
  }[];
  readonly states: readonly {
    readonly key: string;
    readonly violations: readonly string[];
    readonly transitions: readonly {
      readonly action: PortableValue;
      readonly target: string;
      readonly observation: PortableValue;
    }[];
  }[];
}

export type HistoryEvent =
  | {
      readonly kind: "invoke";
      readonly id: string;
      readonly action: PortableValue;
    }
  | {
      readonly kind: "return";
      readonly id: string;
      readonly observation: PortableValue;
    }
  | { readonly kind: "fail" | "unknown" | "cancel"; readonly id: string };

export interface ConcurrentHistory {
  readonly format: "modelt.concurrent-history";
  readonly version: 1;
  readonly model: string;
  readonly initial: PortableValue;
  readonly events: readonly HistoryEvent[];
}

export interface ConcurrentResult {
  readonly status:
    | "linearizable"
    | "not-linearizable"
    | "inconclusive"
    | "empty";
  readonly explored: number;
  readonly order: readonly string[];
  readonly omitted: readonly string[];
  readonly pending: readonly string[];
}

export interface HistoryRecorder {
  readonly invoke: (id: string, action: PortableValue) => void;
  readonly returned: (id: string, observation: PortableValue) => void;
  readonly end: (id: string, kind: "fail" | "unknown" | "cancel") => void;
  readonly snapshot: () => ConcurrentHistory;
}

export interface LoadedFiniteModel {
  readonly checkHistories: (
    traces: readonly PortableTrace[],
  ) => readonly PortableResult[];
  readonly checkHistory: (
    history: ConcurrentHistory,
    maxNodes: number,
  ) => ConcurrentResult;
  readonly checkConformance: (
    driver: Driver<PortableValue, PortableValue>,
    actions: readonly PortableValue[],
  ) => Promise<PortableResult>;
  readonly release: () => void;
}
export interface DeletionRange {
  readonly start: number;
  readonly count: number;
}
export interface ActionDependency {
  readonly producer: number;
  readonly consumer: number;
}
export type ShrinkVerdict =
  | { readonly status: "matched" | "invalid" }
  | { readonly status: "mismatch"; readonly signature: string };
export interface ShrinkResult<Action> {
  readonly status: "minimal" | "bounded" | "not-failing";
  readonly actions: readonly Action[];
  readonly checks: number;
}
export interface HostTape {
  readonly format: "modelt.host-tape";
  readonly version: 1;
  readonly events: readonly {
    readonly operation: string;
    readonly input: PortableValue;
    readonly output: PortableValue;
  }[];
}
export interface HostMessage {
  readonly id: number;
  readonly from: string;
  readonly to: string;
  readonly payload: PortableValue;
}
export interface SimulationHost {
  readonly now: () => number;
  readonly advance: (amount: number) => void;
  readonly choose: <Choice>(
    label: string,
    choices: readonly Choice[],
  ) => Choice;
  readonly send: (from: string, to: string, payload: unknown) => number;
  readonly deliver: (index: number) => HostMessage;
  readonly drop: (index: number) => void;
  readonly pending: () => readonly HostMessage[];
  readonly write: (key: string, value: unknown) => void;
  readonly read: (key: string) => unknown;
  readonly persist: () => void;
  readonly crash: () => void;
  readonly restart: () => void;
  readonly trace: () => HostTape;
  readonly finish: () => void;
}

export interface SchedulingJob {
  readonly id: string;
  readonly estimatedMs: number;
  readonly memoryBytes: number;
}
export interface SchedulingProfile {
  readonly id: string;
  readonly workers: number;
  readonly startupMs: number;
  readonly costScale: number;
  readonly fixedMemoryBytes: number;
}
export interface SchedulingBudget {
  readonly durationMs: number;
  readonly memoryBytes: number;
  readonly maxWorkers: number;
}
export interface SchedulingOptions {
  readonly policy: "fifo" | "cost";
  readonly batchTargetMs: number;
  readonly maxBatchSize: number;
}
export interface SchedulePlan {
  readonly status: "planned" | "empty" | "uncalibrated" | "insufficient-budget";
  readonly profile: string | null;
  readonly workers: number;
  readonly estimatedMs: number;
  readonly estimatedMemoryBytes: number;
  readonly startupMs: number;
  readonly durationMs: number;
  readonly jobs: readonly SchedulingJob[];
  readonly batches: readonly (readonly string[])[];
}
export interface ScheduledCompletion<Result> {
  readonly id: string;
  readonly result: Result;
}
export interface ScheduledExecutor<Result> {
  readonly execute: (
    ids: readonly string[],
  ) => Promise<readonly ScheduledCompletion<Result>[]>;
}
export interface ScheduledRun<Result> {
  readonly executionStatus: "complete" | "partial";
  readonly completed: readonly ScheduledCompletion<Result>[];
  readonly remaining: readonly string[];
  readonly elapsedMs: number;
  readonly dispatches: readonly {
    readonly worker: number;
    readonly ids: readonly string[];
    readonly queuedMs: number;
    readonly elapsedMs: number;
    readonly error: string | null;
  }[];
}
export interface AlloyInstance {
  readonly signatures: Readonly<Record<string, readonly string[]>>;
  readonly fields: Readonly<
    // Owner atom first; binary fields have pairs, n-ary fields longer tuples.
    Record<string, readonly (readonly string[])[]>
  >;
}
export interface AlloyInput {
  readonly source: string;
  readonly command: number;
  readonly maxWork: number;
}
export interface AlloyCommand {
  readonly label: string;
  readonly kind: "run" | "check";
  readonly scope: {
    readonly signatures: readonly {
      readonly name: string;
      readonly maximum: number;
      readonly exact: boolean;
    }[];
    readonly integerBitwidth: number;
  };
}
export interface AlloyStats {
  readonly work: number;
  readonly candidates: number;
  readonly factInstances: number;
}
export interface AlloyAnalysis {
  readonly command: AlloyCommand;
  readonly status:
    | "instance"
    | "counterexample"
    | "no-instance"
    | "no-counterexample"
    | "unknown";
  readonly instance: AlloyInstance | null;
  readonly stats: AlloyStats;
}
export interface AlloyInstanceCheck {
  readonly command: AlloyCommand;
  readonly status: "matched" | "mismatch" | "invalid" | "unknown";
  readonly reason:
    | "scope"
    | "declaration"
    | "facts"
    | "property"
    | "work"
    | null;
  readonly stats: AlloyStats;
}
export interface AlloyRuntime {
  readonly analyze: (input: AlloyInput) => AlloyAnalysis;
  readonly checkInstance: (
    input: AlloyInput,
    instance: AlloyInstance,
  ) => AlloyInstanceCheck;
}
export interface TestingJsonObject {
  readonly [name: string]: TestingJson;
}
export type TestingJson =
  | null
  | boolean
  | number
  | string
  | readonly TestingJson[]
  | TestingJsonObject;
export interface TestingScheduleActor {
  readonly id: string;
  readonly operations: readonly TestingJson[];
}
export interface TestingSchedule {
  readonly id: string;
  readonly version: string;
  readonly actors: readonly TestingScheduleActor[];
}
export interface TestingGenerator {
  readonly id: string;
  readonly version: string;
  readonly initial: readonly string[];
  readonly states: Readonly<
    Record<
      string,
      readonly { readonly action: TestingJson; readonly target: string }[]
    >
  >;
  readonly minLength: number;
  readonly maxLength: number;
}
export interface TestingCase {
  readonly format: "idealis.test-case";
  readonly version: 1;
  readonly rng: "xorshift32-v1";
  readonly generatorId: string;
  readonly generatorVersion: string;
  readonly seed: number;
  readonly index: number;
  readonly actions: readonly TestingJson[];
}
export interface TestingPlan {
  readonly generator: TestingGenerator;
  readonly seed: number;
  readonly cases: number;
  readonly maxAttempts: number;
  readonly maxShrinkChecks: number;
  readonly maxWork: number;
}
export type TestingVerdict =
  | { readonly status: "matched" | "invalid" }
  | { readonly status: "mismatch"; readonly signature: string }
  | { readonly status: "unknown"; readonly reason: string }
  | { readonly status: "error"; readonly message: string };
export type TestingPhase =
  | "generation"
  | "baseline"
  | "shrink"
  | "confirmation"
  | "replay";
export interface TestingTrial {
  readonly id: number;
  readonly phase: TestingPhase;
  readonly case: TestingCase;
  readonly verdict: TestingVerdict;
}
export interface TestingResult {
  readonly status:
    | "passed"
    | "failed"
    | "exhausted"
    | "inconclusive"
    | "error"
    | "not-failing";
  readonly reason: string | null;
  readonly seed: number;
  readonly attempts: number;
  readonly cases: number;
  readonly skips: number;
  readonly work: number;
  readonly shrink: {
    readonly status: "minimal" | "bounded";
    readonly checks: number;
  } | null;
  readonly counterexample: TestingCase | null;
  readonly signature: string | null;
  readonly trials: readonly TestingTrial[];
}
export type TestingEffect =
  | {
      readonly kind: "evaluate";
      readonly id: number;
      readonly phase: TestingPhase;
      readonly case: TestingCase;
    }
  | { readonly kind: "done"; readonly result: TestingResult };
export type TestingObservation =
  | { readonly kind: "allowed"; readonly values: readonly TestingJson[] }
  | { readonly kind: "value"; readonly value: TestingJson }
  | { readonly kind: "invalid" | "unknown"; readonly reason: string }
  | { readonly kind: "error"; readonly message: string };
export interface TestingTarget {
  readonly id: string;
  readonly outcome: TestingObservation;
}
export type TestingComparisonStatus =
  | "matched"
  | "mismatch"
  | "invalid"
  | "unknown"
  | "error";
export interface TestingComparison {
  readonly id: string;
  readonly status: TestingComparisonStatus;
  readonly expected: readonly TestingJson[];
  readonly actual: TestingJson | null;
  readonly reason: string | null;
  readonly outcome: TestingObservation;
}
export interface TestingDifferential {
  readonly status: TestingComparisonStatus;
  readonly oracle: TestingObservation;
  readonly comparisons: readonly TestingComparison[];
}
export type TestingRequest =
  | { readonly kind: "schedule"; readonly schedule: TestingSchedule }
  | { readonly kind: "property"; readonly plan: TestingPlan }
  | {
      readonly kind: "shrink";
      readonly generator: TestingGenerator;
      readonly case: TestingCase;
      readonly maxChecks: number;
      readonly maxWork: number;
    }
  | {
      readonly kind: "replay";
      readonly case: TestingCase;
      readonly expectedSignature: string | null;
    }
  | {
      readonly kind: "resume";
      readonly id: number;
      readonly verdict: TestingVerdict;
    }
  | { readonly kind: "cancel" }
  | {
      readonly kind: "compare";
      readonly oracle: TestingObservation;
      readonly targets: readonly TestingTarget[];
    };
export type TestingEvaluator = (
  testCase: TestingCase,
  phase: TestingPhase,
) => TestingVerdict | Promise<TestingVerdict>;
export interface TestingRuntime {
  readonly schedule: (schedule: TestingSchedule) => Promise<TestingGenerator>;
  readonly property: (
    plan: TestingPlan,
    evaluate: TestingEvaluator,
  ) => Promise<TestingResult>;
  readonly shrink: (
    generator: TestingGenerator,
    testCase: TestingCase,
    evaluate: TestingEvaluator,
    budget: { readonly maxChecks: number; readonly maxWork: number },
  ) => Promise<TestingResult>;
  readonly replay: (
    testCase: TestingCase,
    evaluate: TestingEvaluator,
    expectedSignature?: string,
  ) => Promise<TestingResult>;
  readonly compare: (
    oracle: TestingObservation,
    targets: readonly TestingTarget[],
  ) => Promise<TestingDifferential>;
}
