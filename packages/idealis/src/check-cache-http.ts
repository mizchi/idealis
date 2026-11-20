import { CheckCacheError } from "./check-cache-errors.js";
import {
  cacheCount,
  cacheFields,
  decodeCacheCell,
  validateCacheExchange,
} from "./check-cache-wire.js";
import type { CheckCacheStore } from "./contracts.js";

export const createHttpCheckCacheStore = (
  endpoint: string,
  fetcher: typeof fetch = globalThis.fetch,
): CheckCacheStore => {
  const base = new URL(endpoint);
  if (!["http:", "https:"].includes(base.protocol) || base.search || base.hash)
    throw new CheckCacheError("Invalid cache HTTP endpoint");
  const address = (key: string) => {
    if (!/^[0-9a-f]{64}$/.test(key))
      throw new CheckCacheError("Invalid cache key");
    return `${base.href.replace(/\/$/, "")}/${key}`;
  };
  return {
    read: async (key) => {
      const response = await fetcher(address(key));
      if (!response.ok)
        throw new CheckCacheError(`Cache HTTP read failed: ${response.status}`);
      const value = cacheFields(await response.json(), ["version", "cell"]);
      if (value.version !== 1)
        throw new CheckCacheError("Unsupported cache HTTP response");
      return value.cell === null ? null : decodeCacheCell(value.cell);
    },
    compareExchange: async (key, expected, input) => {
      if (expected !== null) cacheCount(expected, true);
      const next = decodeCacheCell(input);
      validateCacheExchange(null, expected, next);
      const response = await fetcher(address(key), {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({ version: 1, expectedRevision: expected, next }),
      });
      if (!response.ok)
        throw new CheckCacheError(
          `Cache HTTP exchange failed: ${response.status}`,
        );
      const value = cacheFields(await response.json(), [
        "version",
        "exchanged",
      ]);
      if (value.version !== 1 || typeof value.exchanged !== "boolean")
        throw new CheckCacheError("Invalid cache HTTP exchange response");
      return value.exchanged;
    },
  };
};
