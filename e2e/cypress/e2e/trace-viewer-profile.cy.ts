const viewport = {
  width: 1180,
  height: 760
} as const;

const points = {
  generateButton: { x: 130, y: 164 },
  stepButton: { x: 105, y: 356 }
} as const;

const defaultStepCount = 120;

type TimingSummary = {
  count: number;
  totalMs: number;
  minMs: number;
  meanMs: number;
  medianMs: number;
  p95Ms: number;
  maxMs: number;
};

type ProfileResult = {
  generatedAt: string;
  browser: string;
  cpuProfilePath?: string;
  viewport: typeof viewport;
  puzzle: {
    width: number;
    height: number;
    strategy: string;
  };
  generateMs: number;
  steps: TimingSummary & {
    requested: number;
    completed: number;
    timingsMs: number[];
  };
  finalStepIndex: number;
  finalStatus: string;
};

describe("trace viewer browser performance", () => {
  it("profiles 20x20 side-indexed step and render time", () => {
    const stepCount = Number(Cypress.env("PROFILE_STEPS") ?? defaultStepCount);

    cy.viewport(viewport.width, viewport.height);
    cy.visit("/?width=20&height=20&strategy=side-indexed", {
      onBeforeLoad(window) {
        const diagnostics: string[] = [];
        const originalConsoleError = window.console.error;
        const testWindow = window as Cypress.AUTWindow & {
          __e2eDiagnostics: string[];
          __trunkStarted: boolean;
        };

        testWindow.__e2eDiagnostics = diagnostics;
        testWindow.__trunkStarted = false;
        window.addEventListener("TrunkApplicationStarted", () => {
          testWindow.__trunkStarted = true;
        });
        window.addEventListener("error", (event) => {
          diagnostics.push(`error: ${event.message}`);
        });
        window.addEventListener("unhandledrejection", (event) => {
          diagnostics.push(`unhandledrejection: ${String(event.reason)}`);
        });
        window.console.error = (...args) => {
          diagnostics.push(`console.error: ${args.map(String).join(" ")}`);
          originalConsoleError.apply(window.console, args);
        };
      }
    });

    cy.window({ timeout: 20_000 }).should((window) => {
      const testWindow = window as Cypress.AUTWindow & {
        __e2eDiagnostics: string[];
        __trunkStarted: boolean;
      };

      expect(
        testWindow.__trunkStarted,
        testWindow.__e2eDiagnostics.join("\n")
      ).to.equal(true);
    });

    getCanvas()
      .should(($canvas) => {
        expect($canvas[0].width).to.be.greaterThan(300);
        expect($canvas[0].height).to.be.greaterThan(150);
        expect($canvas.attr("data-width")).to.equal("20");
        expect($canvas.attr("data-height")).to.equal("20");
        expect($canvas.attr("data-strategy")).to.equal("Side indexed");
      });

    let profileResult: ProfileResult;

    startCpuProfile();

    cy.window()
      .then((window) => profileInBrowser(window, stepCount))
      .then((result) => {
        expect(result.steps.completed).to.equal(stepCount);
        expect(result.finalStepIndex).to.be.greaterThan(0);
        expect(result.finalStatus).to.include("executed step");

        profileResult = result;
      });

    stopCpuProfile()
      .then((cpuProfile) => {
        if (!cpuProfile) {
          return null;
        }

        return cy.task("writeCpuProfile", {
          browser: profileResult.browser,
          generatedAt: profileResult.generatedAt,
          profile: cpuProfile
        });
      })
      .then((cpuProfilePath) => {
        if (typeof cpuProfilePath === "string") {
          profileResult.cpuProfilePath = cpuProfilePath;
        }

        cy.task("writeProfileResult", profileResult).then((filePath) => {
          Cypress.log({
            name: "profile",
            message: [
              `${profileResult.browser}: generated in ${formatMs(profileResult.generateMs)}`,
              `${profileResult.steps.completed} steps mean ${formatMs(profileResult.steps.meanMs)}`,
              `p95 ${formatMs(profileResult.steps.p95Ms)}`,
              profileResult.cpuProfilePath
                ? `flamegraph profile ${profileResult.cpuProfilePath}`
                : "flamegraph profile unavailable for this browser",
              `wrote ${filePath}`
            ]
          });
        });
      });
  });
});

function getCanvas() {
  return cy.get<HTMLCanvasElement>("#trace-viewer-canvas");
}

function startCpuProfile() {
  if (!supportsChromeDevToolsProtocol()) {
    cy.log(`CPU flamegraph capture is not available for ${Cypress.browser.name}`);
    return cy.wrap(null);
  }

  return runChromeDevToolsCommand("Profiler.enable").then(() =>
    runChromeDevToolsCommand("Profiler.start")
  );
}

function stopCpuProfile() {
  if (!supportsChromeDevToolsProtocol()) {
    return cy.wrap(null);
  }

  return runChromeDevToolsCommand<{ profile: unknown }>("Profiler.stop").then(
    (result) => result.profile
  );
}

function supportsChromeDevToolsProtocol(): boolean {
  return Cypress.browser.family === "chromium";
}

function runChromeDevToolsCommand<Result = unknown>(command: string) {
  const cypressWithAutomation = Cypress as unknown as {
    automation(
      eventName: "remote:debugger:protocol",
      options: { command: string }
    ): Promise<Result>;
  };

  return cy.then(() =>
    cypressWithAutomation.automation("remote:debugger:protocol", { command })
  ) as Cypress.Chainable<Result>;
}

