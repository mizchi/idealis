// The cost runner fingerprints these same files when it runs in the checkout.
export const costWorkerFiles: readonly string[] = [
  "dist/evals/experiments/workspace-oracle/cost-worker.js",
  "dist/evals/experiments/workspace-oracle/node.js",
  "dist/evals/experiments/workspace-oracle/snapshot.js",
  "dist/evals/experiments/workspace-oracle/wasm.js",
  "dist/evals/experiments/workspace-oracle/protocol.js",
  "dist/evals/experiments/roracle-native/client.js",
  "dist/evals/experiments/roracle-native/canonical.js",
  "dist/packages/idealis/src/trace.js",
  "crates/roracle/target/wasm32-unknown-unknown/release/roracle.wasm",
];
