import { defineConfig } from "cypress";

export default defineConfig({
  e2e: {
    defaultCommandTimeout: 120_000,
    pageLoadTimeout: 20_000,
    baseUrl: "http://127.0.0.1:8181",
    setupNodeEvents(on) {
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
