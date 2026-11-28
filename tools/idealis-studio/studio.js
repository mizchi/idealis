import { edgeCaption, stateNames, wireLayer } from "./topology.js";

const $ = (selector) => document.querySelector(selector);
const labels = {
  invariant: "不変条件",
  transition: "遷移条件",
  reachable: "到達可能性",
  temporal: "時相性",
  action: "振る舞い",
  check: "検査",
  state: "状態",
  rule: "仕様ルール",
  term: "用語",
  type: "型",
  component: "構成要素",
  domain: "ドメイン境界",
  model: "モデル",
  added: "追加",
  removed: "削除",
  modified: "変更",
  holds: "成立",
  violated: "違反",
  reachableVerdict: "到達",
  unreachable: "到達不可",
  unknown: "未確定",
  empty: "空の探索",
  invalid: "無効",
};
const nameOf = (label) => labels[label] ?? label;
const node = (tag, className, ...children) => {
  const element = document.createElement(tag);
  if (className) element.className = className;
  element.append(
    ...children.filter((child) => child !== null && child !== undefined),
  );
  return element;
};
const text = (tag, value, className) =>
  node(tag, className, String(value ?? ""));
const chip = (label, tone) => text("span", label, "chip " + (tone ?? ""));
const code = (value) => text("code", value);
const pretty = (value) => text("pre", JSON.stringify(value, null, 2));
const leaf = (path) => path.split("/").at(-1);
let model;
let view = "graph";
let selectedModel;
let selectedConnection;
let sharedState = "";
let descriptionLanguage = "ja";
let graphZoom = 1;
let selection;
let notice;
let result;
let busy = false;

function button(label, fn, className = "", pressed) {
  const value = text("button", label, className);
  value.type = "button";
  value.addEventListener("click", fn);
  if (pressed !== undefined)
    value.setAttribute("aria-pressed", String(pressed));
  return value;
}
function go(page, kind, id) {
  view = page;
  selection = kind ? { kind, id } : undefined;
  result = undefined;
  render();
}
const select = (kind, id) => go("relations", kind, id);
const chosen = (kind, id) => selection?.kind === kind && selection.id === id;

function list(title, entries) {
  if (!entries.length) return null;
  return node("section", "nav-group", text("h2", title), ...entries);
}
function renderNavigation() {
  $("#project").textContent = model.current.name;
  $("#model-path").textContent = model.model;
  const count = model.preview.files.length;
  $("#tabs").replaceChildren(
    button("モデル関係図", () => go("graph"), "tab", view === "graph"),
    button("モデルと仕様", () => go("relations"), "tab", view === "relations"),
    button(
      count ? `差分プレビュー (${count})` : "差分プレビュー",
      () => go("preview"),
      "tab",
      view === "preview",
    ),
  );
  const { rules, checks, components, states, actions, files } = model.current;
  const entry = (label, active, fn, detail) => {
    const item = button(label, fn, "nav-entry", active);
    if (detail) item.append(text("small", detail));
    return item;
  };
  $("#explorer").replaceChildren(
    ...[
      list(
        "仕様ルール",
        rules.map(({ id, text: title }) =>
          entry(title.ja, chosen("rule", id), () => select("rule", id)),
        ),
      ),
      list(
        "モデル検査",
        checks.map(({ name, kind }) =>
          entry(
            name,
            chosen("check", name),
            () => select("check", name),
            nameOf(kind),
          ),
        ),
      ),
      list(
        "構成要素",
        components.map(({ name }) =>
          entry(name, chosen("component", name), () =>
            select("component", name),
          ),
        ),
      ),
      list(
        "状態",
        states.map(({ name }) =>
          entry(name, chosen("state", name), () => select("state", name)),
        ),
      ),
      list(
        "振る舞い",
        actions.map(({ name }) =>
          entry(name, chosen("action", name), () => select("action", name)),
        ),
      ),
      list(
        "ソース",
        Object.keys(files).map((file) =>
          entry(leaf(file), chosen("file", file), () => select("file", file)),
        ),
      ),
    ].filter(Boolean),
  );
  const status = $("#status");
  const invalid = model.current.diagnostics.length;
  status.textContent =
    notice ||
    (invalid
      ? "入力エラー"
      : count
        ? `保存済みの変更 ${count} 件`
        : "差分なし");
  status.dataset.tone =
    notice || invalid ? "error" : count ? "changed" : "clean";
}

