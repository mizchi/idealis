# Idealis Studio

```sh
just studio
```

Open http://127.0.0.1:4317. The default workspace is the [Worker microblog](../../examples/model-patterns/worker-microblog/).
To open another workspace, pass an `.ideal` or `.project.json` path and optionally an Idealis spec JSON path.
Paths in spec claims are relative to the repository root. Without a spec argument, Studio uses `spec.json` next to the model when present.

```sh
just studio examples/model-patterns/autosave/autosave.ideal examples/model-patterns/autosave/spec.json
IDEALIS_STUDIO_PORT=4318 just studio
```

- **モデル関係図** opens by default. It uses ELK to arrange models in layers, route arrows, and reserve room for Japanese and English labels. It compares horizontal and vertical layouts with right-angle and diagonal routes. It prioritizes non-overlapping cards and labels, then scores bends, backtracking, crossings, wire length, and canvas extent. Large graphs keep readable dimensions at 100%. Use **全体表示** to see the full layout. At small scales, descriptions and edge labels reappear as you zoom in. A details panel opens when you select a model or connection. Arrows follow shared state from writing models to reading models. Arrow labels use linked vocabulary in the selected language, or the state name. Select a label to inspect the connection. Select a node or filter by state to inspect its connections. Nodes whose files changed since startup show a marker. Scroll the mouse wheel over the graph to zoom at the pointer. Drag the canvas background to pan. Click the zoom percentage to reset the scale and position.
- **モデルと仕様** follows links between natural-language rules and model checks in both directions. For composed projects, a component opens its read/write ports and the components on the other end.
- **差分プレビュー** compares saved source and model declarations against the startup snapshot, including untracked files. The snapshot lasts until the server restarts. A parser error leaves the raw diff available and suspends declaration comparison.
- **現在のモデルを検査** runs the selected check with a 200,000-state budget. When the checker returns a trace, you can inspect its commands. A saved change invalidates the previous result.

## Domain boundaries

Group the namespaces of a composed project in `model.project.json`:

```json
{
  "domains": [
    {
      "id": "delivery",
      "name": { "ja": "配信", "en": "Delivery" },
      "models": ["relay", "queue", "consumer"]
    }
  ]
}
```

Place `domains` beside `fragments`. Each model can belong to one domain. Models with ports that are left out remain visible outside the domain boxes. Domain diagrams use a left-to-right layout. Connections that cross a boundary are colored differently and show their source and destination domains in the inspector. Use **全体表示** to see the entire application at once.

## Model state

Cards show counts for internal, shared, and single-model project states. Select a card to see the names, type shapes, and each port's read or write access.

- **内部状態** is declared inside a model's namespace. Its name and owner are taken from the checked source.
- **共有状態** is a project variable bound by ports in two or more distinct models. Select a state in the inspector to highlight its connections and every model using it.
- **プロジェクト状態** includes project variables bound by only one model, such as the sample's Queue `pending`. It appears as **単独** on that model's card.

## Model descriptions

Add one `description` block to a model source or each composed `.ideal` fragment. For a reusable component, put it in the component body.

```ideal
description = {
  ja: "初回配信・再配信・ACK を管理する。",
  en: """Manages delivery and retries.
Acknowledges the active attempt.""",
}
```

The language switch in the graph previews Japanese or English. Select a card to read the full description. Saving the source refreshes the card and shows the edit in the diff preview.
The checker exports this metadata next to the checked model with `generate MODEL source-view`.

The server listens on loopback and reads files from the selected workspace. The preview displays source and declaration changes. For behavioral evidence, run the corresponding model check or the repository's implementation conformance tests.

Use `just studio-test` to test changes in a temporary workspace and in Chromium. Install the workspace Chromium with `pnpm exec playwright install chromium` if needed.