async function profileInBrowser(
  window: Cypress.AUTWindow,
  stepCount: number
): Promise<ProfileResult> {
  const canvas = window.document.querySelector<HTMLCanvasElement>("#trace-viewer-canvas");

  if (!canvas) {
    throw new Error("trace viewer canvas was not found");
  }

  await waitForInitialCanvas(canvas, window);

  const generateStartedAt = window.performance.now();
  clickCanvas(window, canvas, points.generateButton.x, points.generateButton.y);
  await waitForAnimationFrames(window, 2);
  const generateMs = window.performance.now() - generateStartedAt;

  const timingsMs: number[] = [];

  for (let step = 0; step < stepCount; step += 1) {
    const previousStepIndex = canvasStepIndex(canvas);
    const startedAt = window.performance.now();

    clickCanvas(window, canvas, points.stepButton.x, points.stepButton.y);
    await waitForStepState(canvas, window, previousStepIndex);

    timingsMs.push(window.performance.now() - startedAt);
  }

  return {
    generatedAt: new Date().toISOString(),
    browser: Cypress.browser.name,
    viewport,
    puzzle: {
      width: Number(canvas.getAttribute("data-width")),
      height: Number(canvas.getAttribute("data-height")),
      strategy: canvas.getAttribute("data-strategy") ?? "unknown"
    },
    generateMs,
    steps: {
      requested: stepCount,
      completed: timingsMs.length,
      timingsMs,
      ...summarize(timingsMs)
    },
    finalStepIndex: canvasStepIndex(canvas),
    finalStatus: canvas.getAttribute("data-status") ?? ""
  };
}

async function waitForInitialCanvas(canvas: HTMLCanvasElement, window: Cypress.AUTWindow) {
  await waitUntil(
    window,
    () =>
      canvas.width > 300 &&
      canvas.height > 150 &&
      canvas.getAttribute("data-width") === "20" &&
      canvas.getAttribute("data-height") === "20" &&
      canvas.getAttribute("data-strategy") === "Side indexed",
    "trace viewer metadata did not initialize"
  );
}

async function waitForStepState(
  canvas: HTMLCanvasElement,
  window: Cypress.AUTWindow,
  previousStepIndex: number
) {
  await waitUntil(
    window,
    () => canvasStepIndex(canvas) > previousStepIndex,
    `step ${previousStepIndex + 1} did not render`
  );
}

function clickCanvas(
  window: Cypress.AUTWindow,
  canvas: HTMLCanvasElement,
  x: number,
  y: number
) {
  const rect = canvas.getBoundingClientRect();
  const clientX = rect.left + x;
  const clientY = rect.top + y;
  const eventBase = {
    bubbles: true,
    cancelable: true,
    composed: true,
    clientX,
    clientY,
    button: 0
  };

  canvas.dispatchEvent(
    new window.PointerEvent("pointerdown", {
      ...eventBase,
      pointerId: 1,
      pointerType: "mouse",
      isPrimary: true,
      buttons: 1
    })
  );
  canvas.dispatchEvent(new window.MouseEvent("mousedown", { ...eventBase, buttons: 1 }));
  canvas.dispatchEvent(
    new window.PointerEvent("pointerup", {
      ...eventBase,
      pointerId: 1,
      pointerType: "mouse",
      isPrimary: true,
      buttons: 0
    })
  );
  canvas.dispatchEvent(new window.MouseEvent("mouseup", { ...eventBase, buttons: 0 }));
  canvas.dispatchEvent(new window.MouseEvent("click", eventBase));
}

async function waitForAnimationFrames(window: Cypress.AUTWindow, count: number) {
  for (let frame = 0; frame < count; frame += 1) {
    await new Promise<void>((resolve) => window.requestAnimationFrame(() => resolve()));
  }
}

async function waitUntil(
  window: Cypress.AUTWindow,
  predicate: () => boolean,
  timeoutMessage: string
) {
  const startedAt = window.performance.now();
  const timeoutMs = 10_000;

  while (window.performance.now() - startedAt < timeoutMs) {
    if (predicate()) {
      return;
    }

    await waitForAnimationFrames(window, 1);
  }

  throw new Error(timeoutMessage);
}

function canvasStepIndex(canvas: HTMLCanvasElement): number {
  return Number(canvas.getAttribute("data-step-index") ?? 0);
}

function summarize(values: number[]): TimingSummary {
  const sorted = [...values].sort((left, right) => left - right);
  const totalMs = values.reduce((total, value) => total + value, 0);

  return {
    count: values.length,
    totalMs,
    minMs: sorted[0] ?? 0,
    meanMs: values.length === 0 ? 0 : totalMs / values.length,
    medianMs: percentile(sorted, 0.5),
    p95Ms: percentile(sorted, 0.95),
    maxMs: sorted[sorted.length - 1] ?? 0
  };
}

function percentile(sortedValues: number[], percentileValue: number): number {
  if (sortedValues.length === 0) {
    return 0;
  }

  const index = Math.min(
    sortedValues.length - 1,
    Math.ceil(sortedValues.length * percentileValue) - 1
  );

  return sortedValues[index];
}

function formatMs(value: number): string {
  return `${value.toFixed(2)}ms`;
}

export {};