function heading(label, title, subtitle) {
  return node(
    "header",
    "page-heading",
    text("div", label, "eyebrow"),
    text("h1", title),
    subtitle && text("p", subtitle),
  );
}
function card(title, ...children) {
  return node("section", "card", text("h2", title), ...children);
}
const linkCheck = (check) =>
  button(check, () => select("check", check), "inline-link mono");
const linkRule = (rule) =>
  button(rule, () => select("rule", rule), "inline-link mono");
const openSource = (file) =>
  button(leaf(file), () => select("file", file), "inline-link mono");

function descriptionText(
  descriptions,
  tag = "p",
  className = "model-description",
) {
  if (!descriptions) return null;
  const language = descriptions[descriptionLanguage]
    ? descriptionLanguage
    : descriptionLanguage === "ja"
      ? "en"
      : "ja";
  if (!descriptions[language]) return null;
  const paragraph = text(tag, descriptions[language].trim(), className);
  paragraph.lang = language;
  return paragraph;
}

function descriptionLanguageSelect() {
  const select = node(
    "select",
    "",
    ...[
      ["ja", "日本語"],
      ["en", "English"],
    ].map(([code, label]) => {
      const option = text("option", label);
      option.value = code;
      option.lang = code;
      return option;
    }),
  );
  select.id = "description-language";
  select.value = descriptionLanguage;
  select.addEventListener("change", () => {
    descriptionLanguage = select.value;
    render();
    $("#description-language")?.focus();
  });
  return node("label", "graph-filter", text("span", "説明の言語"), select);
}

function graphCenter(viewport) {
  return {
    x: (viewport.scrollWidth - viewport.clientWidth) / 2,
    y: (viewport.scrollHeight - viewport.clientHeight) / 2,
  };
}

function enableGraphDrag(viewport) {
  let drag;
  viewport.addEventListener("pointerdown", (event) => {
    if (
      event.pointerType !== "mouse" ||
      event.button !== 0 ||
      event.target.closest("button")
    )
      return;
    const frame = viewport.getBoundingClientRect();
    if (
      event.clientX - frame.left >= viewport.clientWidth ||
      event.clientY - frame.top >= viewport.clientHeight
    )
      return;

    event.preventDefault();
    drag = {
      id: event.pointerId,
      x: event.clientX,
      y: event.clientY,
      left: viewport.scrollLeft,
      top: viewport.scrollTop,
    };
    viewport.setPointerCapture(event.pointerId);
    viewport.classList.add("graph-dragging");
  });
  viewport.addEventListener("pointermove", (event) => {
    if (drag?.id !== event.pointerId) return;
    viewport.scrollTo(
      drag.left + drag.x - event.clientX,
      drag.top + drag.y - event.clientY,
    );
  });
  viewport.addEventListener("lostpointercapture", () => {
    drag = undefined;
    viewport.classList.remove("graph-dragging");
  });
}

function zoomableGraph(stage, layout) {
  const viewport = node(
    "div",
    "graph-scroll",
    node("div", "graph-world", stage),
  );
  enableGraphDrag(viewport);
  const resetCamera = (scale) => {
    setZoom(scale);
    const center = graphCenter(viewport);
    viewport.scrollTo(center.x, center.y);
  };
  const reset = button("", () => resetCamera(1), "graph-zoom");
  const fit = button("全体表示", () => resetCamera(fitScale()), "graph-zoom");
  const aspectRatio = layout.width / layout.height;

  function fitScale() {
    const available = Math.min(
      viewport.clientWidth,
      viewport.clientHeight * aspectRatio,
    );
    const natural = Math.max(layout.width * 0.88, available);
    return Math.min(1, available / natural);
  }
  reset.setAttribute("aria-label", "ズームをリセット");
  reset.title = "ズームをリセット";

  function setZoom(scale) {
    graphZoom = scale;
    const minimum = layout.width * 0.88 * scale;
    stage.dataset.compact = String(
      scale < (layout.domains.length ? 0.7 : 0.88),
    );
    stage.dataset.small = String(scale < 0.88);
    stage.dataset.thumbnail = String(scale < 0.35);
    const widthAtHeight = 100 * aspectRatio * scale;
    stage.style.width = `max(${minimum}px, min(${100 * scale}cqw, ${widthAtHeight}cqh))`;
    reset.textContent = Math.round(scale * 100) + "%";
  }

  setZoom(graphZoom);
  viewport.addEventListener(
    "wheel",
    (event) => {
      if (!event.deltaY) return;
      event.preventDefault();
      const pixels =
        event.deltaY *
        (event.deltaMode === 1
          ? 16
          : event.deltaMode === 2
            ? viewport.clientHeight
            : 1);
      const zoom = Math.min(
        2.5,
        Math.max(
          Math.min(0.55, fitScale()),
          graphZoom * Math.exp(-pixels * 0.0015),
        ),
      );
      if (zoom === graphZoom) return;

      const before = stage.getBoundingClientRect();
      const x = (event.clientX - before.left) / before.width;
      const y = (event.clientY - before.top) / before.height;
      setZoom(zoom);
      const after = stage.getBoundingClientRect();
      viewport.scrollTo(
        viewport.scrollLeft + after.left + x * after.width - event.clientX,
        viewport.scrollTop + after.top + y * after.height - event.clientY,
      );
    },
    { passive: false },
  );
  return { viewport, reset, fit };
}

