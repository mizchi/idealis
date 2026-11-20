import assert from "node:assert/strict";
import type { Profiler } from "node:inspector";

interface WeightedStack {
  readonly frames: readonly string[];
  readonly weight: number;
}
interface Hotspot {
  readonly name: string;
  self: number;
  inclusive: number;
}
interface Symbolication {
  readonly string_table: readonly string[];
  readonly data: readonly {
    readonly code_id: string;
    readonly known_addresses: readonly (readonly [number, number | null])[];
    readonly symbol_table: readonly { readonly symbol: number }[];
  }[];
}
interface NativeProfile {
  readonly libs: readonly { readonly codeId: string }[];
  readonly threads: readonly {
    readonly samples: {
      readonly stack: readonly (number | null)[];
      readonly weight: readonly number[];
      readonly threadCPUDelta: readonly (number | null)[];
    };
    readonly stackTable: {
      readonly frame: readonly number[];
      readonly prefix: readonly (number | null)[];
    };
    readonly frameTable: {
      readonly func: readonly number[];
      readonly address: readonly number[];
    };
    readonly funcTable: {
      readonly name: readonly number[];
      readonly resource: readonly number[];
    };
    readonly resourceTable: { readonly lib: readonly number[] };
    readonly stringArray: readonly string[];
  }[];
}
export const summarizeStacks = (stacks: readonly WeightedStack[]) => {
  const hotspots = new Map<string, Hotspot>();
  let totalWeight = 0;
  for (const stack of stacks) {
    assert.ok(
      stack.frames.length > 0 &&
        Number.isFinite(stack.weight) &&
        stack.weight >= 0,
    );
    totalWeight += stack.weight;
    for (const name of new Set(stack.frames)) {
      const row = hotspots.get(name) ?? { name, self: 0, inclusive: 0 };
      row.inclusive += stack.weight;
      hotspots.set(name, row);
    }
    const name = stack.frames.at(-1);
    assert.ok(name !== undefined);
    const row = hotspots.get(name);
    assert.ok(row);
    row.self += stack.weight;
  }
  assert.ok(totalWeight > 0, "Profile has no positive sampling weight");
  return {
    totalWeight,
    hotspots: [...hotspots.values()].sort(
      (left, right) =>
        right.self - left.self || left.name.localeCompare(right.name),
    ),
  };
};
export const summarizeCpu = (profile: Profiler.Profile) => {
  assert.ok(profile.samples?.length);
  const nodes = new Map(profile.nodes.map((node) => [node.id, node]));
  const parents = new Map(
    profile.nodes.flatMap((node) =>
      (node.children ?? []).map((child) => [child, node.id] as const),
    ),
  );
  return summarizeStacks(
    profile.samples.map((id) => {
      const frames: string[] = [];
      const visited = new Set<number>();
      for (
        let cursor: number | undefined = id;
        cursor !== undefined;
        cursor = parents.get(cursor)
      ) {
        assert.ok(!visited.has(cursor), "Profile contains a parent cycle");
        visited.add(cursor);
        const node = nodes.get(cursor);
        assert.ok(node, "Unknown profile node");
        frames.push(node.callFrame.functionName);
      }
      return { frames: frames.reverse(), weight: 1 };
    }),
  );
};
export const summarizeNative = (
  profile: NativeProfile,
  symbols: Symbolication,
  mode: "samples" | "cpu",
) => {
  const addresses = profile.libs.map((library) => {
    const data = symbols.data.find((entry) => entry.code_id === library.codeId);
    return new Map(
      data?.known_addresses.flatMap(([address, index]) => {
        if (index === null) return [];
        const entry = data.symbol_table[index];
        assert.ok(entry);
        const name = symbols.string_table[entry.symbol];
        assert.ok(name !== undefined);
        return [[address, name] as const];
      }),
    );
  });
  const stacks: WeightedStack[] = [];
  for (const thread of profile.threads) {
    for (const [index, stack] of thread.samples.stack.entries()) {
      if (stack === null) continue;
      const frames: string[] = [];
      const visited = new Set<number>();
      for (
        let cursor: number | null | undefined = stack;
        cursor !== null && cursor !== undefined;
        cursor = thread.stackTable.prefix[cursor]
      ) {
        assert.ok(!visited.has(cursor), "Profile contains a stack cycle");
        visited.add(cursor);
        const frame = thread.stackTable.frame[cursor];
        assert.ok(frame !== undefined);
        const func = thread.frameTable.func[frame];
        assert.ok(func !== undefined);
        const resource = thread.funcTable.resource[func];
        const library =
          resource === undefined
            ? undefined
            : thread.resourceTable.lib[resource];
        const address = thread.frameTable.address[frame];
        const fallback = thread.funcTable.name[func];
        assert.ok(fallback !== undefined);
        const name =
          (library !== undefined && address !== undefined
            ? addresses[library]?.get(address)
            : undefined) ?? thread.stringArray[fallback];
        assert.ok(name !== undefined);
        frames.push(name);
      }
      const weight =
        mode === "cpu"
          ? (thread.samples.threadCPUDelta[index] ?? 0)
          : thread.samples.weight[index];
      assert.ok(weight !== undefined);
      stacks.push({ frames: frames.reverse(), weight });
    }
  }
  return summarizeStacks(stacks);
};
