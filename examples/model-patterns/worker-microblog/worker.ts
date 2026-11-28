import { connectMicroblog, type MicroblogStore } from "./app.js";
import type { Command, Delivery } from "./idealis-generated.js";

// The model bounds IDs for exploration. The Worker accepts runtime IDs and the
// replay checker validates that a recorded trace is inside the model's scope.
type WithRuntimeIds<T> = {
  [Key in keyof T]: Key extends "p" | "reader" | "author" ? string : T[Key];
};

export interface Observation {
  readonly command: WithRuntimeIds<Command>;
  readonly message?: { id: string; attempts: number };
}

interface Statement {
  bind(...values: string[]): Statement;
  first<T>(column: string): Promise<T | null>;
  run(): Promise<{ meta: { changes: number } }>;
  all<T>(): Promise<{ results: T[] }>;
}

export interface Database {
  prepare(sql: string): Statement;
  batch(statements: Statement[]): Promise<unknown>;
}

interface Message {
  readonly id: string;
  readonly attempts: number;
  readonly body: string;
  ack(): void;
  retry(): void;
}

interface Env {
  readonly DB: Database;
  readonly POSTS: { send(post: string): Promise<unknown> };
  readonly OBSERVE?: { fetch(request: Request): Promise<Response> };
}

function d1Store(db: Database): MicroblogStore {
  return {
    async setFollowing(reader, author, following) {
      const sql = following
        ? "INSERT OR IGNORE INTO follows VALUES (?, ?)"
        : "DELETE FROM follows WHERE reader = ? AND author = ?";
      const result = await db.prepare(sql).bind(reader, author).run();
      return result.meta.changes > 0;
    },
    async publish(p, author) {
      const result = await db
        .prepare("INSERT OR IGNORE INTO posts VALUES (?, ?)")
        .bind(p, author)
        .run();
      return result.meta.changes > 0;
    },
    async author(p) {
      return db
        .prepare("SELECT author FROM posts WHERE post = ?")
        .bind(p)
        .first<string>("author");
    },
    async applyToTimeline(p, author) {
      // D1 commits both statements together. A retry can observe an already-applied post.
      await db.batch([
        db
          .prepare(`
          INSERT OR IGNORE INTO feed (reader, post)
          SELECT reader, ? FROM follows
          WHERE author = ? AND NOT EXISTS (SELECT 1 FROM applied WHERE post = ?)
        `)
          .bind(p, author, p),
        db.prepare("INSERT OR IGNORE INTO applied VALUES (?)").bind(p),
      ]);
    },
    async visibleAuthor(reader, p) {
      return db
        .prepare(`
        SELECT posts.author FROM feed JOIN posts USING (post)
        JOIN follows ON follows.reader = feed.reader AND follows.author = posts.author
        WHERE feed.reader = ? AND feed.post = ?
      `)
        .bind(reader, p)
        .first<string>("author");
    },
  };
}

async function observe(env: Env, observation: Observation): Promise<void> {
  if (env.OBSERVE === undefined) return;
  const response = await env.OBSERVE.fetch(
    new Request("https://observe.invalid/event", {
      method: "POST",
      body: JSON.stringify(observation),
    }),
  );
  if (!response.ok)
    throw new Error(`observation interrupted: ${response.status}`);
}

function connectWorker(env: Env) {
  return connectMicroblog(d1Store(env.DB), async (p) => {
    await observe(env, { command: { kind: "relay_offer", p } });
    await env.POSTS.send(p);
    await observe(env, { command: { kind: "queue_enqueue", p } });
  });
}

export default {
  async fetch(request: Request, env: Env): Promise<Response> {
    const [, operation, first, second] = new URL(request.url).pathname.split(
      "/",
    );
    const app = connectWorker(env);
    const changed = async (ok: boolean, command?: Observation["command"]) => {
      if (ok && command) await observe(env, { command });
      return Response.json({ ok }, { status: ok ? 200 : 409 });
    };

    if (
      operation === "follow" &&
      first &&
      second &&
      ["POST", "DELETE"].includes(request.method)
    ) {
      const following = request.method === "POST";
      return changed(await app.follow(first, second, following), {
        kind: following ? "users_follow" : "users_unfollow",
        reader: first,
        author: second,
      });
    }
    if (request.method === "POST" && first) {
      if (operation === "posts" && second)
        return changed(await app.publish(first, second), {
          kind: "posts_publish",
          p: first,
          author: second,
        });
      if (operation === "relay") return changed(await app.relay(first));
    }
    if (
      request.method === "GET" &&
      operation === "timeline" &&
      first &&
      second
    ) {
      const result = await app.read(first, second);
      if (result !== null) {
        await observe(env, {
          command: { kind: "timeline_read", reader: first, ...result },
        });
      }
      return Response.json(result, { status: result === null ? 404 : 200 });
    }
    return new Response(null, { status: 404 });
  },

  async queue(
    batch: { messages: readonly Message[] },
    env: Env,
  ): Promise<void> {
    const app = connectWorker(env);
    for (const message of batch.messages) {
      const p = message.body;
      const delivery: Delivery = message.attempts === 1 ? "Initial" : "Retry";
      const metadata = { id: message.id, attempts: message.attempts };
      try {
        await observe(env, {
          command: { kind: "queue_deliver", p, delivery },
          message: metadata,
        });
        const author = await app.applyPost(p);
        await observe(env, { command: { kind: "timeline_apply", p, author } });
        await observe(env, {
          command: { kind: "consumer_requestAck", p, delivery },
        });
      } catch {
        message.retry();
        await observe(env, {
          command: { kind: "queue_retry", p },
          message: metadata,
        });
        continue;
      }
      message.ack();
      await observe(env, {
        command: { kind: "queue_ack", p, delivery },
        message: metadata,
      });
    }
  },
};