const connectionId = ({ from, to }) => from + " → " + to;

const stateKinds = [
  { id: "internal", label: "内部状態", short: "内部" },
  { id: "shared", label: "共有状態", short: "共有" },
  { id: "project", label: "プロジェクト状態", short: "単独" },
];

function statesByModel(catalog) {
  const models = new Map();
  for (const state of catalog) {
    const participants = state.owner
      ? [state.owner]
      : new Set(state.ports.map(({ model }) => model));
    for (const name of participants) {
      if (!models.has(name)) models.set(name, []);
      models.get(name).push(state);
    }
  }
  return models;
}

function stateSummary(states) {
  if (!states.length) return null;
  return node(
    "span",
    "graph-state-summary",
    ...stateKinds.map(({ id, short }) => {
      const count = states.filter(({ kind }) => kind === id).length;
      return count
        ? text("span", short + " " + count, "graph-state-count state-" + id)
        : null;
    }),
  );
}

function stateSections(states, focusState, highlightShared) {
  return node(
    "div",
    "graph-state-sections",
    ...stateKinds.map(({ id, label }) => {
      const entries = states.filter(({ kind }) => kind === id);
      if (!entries.length) return null;
      return node(
        "section",
        "graph-state-group",
        text("h3", label),
        node(
          "ul",
          "graph-state-list",
          ...entries.map((state) =>
            node(
              "li",
              "",
              node(
                "div",
                "graph-state-entry",
                id === "shared"
                  ? button(
                      state.localName,
                      () => highlightShared(state.name),
                      "inline-link mono",
                      focusState === state.name,
                    )
                  : button(
                      state.localName,
                      () => select("state", state.name),
                      "inline-link mono",
                    ),
                text("span", state.shape, "graph-state-shape"),
              ),
              state.ports.length
                ? node(
                    "div",
                    "graph-state-ports",
                    ...state.ports.map(({ model, port, access }) =>
                      text(
                        "span",
                        (access === "write" ? "書く " : "読む ") +
                          model +
                          (port === state.name ? "" : "." + port),
                        access === "write"
                          ? "state-port-write"
                          : "state-port-read",
                      ),
                    ),
                  )
                : null,
            ),
          ),
        ),
      );
    }),
  );
}

function centerGraphParticipants(ports) {
  if (!ports.length) return;
  const participants = new Set(ports.map(({ model }) => model));
  const viewport = $(".graph-scroll");
  const boxes = [...viewport.querySelectorAll(".graph-model")]
    .filter(({ dataset }) => participants.has(dataset.model))
    .map((item) => item.getBoundingClientRect());
  if (!boxes.length) return;
  const left = Math.min(...boxes.map((box) => box.left));
  const right = Math.max(...boxes.map((box) => box.right));
  const top = Math.min(...boxes.map((box) => box.top));
  const bottom = Math.max(...boxes.map((box) => box.bottom));
  const frame = viewport.getBoundingClientRect();
  viewport.scrollBy(
    (left + right) / 2 - (frame.left + viewport.clientWidth / 2),
    (top + bottom) / 2 - (frame.top + viewport.clientHeight / 2),
  );
}

