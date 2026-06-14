import fs from "node:fs";
import path from "node:path";

import { defineConfig } from "cypress";

export default defineConfig({
  e2e: {
    defaultCommandTimeout: 120_000,
    pageLoadTimeout: 20_000,
    baseUrl: "http://127.0.0.1:8181",
    setupNodeEvents(on) {
      on("task", {
        writeCpuProfile(profileResult: {
          browser: string;
          generatedAt: string;
          profile: unknown;
        }) {
          const resultsDir = path.join(__dirname, "profile-results", "flamegraphs");
          const timestamp = profileResult.generatedAt.replace(/[:.]/g, "-");
          const fileName = `trace-viewer-${profileResult.browser}-${timestamp}.cpuprofile`;
          const filePath = path.join(resultsDir, fileName);

          fs.mkdirSync(resultsDir, { recursive: true });
          fs.writeFileSync(filePath, `${JSON.stringify(profileResult.profile)}\n`);

          return filePath;
        },

        writeProfileResult(result: { browser: string; generatedAt: string }) {
          const resultsDir = path.join(__dirname, "profile-results");
          const timestamp = result.generatedAt.replace(/[:.]/g, "-");
          const fileName = `trace-viewer-${result.browser}-${timestamp}.json`;
          const filePath = path.join(resultsDir, fileName);

          fs.mkdirSync(resultsDir, { recursive: true });
          fs.writeFileSync(filePath, `${JSON.stringify(result, null, 2)}\n`);

          return filePath;
        }
      });

      on("before:browser:launch", (browser, launchOptions) => {
        if (browser.family === "chromium") {
          launchOptions.args.push("--enable-webgl");
          launchOptions.args.push("--ignore-gpu-blocklist");
          launchOptions.args.push("--use-gl=swiftshader");
          launchOptions.args.push("--enable-unsafe-swiftshader");
        }

        if (browser.family === "firefox") {
          launchOptions.preferences["webgl.disabled"] = false;
          launchOptions.preferences["webgl.force-enabled"] = true;
        }

        return launchOptions;
      });
    },
    specPattern: "cypress/e2e/**/*.cy.ts",
    supportFile: false,
    video: false
  }
});
