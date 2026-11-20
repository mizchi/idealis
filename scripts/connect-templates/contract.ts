// Language-neutral input passed to render(). Names are exactly as declared in Quint.
export interface Field {
  readonly name: string;
  readonly type: "int" | "bool" | "str";
}
export interface Variant {
  readonly name: string;
  readonly fields: readonly Field[];
}
export interface ConnectContract {
  readonly version: 1;
  readonly module: string;
  readonly bindings: { readonly command: string; readonly observation: string };
  readonly variants: readonly Variant[];
  readonly observation: readonly Field[];
}
export type Render = (contract: ConnectContract) => string | Promise<string>;