function graphPage() {
  const layout = model.current.graph;
  const edges = layout?.edges ?? [];
  const components = model.current.components;
  const addresses = new Map(
    components.map((component) => [component.name, component]),
  );
  const catalog = model.current.stateCatalog;
  const modelStates = statesByModel(catalog);
  const shared = new Map(
    catalog
      .filter(({ kind }) => kind === "shared")
      .map((entry) => [entry.name, entry]),
  );
  const states = [...shared.keys()].sort();
  const current = addresses.get(selectedModel);
  const connection = edges.find(
    (edge) => connectionId(edge) === selectedConnection,
  );
  const state = states.includes(sharedState) ? sharedState : "";
  const filtered = Boolean(current || connection || state);
  const visible = new Set(
    edges.filter(
      (edge) =>
        (!connection || edge === connection) &&
        (!current ||
          state ||
          edge.from === current.name ||
          edge.to === current.name) &&
        (!state || edge.states.includes(state)),
    ),
  );
  const focused = new Set(current ? [current.name] : []);
  for (const { model } of shared.get(state)?.ports ?? []) focused.add(model);
  for (const { from, to } of visible) {
    focused.add(from);
    focused.add(to);
  }
  const changed = new Set(model.preview.files.map(({ name }) => name));
  const changedModel = ({ file, dependencies = [] }) =>
    changed.has(file) ||
    dependencies.some((dependency) => changed.has(dependency));
  const filter = node(
    "select",
    "",
    ...["", ...states].map((state) => {
      const option = text("option", state || "すべての共有状態");
      option.value = state;
      return option;
    }),
  );
  filter.id = "graph-state";
  filter.value = state;
  const showShared = (name) => {
    sharedState = name;
    selectedConnection = undefined;
    render();
    centerGraphParticipants(shared.get(name)?.ports ?? []);
    $("#graph-state")?.focus({ preventScroll: true });
  };
  filter.addEventListener("change", () => showShared(filter.value));
  const summary = descriptionText(model.current.description);
  const intro = node(
    "header",
    "graph-page-heading",
    text("h1", model.current.name),
    summary,
  );
  if (!layout) {
    return [
      intro,
      card(
        "構成モデル",
        summary ? descriptionLanguageSelect() : null,
        text(
          "p",
          model.current.diagnostics.some(({ area }) => area === "layout")
            ? "関係図を配置できませんでした。"
            : "接続ポートを持つ構成モデルがありません。",
        ),
        button("モデルと仕様を開く", () => go("relations"), "inline-link"),
      ),
    ];
  }

  const stage = node(
    "div",
    "graph-stage",
    wireLayer(layout, visible, filtered),
  );
  stage.style.aspectRatio = layout.width + " / " + layout.height;
  const domainNames = new Map(
    layout.domains.map(({ id, name }) => [id, name[descriptionLanguage]]),
  );
  const graphNodes = new Map(
    layout.nodes.map((position) => [position.name, position]),
  );
  const domainOf = (name) => domainNames.get(graphNodes.get(name)?.domain);
  for (const domain of layout.domains) {
    const name = domainNames.get(domain.id);
    const title = text("h2", name, "graph-domain-title");
    title.id = "graph-domain-" + domain.id;
    title.lang = descriptionLanguage;
    const region = node("section", "graph-domain", title);
    region.setAttribute("aria-labelledby", title.id);
    region.style.left = (100 * domain.x) / layout.width + "%";
    region.style.top = (100 * domain.y) / layout.height + "%";
    region.style.width = (100 * domain.width) / layout.width + "%";
    region.style.height = (100 * domain.height) / layout.height + "%";
    stage.prepend(region);
  }
  const { viewport, reset, fit } = zoomableGraph(stage, layout);
  const selectModel = (name) => {
    selectedModel = selectedModel === name ? undefined : name;
    selectedConnection = undefined;
    render();
    const focus = [...document.querySelectorAll(".graph-model")].find(
      (item) => item.dataset.model === name,
    );
    focus?.focus({ preventScroll: true });
  };
  for (const position of layout.nodes) {
    const component = addresses.get(position.name);
    const { name } = component;
    const item = button(
      name,
      () => selectModel(name),
      "graph-model",
      current?.name === name,
    );
    item.classList.add("graph-model-card");
    item.replaceChildren(text("span", name, "graph-model-name"));
    const description = descriptionText(
      component.description,
      "span",
      "graph-model-description",
    );
    const describedBy = [];
    if (position.domain) describedBy.push("graph-domain-" + position.domain);
    if (description) {
      description.id = "graph-description-" + name;
      describedBy.push(description.id);
      item.append(description);
    }
    const counts = stateSummary(modelStates.get(name) ?? []);
    if (counts) {
      counts.id = "graph-state-counts-" + name;
      describedBy.push(counts.id);
      item.classList.add("has-state-summary");
      item.append(counts);
    }
    if (describedBy.length)
      item.setAttribute("aria-describedby", describedBy.join(" "));
    item.setAttribute("aria-label", name);
    item.dataset.model = name;
    item.dataset.dimmed = String(filtered && !focused.has(name));
    item.style.left = (100 * position.x) / layout.width + "%";
    item.style.top = (100 * position.y) / layout.height + "%";
    item.style.width = (100 * position.width) / layout.width + "%";
    item.style.height = (100 * position.height) / layout.height + "%";
    if (changedModel(component)) {
      item.append(node("span", "graph-change"));
      item.setAttribute("aria-label", name + "、保存済みの変更あり");
    }
    stage.append(item);
  }

  const names = stateNames(
    model.current.terms,
    model.model,
    descriptionLanguage,
  );
  const labelOf = (name) => names.get(name) ?? name;
  for (const edge of visible) {
    const caption = edgeCaption(edge, names, state);
    const { x, y, width, height } = edge.label;
    const id = connectionId(edge);
    const item = button(
      caption,
      () => {
        selectedConnection = connection === edge ? undefined : id;
        render();
        [...document.querySelectorAll(".graph-wire-label")]
          .find((label) => label.dataset.connection === id)
          ?.focus({ preventScroll: true });
      },
      edge.crossDomain
        ? "graph-wire-label is-cross-domain"
        : "graph-wire-label",
      connection === edge,
    );
    item.setAttribute(
      "aria-label",
      id + ": " + edge.states.map(labelOf).join(", "),
    );
    item.dataset.connection = id;
    item.title = id + "\n" + edge.states.join(", ");
    item.style.left = (100 * x) / layout.width + "%";
    item.style.top = (100 * y) / layout.height + "%";
    item.style.width = (100 * width) / layout.width + "%";
    item.style.height = (100 * height) / layout.height + "%";
    stage.append(item);
  }

  const surface = node("div", "graph-surface", viewport);
  const diagram = node(
    "section",
    "card graph-card",
    node(
      "div",
      "graph-toolbar",
      node(
        "div",
        "graph-controls",
        node(
          "label",
          "graph-filter",
          text("span", "共有状態で絞り込む"),
          filter,
        ),
        descriptionLanguageSelect(),
        fit,
        reset,
      ),
      filtered
        ? button(
            "絞り込みを解除",
            () => {
              selectedModel = undefined;
              selectedConnection = undefined;
              sharedState = "";
              render();
              $("#graph-state")?.focus();
            },
            "inline-link",
          )
        : null,
    ),
    surface,
    node(
      "p",
      "graph-legend",
      "共有状態の流れ  書き込み → 読み取り",
      layout.domains.length
        ? text("span", "ドメインをまたぐ接続", "graph-boundary-key")
        : null,
      text("span", " · 拡大すると説明と接続ラベルを表示", "graph-zoom-hint"),
    ),
  );
  diagram.setAttribute("aria-label", "構成モデルの関係図");
  const detailsTitle = connection
    ? connectionId(connection) + " の接続"
    : current
      ? current.name + " の接続"
      : "モデル間の接続";
  if (!connection && !current)
    return [intro, node("div", "graph-layout", diagram)];

  const close = button(
    "閉じる",
    () => {
      selectedConnection = undefined;
      selectedModel = undefined;
      render();
      $("#graph-state")?.focus();
    },
    "inline-link",
  );
  const connectionStates = connection
    ? connection.states
        .filter((name) => !state || name === state)
        .map((name) => shared.get(name))
        .filter(Boolean)
    : [];
  const inspectorStates = connectionStates.length
    ? connectionStates
    : (modelStates.get(current?.name) ?? []);
  const details = node(
    "section",
    "card graph-inspector",
    node("header", "graph-inspector-heading", text("h2", detailsTitle), close),
    connection?.crossDomain
      ? text(
          "p",
          (domainOf(connection.from) ?? "境界外") +
            " → " +
            (domainOf(connection.to) ?? "境界外"),
          "graph-domain-path",
        )
      : current && domainOf(current.name)
        ? text("p", domainOf(current.name), "graph-domain-path")
        : null,
    descriptionText(current?.description),
    current
      ? node(
          "div",
          "graph-details-actions",
          button(
            "構成要素を開く",
            () => select("component", current.name),
            "inline-link",
          ),
          changedModel(current) ? chip("保存済みの変更あり", "modified") : null,
        )
      : null,
    inspectorStates.length
      ? stateSections(inspectorStates, state, (name) =>
          showShared(sharedState === name ? "" : name),
        )
      : null,
    connectionStates.length
      ? null
      : visible.size
        ? node(
            "ul",
            "graph-edges",
            ...[...visible].map(({ from, to, states }) =>
              node(
                "li",
                "",
                text(
                  "strong",
                  addresses.get(from).name + " → " + addresses.get(to).name,
                  "mono",
                ),
                node(
                  "div",
                  "graph-edge-states",
                  ...states
                    .filter((name) => !state || name === state)
                    .map((name) => code(name)),
                ),
              ),
            ),
          )
        : text("p", "該当するモデル間の接続はありません。", "muted"),
  );
  details.setAttribute("aria-label", detailsTitle);
  surface.append(details);
  return [intro, node("div", "graph-layout", diagram)];
}

