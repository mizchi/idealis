import { readFile, access } from "node:fs/promises";
import { createServer } from "node:http";
import { dirname, join, resolve } from "node:path";
import { createWorkspace } from "./workspace.mjs";

const example = "examples/model-patterns/worker-microblog";
const root = process.cwd();
const model = process.argv[2] || join(example, "model.project.json");
const spec =
  process.argv[3] ||
  (process.argv[2]
    ? join(dirname(model), "spec.json")
    : join(example, "spec.json"));
const checker = resolve(
  process.env.IDEALIS_CHECKER ??
    "crates/idealis-check/target/debug/idealis-check",
);
const port = Number(process.env.IDEALIS_STUDIO_PORT ?? 4317);

try {
  await access(checker);
} catch {
  throw new Error(
    "idealis-check が見つかりません。just studio でビルドして起動してください。",
  );
}
const optionalSpec =
  process.argv[3] ||
  (await access(resolve(root, spec)).then(
    () => spec,
    () => undefined,
  ));
const workspace = await createWorkspace({
  root,
  model,
  spec: optionalSpec,
  checker,
});
const assets = new Map(
  await Promise.all(
    [
      ["/", "index.html", "text/html"],
      ["/studio.js", "studio.js", "text/javascript"],
      ["/topology.js", "topology.js", "text/javascript"],
      ["/studio.css", "studio.css", "text/css"],
    ].map(async ([route, file, contentType]) => [
      route,
      { contentType, body: await readFile(new URL(file, import.meta.url)) },
    ]),
  ),
);

let checking = false;
const server = createServer(async (request, response) => {
  response.setHeader("Cache-Control", "no-store");
  response.setHeader("X-Content-Type-Options", "nosniff");
  response.setHeader(
    "Content-Security-Policy",
    "default-src 'self'; base-uri 'none'; frame-ancestors 'none'",
  );
  const allowed = new Set([
    `127.0.0.1:${server.address().port}`,
    `localhost:${server.address().port}`,
  ]);
  if (!allowed.has(request.headers.host)) {
    response.writeHead(403).end();
    return;
  }
  if (request.method !== "GET") {
    response.writeHead(405, { Allow: "GET" }).end();
    return;
  }
  if (!request.url?.startsWith("/") || request.url.startsWith("//")) {
    response.writeHead(400).end();
    return;
  }
  const url = new URL("http://127.0.0.1" + request.url);
  const asset = assets.get(url.pathname);
  if (asset) {
    response
      .writeHead(200, { "Content-Type": `${asset.contentType}; charset=utf-8` })
      .end(asset.body);
    return;
  }
  const json = (status, value) =>
    response
      .writeHead(status, { "Content-Type": "application/json; charset=utf-8" })
      .end(JSON.stringify(value));
  try {
    if (url.pathname === "/api/workspace") {
      const workspaceView = await workspace.refresh();
      if (url.searchParams.get("since") === workspaceView.revision)
        response.writeHead(204).end();
      else json(200, workspaceView);
    } else if (url.pathname === "/api/witness") {
      if (checking) {
        json(409, { error: "別の検査を実行中です" });
        return;
      }
      checking = true;
      try {
        json(
          200,
          await workspace.witness(
            url.searchParams.get("check"),
            url.searchParams.get("revision"),
          ),
        );
      } finally {
        checking = false;
      }
    } else {
      json(404, { error: "見つかりません" });
    }
  } catch (error) {
    json(422, { error: error.stderr?.trim() || error.message });
  }
});

server.listen(port, "127.0.0.1", () => {
  console.log(`Idealis Studio  http://127.0.0.1:${server.address().port}`);
});
