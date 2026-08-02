import { createElement } from "react";
import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

describe("frontend test harness", () => {
  it("renders React components in jsdom", () => {
    render(createElement("div", null, "Cadence"));

    expect(screen.getByText("Cadence")).toBeInTheDocument();
  });
});