function overview() {
  const { rules, checks, components } = model.current;
  const linked = rules.flatMap((rule) =>
    rule.claims.filter(({ resolved }) => resolved),
  );
  return [
    heading(
      "モデルと仕様",
      model.current.name,
      "仕様の言葉から、検査式とモデルの構成をたどる。",
    ),
    node(
      "div",
      "split",
      card(
        "仕様 → モデル検査",
        rules.length
          ? node(
              "div",
              "relationships",
              ...rules.map((rule) =>
                node(
                  "div",
                  "relation",
                  button(
                    rule.text.ja,
                    () => select("rule", rule.id),
                    "rule-link",
                  ),
                  text("span", "→", "arrow"),
                  node(
                    "div",
                    "check-links",
                    ...(rule.claims.length
                      ? rule.claims.map((claim) =>
                          claim.resolved
                            ? linkCheck(claim.check)
                            : chip(claim.check + " · 未解決", "error"),
                        )
                      : [chip("検査リンクなし")]),
                  ),
                ),
              ),
            )
          : text("p", "仕様ルールがありません。"),
      ),
      node(
        "div",
        "stack",
        card(
          "モデルの構成",
          ...components.map(({ name, file, ports }) =>
            node(
              "div",
              "component-row",
              button(name, () => select("component", name), "inline-link mono"),
              text(
                "small",
                ports.length ? `${ports.length} ポート` : leaf(file),
              ),
            ),
          ),
          components.length ? null : openSource(model.model),
        ),
        card(
          "接続状況",
          text(
            "p",
            `${rules.length} ルール · ${linked.length} 検査リンク`,
            "metric",
          ),
          text(
            "p",
            `${checks.filter(({ rules }) => rules.length === 0).length} 件の検査に仕様リンクなし`,
            "muted",
          ),
        ),
      ),
    ),
  ];
}

