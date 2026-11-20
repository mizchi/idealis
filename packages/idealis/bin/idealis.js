#!/usr/bin/env node
if (process.argv[2] === "tlc" || process.argv[2] === "tla") {
  await import("../dist/cli/tla-tools.js");
} else {
  await import("../dist/cli/idealis.js");
}
