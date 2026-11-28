import type { ObservationAdapter, ObservationProjection } from "../replay.js";

export type IoCallback = "Microtask" | "Immediate" | "Timer";
export type NextTickCallback =
  | "IoNextTick"
  | "FirstMicrotask"
  | "SecondMicrotask"
  | "NestedNextTick";
export type DrainIoBatch = { readonly kind: "drainIoBatch" };

export interface IoBatch<Callback extends string = IoCallback> {
  drain(): Promise<void>;
  observe(): readonly Callback[];
}

export const ioBatchProjection: ObservationProjection = (state) => state.calls;

export const nextTickProjection: ObservationProjection = (state) => {
  const positions = ["First", "Second", "Third", "Fourth"];
  const callbacks = state.callbackAt as Readonly<
    Record<string, NextTickCallback | null>
  >;
  return {
    $sequence: positions
      .map((position) => callbacks[position])
      .filter((callback) => callback != null),
  };
};

export function ioBatchAdapter<Callback extends string>(
  batch: IoBatch<Callback>,
  project = ioBatchProjection,
): ObservationAdapter<DrainIoBatch> {
  return {
    apply: () => batch.drain(),
    async observe() {
      return { $sequence: [...batch.observe()] };
    },
    project,
  };
}