function ruleDetail(rule) {
  const terms = model.current.terms.filter(({ id }) =>
    rule.terms?.includes(id),
  );
  return [
    heading("仕様ルール", rule.text.ja, rule.id),
    card(
      "対応するモデル検査",
      ...(rule.claims.length
        ? rule.claims.map((claim) =>
            node(
              "div",
              "claim",
              claim.resolved
                ? linkCheck(claim.check)
                : chip(claim.check + " · 未解決", "error"),
              chip("期待 " + nameOf(claim.expect)),
              claim.clause ? code(claim.clause) : null,
              text("small", claim.model, "muted"),
            ),
          )
        : [text("p", rule.unformalized?.ja ?? "検査リンクがありません。")]),
    ),
    terms.length
      ? card(
          "用語",
          ...terms.map((term) =>
            node(
              "div",
              "term",
              text("strong", term.name?.ja ?? term.id),
              text("p", term.definition?.ja),
              ...(Array.isArray(term.model) ? term.model : []).map((binding) =>
                binding.path === model.model &&
                model.current.states.some(({ name }) => name === binding.name)
                  ? button(
                      binding.name,
                      () => select("state", binding.name),
                      "inline-link mono",
                    )
                  : code(`${binding.name} · ${leaf(binding.path)}`),
              ),
            ),
          ),
        )
      : null,
    model.spec ? card("仕様のソース", openSource(model.spec)) : null,
  ];
}

