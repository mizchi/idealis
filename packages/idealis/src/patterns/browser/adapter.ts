import type { ObservationAdapter, ObservationProjection } from "../replay.js";

export type Navigation<Route extends string> =
  | { readonly kind: "push"; readonly destination: Route }
  | { readonly kind: "goBack" }
  | { readonly kind: "goForward" };

export interface Screen {
  readonly path: string;
  readonly screen: string | null;
}

export interface BrowserHistory {
  push(path: string): Promise<void>;
  goBack(): Promise<void>;
  goForward(): Promise<void>;
  observe(): Promise<Screen>;
}

export function browserScreenProjection<Route extends string>(
  routes: Readonly<Record<Route, Screen>>,
): ObservationProjection {
  return (state) => {
    const route = state.current;
    if (typeof route !== "string" || !Object.hasOwn(routes, route)) {
      throw new Error(`missing screen for model route ${String(route)}`);
    }
    return routes[route as Route];
  };
}

export function browserHistoryAdapter<Route extends string>(
  history: BrowserHistory,
  routes: Readonly<Record<Route, Screen>>,
): ObservationAdapter<Navigation<Route>> {
  return {
    async apply(command) {
      switch (command.kind) {
        case "push":
          await history.push(routes[command.destination].path);
          break;
        case "goBack":
          await history.goBack();
          break;
        case "goForward":
          await history.goForward();
          break;
      }
    },
    observe: () => history.observe(),
    project: browserScreenProjection(routes),
  };
}
