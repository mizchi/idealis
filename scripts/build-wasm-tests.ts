import { execFileSync } from "node:child_process";

for (const manifest of [
  "crates/roracle/Cargo.toml",
  "crates/idealis-core/Cargo.toml",
  "crates/rtla/Cargo.toml",
  "crates/rusmt/Cargo.toml",
  "crates/ralloy/Cargo.toml",
  "crates/idealis-connect/Cargo.toml",
  "evals/experiments/rust-probe/Cargo.toml",
  "evals/experiments/sat-foundation/rust/Cargo.toml",
  "evals/experiments/oracle-comparison/rust/Cargo.toml",
]) {
  const args = ["build", "--release", "--locked", "--manifest-path", manifest];
  execFileSync("cargo", args, { stdio: "inherit" });
  execFileSync(
    "cargo",
    [...args, "--lib", "--target", "wasm32-unknown-unknown"],
    { stdio: "inherit" },
  );
}
for (const manifest of [
  "crates/idealis-cli/Cargo.toml",
  "evals/experiments/controlled-schedules/rust/Cargo.toml",
  "evals/experiments/structured-fuzz/rust/Cargo.toml",
])
  execFileSync(
    "cargo",
    ["build", "--release", "--locked", "--manifest-path", manifest],
    { stdio: "inherit" },
  );