async function inspectCheck(name) {
  const revision = model.revision;
  busy = true;
  result = undefined;
  render();
  try {
    const query = new URLSearchParams({ check: name, revision });
    const response = await fetch("/api/witness?" + query);
    const data = await response.json();
    if (chosen("check", name) && model.revision === revision) result = data;
  } catch (error) {
    if (chosen("check", name) && model.revision === revision)
      result = { error: error.message };
  } finally {
    busy = false;
    render();
  }
}

function checkDetail(check) {
  const run = button(
    busy ? "検査中…" : "現在のモデルを検査",
    () => inspectCheck(check.name),
    "primary",
  );
  run.disabled = busy;
  const evidence = result?.trace?.commands ?? [];
  return [
    heading(
      nameOf(check.kind),
      check.name,
      check.action ? `対象の振る舞い ${check.action}` : null,
    ),
    node(
      "div",
      "split",
      card("検査式", pretty(check.formula ?? check)),
      node(
        "div",
        "stack",
        card(
          "対応する仕様ルール",
          ...(check.rules.length
            ? check.rules.map((id) =>
                node(
                  "div",
                  "claim",
                  linkRule(id),
                  text(
                    "p",
                    model.current.rules.find((rule) => rule.id === id)?.text.ja,
                  ),
                ),
              )
            : [text("p", "対応する仕様ルールがありません。")]),
        ),
        card(
          "検査結果",
          run,
          result?.error && text("p", result.error, "error-text"),
          result?.verdict &&
            node(
              "p",
              "verdict",
              chip(
                nameOf(
                  result.verdict === "reachable"
                    ? "reachableVerdict"
                    : result.verdict,
                ),
              ),
              chip(result.complete ? "探索完了" : "探索未完了"),
            ),
          evidence.length
            ? node(
                "ol",
                "trace",
                ...evidence.map((command) =>
                  node("li", "", code(JSON.stringify(command))),
                ),
              )
            : null,
        ),
      ),
    ),
  ];
}

function componentDetail(component) {
  const related = model.current.components.filter(
    ({ name }) => name !== component.name,
  );
  return [
    heading("構成要素", component.name, component.file),
    descriptionText(component.description)
      ? card(
          "モデルの説明",
          descriptionLanguageSelect(),
          descriptionText(component.description),
        )
      : null,
    card(
      "接続ポート",
      ...(component.ports.length
        ? component.ports.map((port) => {
            const neighbors = related.filter(({ ports }) =>
              ports.some(
                (other) =>
                  other.state === port.state && other.access !== port.access,
              ),
            );
            return node(
              "div",
              "relation",
              node(
                "div",
                "port",
                code(port.state),
                chip(port.access === "write" ? "書き込み" : "読み取り"),
              ),
              text("span", port.access === "write" ? "→" : "←", "arrow"),
              node(
                "div",
                "check-links",
                ...(neighbors.length
                  ? neighbors.map(({ name }) =>
                      button(
                        name,
                        () => select("component", name),
                        "inline-link mono",
                      ),
                    )
                  : [text("small", "接続相手なし")]),
              ),
            );
          })
        : [text("p", "外部ポートがありません。")]),
    ),
    card("モデルのソース", openSource(component.file)),
  ];
}

