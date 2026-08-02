import { describe, expect, it } from "vitest";

describe("frontend test harness", () => {
  it("runs in jsdom", () => {
    const element = document.createElement("div");
    element.textContent = "Cadence";

    expect(element).toHaveTextContent("Cadence");
  });
});
