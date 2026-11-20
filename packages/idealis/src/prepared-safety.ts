import type {
  PortableCore,
  PreparedSafetyProgram,
  SafetyProgram,
} from "./contracts.js";
import { PortableCoreError } from "./portable-value.js";

export const prepareSafetyProgram = (
  core: PortableCore,
  program: SafetyProgram,
): PreparedSafetyProgram => {
  const prepared = core.request({
    version: 1,
    request: { kind: "prepare-safety", program },
  });
  if (prepared.kind !== "safety-prepared")
    throw new PortableCoreError("protocol", "Expected safety handle");
  let released = false;
  const active = () => {
    if (released)
      throw new PortableCoreError("protocol", "Released safety handle");
  };
  return {
    check: (bounds, options) => {
      active();
      const output = core.request({
        version: 1,
        request: {
          kind: "check-prepared-safety",
          handle: prepared.handle,
          bounds,
          ...(options === undefined ? {} : { options }),
        },
      });
      if (output.kind !== "safety")
        throw new PortableCoreError("protocol", "Expected safety result");
      return output.result;
    },
    checkBatch: (jobs) => {
      active();
      const output = core.request({
        version: 1,
        request: {
          kind: "check-prepared-safety-batch",
          handle: prepared.handle,
          jobs,
        },
      });
      if (
        output.kind !== "safety-batch" ||
        output.results.length !== jobs.length
      )
        throw new PortableCoreError(
          "protocol",
          "Expected complete safety batch results",
        );
      return output.results;
    },
    release: () => {
      if (released) return;
      const output = core.request({
        version: 1,
        request: { kind: "release-safety", handle: prepared.handle },
      });
      if (output.kind !== "released")
        throw new PortableCoreError(
          "protocol",
          "Expected released safety handle",
        );
      released = true;
    },
  };
};