function declarationDetail(kind, declaration) {
  const related = model.current.components.filter(({ ports }) =>
    ports.some(({ state }) => state === declaration.name),
  );
  const checks = model.current.checks.filter(
    ({ action }) => action === declaration.name,
  );
  return [
    heading(nameOf(kind), declaration.name),
    card("宣言", pretty(declaration)),
    related.length
      ? card(
          "接続する構成要素",
          ...related.map(({ name }) =>
            node(
              "div",
              "component-row",
              button(name, () => select("component", name), "inline-link mono"),
            ),
          ),
        )
      : null,
    checks.length
      ? card(
          "この振る舞いを検査する条件",
          ...checks.map(({ name }) =>
            node("div", "component-row", linkCheck(name)),
          ),
        )
      : null,
  ];
}
function preview() {
  const { files, symbols } = model.preview;
  const changes =
    symbols === null
      ? text("p", "入力エラーがあるため、宣言の比較は保留しています。")
      : symbols.length
        ? node(
            "ul",
            "symbols",
            ...symbols.map(({ kind, name, status }) =>
              node(
                "li",
                "",
                chip(nameOf(status), status),
                text("span", nameOf(kind), "muted"),
                status !== "removed" && kind === "rule"
                  ? linkRule(name)
                  : status !== "removed" && kind === "check"
                    ? linkCheck(name)
                    : status !== "removed" && ["state", "action"].includes(kind)
                      ? button(
                          name,
                          () => select(kind, name),
                          "inline-link mono",
                        )
                      : code(name),
              ),
            ),
          )
        : text("p", "宣言と検査リンクに差分はありません。");
  return [
    heading(
      "差分プレビュー",
      "保存済みの変更",
      "比較元は Studio 起動時のファイルです。保存すると自動で更新されます。",
    ),
    card("モデルと仕様の宣言", changes),
    files.length
      ? null
      : card("ファイル差分", text("p", "保存済みの変更はありません。")),
    ...files.map(({ name, status, patch }) =>
      node(
        "section",
        "card diff",
        node(
          "header",
          "diff-header",
          text("h2", name),
          chip(nameOf(status), status),
        ),
        node(
          "pre",
          "patch",
          ...patch
            .trimEnd()
            .split("\n")
            .map((line) =>
              text(
                "span",
                line || " ",
                line.startsWith("+")
                  ? "plus"
                  : line.startsWith("-")
                    ? "minus"
                    : line.startsWith("@@")
                      ? "hunk"
                      : "",
              ),
            ),
        ),
      ),
    ),
  ];
}
function render() {
  if (!model) {
    $("#status").textContent = notice ?? "読み込み中";
    return;
  }
  document.body.dataset.view = view;
  renderNavigation();
  const { current } = model;
  const diagnostics = $("#diagnostics");
  diagnostics.hidden = !current.diagnostics.length;
  diagnostics.replaceChildren(
    ...current.diagnostics.map(({ area, message }) =>
      node(
        "p",
        "",
        text(
          "strong",
          area === "model" ? "モデル" : area === "layout" ? "関係図" : "仕様",
        ),
        text("span", message),
      ),
    ),
  );
  let content;
  if (view === "graph") content = graphPage();
  else if (view === "preview") content = preview();
  else if (selection?.kind === "file") {
    const file = selection.id;
    content = [
      heading("ソース", leaf(file), file),
      node(
        "section",
        "card source",
        text("pre", current.files[file] ?? "ファイルを読み込めません。"),
      ),
    ];
  } else {
    const object =
      selection?.kind === "rule"
        ? current.rules.find(({ id }) => id === selection.id)
        : selection?.kind === "check"
          ? current.checks.find(({ name }) => name === selection.id)
          : selection?.kind === "component"
            ? current.components.find(({ name }) => name === selection.id)
            : selection?.kind === "state"
              ? current.states.find(({ name }) => name === selection.id)
              : selection?.kind === "action"
                ? current.actions.find(({ name }) => name === selection.id)
                : undefined;
    content = !object
      ? overview()
      : selection.kind === "rule"
        ? ruleDetail(object)
        : selection.kind === "check"
          ? checkDetail(object)
          : selection.kind === "component"
            ? componentDetail(object)
            : declarationDetail(selection.kind, object);
  }
  const previousGraph = $(".graph-scroll");
  const offset = previousGraph ? graphCenter(previousGraph) : null;
  const x = offset ? previousGraph.scrollLeft - offset.x : 0;
  const y = offset ? previousGraph.scrollTop - offset.y : 0;
  $("#main").replaceChildren(...content.filter(Boolean));
  const viewport = $(".graph-scroll");
  if (viewport) {
    const center = graphCenter(viewport);
    viewport.scrollTo(center.x + x, center.y + y);
  }
}

async function poll() {
  try {
    const params = model ? "?since=" + encodeURIComponent(model.revision) : "";
    const response = await fetch("/api/workspace" + params);
    if (response.status === 204) {
      if (notice) {
        notice = undefined;
        render();
      }
      return;
    }
    const data = await response.json();
    if (!response.ok) throw new Error(data.error);
    model = data;
    notice = undefined;
    result = undefined;
    render();
  } catch (error) {
    notice = error.message;
    render();
  } finally {
    setTimeout(poll, 850);
  }
}
poll();
