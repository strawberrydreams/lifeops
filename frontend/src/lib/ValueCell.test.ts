import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import ValueCell from "./ValueCell.svelte";

describe("ValueCell", () => {
  it("지난 date는 overdue 배지", () => {
    render(ValueCell, { field: { kind: "date", required: false }, value: "2000-01-01", schemas: {} });
    expect(screen.getByText("2000-01-01").className).toContain("overdue");
  });

  it("url은 도메인만 보이는 링크", () => {
    render(ValueCell, { field: { kind: "url", required: false }, value: "https://example.com/x/y", schemas: {} });
    const a = screen.getByRole("link");
    expect(a).toHaveTextContent("example.com");
    expect(a).toHaveAttribute("href", "https://example.com/x/y");
  });

  it("bool은 체크 표시", () => {
    render(ValueCell, { field: { kind: "bool", required: false }, value: true, schemas: {} });
    expect(screen.getByText("✓")).toBeInTheDocument();
  });
});
