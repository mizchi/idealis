import { defineConfig } from "@playwright/test";

export default defineConfig({
  use: { browserName: "chromium" },
  reporter: "list",
  testMatch: "browser.spec.ts",
  projects: [
    {
      name: "core",
      testDir: "tests",
      testMatch:
        /\/tests\/(?:scheduling-browser|check-cache-browser|browser)\.spec\.ts$/,
    },
    {
      name: "portable",
      testDir: "tests",
      testMatch: "portable.browser.spec.ts",
    },
    { name: "rtla", testDir: "tests", testMatch: "rtla.browser.spec.ts" },
    { name: "runtime", testDir: "tests", testMatch: "runtime.browser.spec.ts" },
    {
      name: "oracle",
      testDir: "tests",
      testMatch: [
        "oracle-comparison/browser.spec.ts",
        "workspace-oracle/browser.spec.ts",
      ],
      outputDir: ".artifacts/oracle/browser",
      workers: 1,
    },
    { name: "typed-oracle", testDir: "tests/roracle-wit" },
    {
      name: "sat",
      testDir: "tests/sat-foundation",
      outputDir: ".artifacts/sat-foundation/browser",
      workers: 1,
    },
    { name: "testing", testDir: "tests/testing-core", workers: 1 },
    {
      name: "schedules",
      testDir: "tests/controlled-schedules",
      outputDir: ".artifacts/controlled-schedules/browser",
      workers: 1,
    },
    {
      name: "fuzz",
      testDir: "tests/structured-fuzz",
      outputDir: ".artifacts/structured-fuzz/browser",
      workers: 1,
    },
    {
      name: "smt",
      testDir: "tests/smt-subset",
      testMatch: [
        "browser.spec.ts",
        "expansion-browser.spec.ts",
        "script-browser.spec.ts",
        "cache-browser.spec.ts",
      ],
      workers: 1,
    },
    { name: "alloy", testDir: "tests/alloy-subset", workers: 1 },
    { name: "alloy-instance", testDir: "tests/alloy-instance", workers: 1 },
  ],
});
