import type { ObservationAdapter, ObservationProjection } from "../replay.js";

export type ObjectCommand =
  | { readonly kind: "put"; readonly body: string }
  | { readonly kind: "remove" };

export interface ObjectStore {
  put(key: string, body: string): Promise<void>;
  remove(key: string): Promise<void>;
  get(key: string): Promise<string | null>;
  list(prefix: string): Promise<readonly string[]>;
}

export const objectStoreProjection: ObservationProjection = (state) => ({
  body: state.object,
  listed: state.object !== null,
});

/** Drive one key and observe successful operations through independent reads. */
export function objectStoreAdapter(
  store: ObjectStore,
  key: string,
): ObservationAdapter<ObjectCommand> {
  return {
    async apply(command) {
      if (command.kind === "put") await store.put(key, command.body);
      else await store.remove(key);
    },
    async observe() {
      const [body, keys] = await Promise.all([store.get(key), store.list(key)]);
      return { body, listed: keys.includes(key) };
    },
    project: objectStoreProjection,
  };
}
