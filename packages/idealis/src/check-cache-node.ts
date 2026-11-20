import type { IncomingMessage, ServerResponse } from "node:http";
import { DatabaseSync } from "node:sqlite";
import { CheckCacheError } from "./check-cache-errors.js";
import {
  cacheCount,
  cacheFields,
  decodeCacheCell,
  validateCacheExchange,
} from "./check-cache-wire.js";
import type { CheckCacheStore } from "./contracts.js";
import { isRecord, valueKey } from "./trace.js";

export const createSqliteCheckCacheStore = (
  path: string,
): CheckCacheStore & { readonly close: () => void } => {
  const database = new DatabaseSync(path);
  database.exec(
    "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; CREATE TABLE IF NOT EXISTS idealis_check_cache (key TEXT PRIMARY KEY, revision INTEGER NOT NULL CHECK(revision > 0), body TEXT NOT NULL) STRICT;",
  );
  const get = database.prepare(
    "SELECT revision, body FROM idealis_check_cache WHERE key = ?",
  );
  const insert = database.prepare(
    "INSERT INTO idealis_check_cache(key, revision, body) VALUES (?, ?, ?) ON CONFLICT(key) DO NOTHING",
  );
  const update = database.prepare(
    "UPDATE idealis_check_cache SET revision = ?, body = ? WHERE key = ? AND revision = ?",
  );
  const read = (key: string) => {
    const row = get.get(key);
    if (row === undefined) return null;
    if (!isRecord(row) || typeof row.body !== "string")
      throw new CheckCacheError("Corrupt SQLite cache row");
    let raw: unknown;
    try {
      raw = JSON.parse(row.body);
    } catch {
      throw new CheckCacheError("Corrupt SQLite cache JSON");
    }
    const cell = decodeCacheCell(raw);
    if (cell.revision !== row.revision)
      throw new CheckCacheError("SQLite cache revision mismatch");
    return cell;
  };
  return {
    read: async (key) => read(key),
    compareExchange: async (key, expected, input) => {
      const previous = read(key);
      if ((previous?.revision ?? null) !== expected) return false;
      const next = decodeCacheCell(input);
      validateCacheExchange(previous, expected, next);
      const result =
        expected === null
          ? insert.run(key, next.revision, valueKey(next))
          : update.run(next.revision, valueKey(next), key, expected);
      return Number(result.changes) === 1;
    },
    close: () => database.close(),
  };
};

export const createCheckCacheHttpHandler = (
  store: CheckCacheStore,
  options: { readonly maxRequestBytes?: number } = {},
): ((request: IncomingMessage, response: ServerResponse) => Promise<void>) => {
  const maximum = cacheCount(options.maxRequestBytes ?? 16 * 1024 * 1024, true);
  return async (request, response) => {
    const reply = (status: number, body: unknown) => {
      response.writeHead(status, {
        "content-type": "application/json",
        "access-control-allow-origin": "*",
        "access-control-allow-methods": "GET, POST, OPTIONS",
        "access-control-allow-headers": "content-type",
      });
      response.end(JSON.stringify(body));
    };
    try {
      if (request.method === "OPTIONS") {
        reply(204, null);
        return;
      }
      const path = new URL(request.url ?? "", "http://cache.local");
      if (!/^\/[0-9a-f]{64}$/.test(path.pathname) || path.search) {
        reply(404, { version: 1, error: "Unknown cache key" });
        return;
      }
      const key = path.pathname.slice(1);
      if (request.method === "GET") {
        reply(200, { version: 1, cell: await store.read(key) });
        return;
      }
      if (request.method !== "POST") {
        reply(405, { version: 1, error: "Unsupported cache method" });
        return;
      }
      const chunks: Buffer[] = [];
      let length = 0;
      for await (const chunk of request) {
        const data = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
        length += data.length;
        if (length > maximum) {
          reply(413, {
            version: 1,
            error: "Cache request exceeds transport limit",
          });
          return;
        }
        chunks.push(data);
      }
      let raw: unknown;
      try {
        raw = JSON.parse(Buffer.concat(chunks).toString("utf8"));
      } catch {
        throw new CheckCacheError("Invalid cache HTTP JSON");
      }
      const exchange = cacheFields(raw, [
        "version",
        "expectedRevision",
        "next",
      ]);
      if (exchange.version !== 1)
        throw new CheckCacheError("Unsupported cache HTTP version");
      const expected =
        exchange.expectedRevision === null
          ? null
          : cacheCount(exchange.expectedRevision, true);
      reply(200, {
        version: 1,
        exchanged: await store.compareExchange(
          key,
          expected,
          decodeCacheCell(exchange.next),
        ),
      });
    } catch (error) {
      reply(
        error instanceof CheckCacheError || error instanceof SyntaxError
          ? 400
          : 500,
        {
          version: 1,
          error: error instanceof Error ? error.name : "CacheError",
        },
      );
    }
  };
};
