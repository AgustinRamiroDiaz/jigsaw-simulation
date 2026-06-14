const viewport = {
  width: 1180,
  height: 760
} as const;

const points = {
  generateButton: { x: 130, y: 164 },
  stepButton: { x: 105, y: 356 }
} as const;

describe("trace viewer wasm UI", () => {
  it("renders and drives a 20x20 side-indexed puzzle in the browser", () => {
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

    getCanvas().click(points.generateButton.x, points.generateButton.y);

    getCanvas().should(($canvas) => {
      expect($canvas.attr("data-width")).to.equal("20");
      expect($canvas.attr("data-height")).to.equal("20");
      expect($canvas.attr("data-strategy")).to.equal("Side indexed");
      expect($canvas.attr("data-status")).to.include("puzzle ready");
    });

    Cypress._.times(8, () => {
      getCanvas().click(points.stepButton.x, points.stepButton.y);
    });

    getCanvas().should(($canvas) => {
      expect(Number($canvas.attr("data-step-index"))).to.be.greaterThan(0);
      expect($canvas.attr("data-status")).to.include("executed step");
    });
  });
});

function getCanvas() {
  return cy.get<HTMLCanvasElement>("#trace-viewer-canvas");
}

export {};
