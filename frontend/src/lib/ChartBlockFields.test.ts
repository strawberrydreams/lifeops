import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ChartBlockFields from "./ChartBlockFields.svelte";
import type { ViewBlockDef } from "./api";

const fields = ["시각", "값", "지표"];
function base(over: Partial<ViewBlockDef> = {}): ViewBlockDef {
  return { view: "추세", source: "측정", layout: "chart", ...over };
}

describe("ChartBlockFields", () => {
  it("현재 x/y 선택을 반영해 렌더한다", () => {
    render(ChartBlockFields, { block: base({ x: "시각", y: "값" }), fields, onchange: () => {} });
    expect((screen.getByLabelText("x축") as HTMLSelectElement).value).toBe("시각");
    expect((screen.getByLabelText("y축") as HTMLSelectElement).value).toBe("값");
  });

  it("y축을 고르면 onchange로 patch를 emit한다", async () => {
    const onchange = vi.fn();
    render(ChartBlockFields, { block: base(), fields, onchange });
    await fireEvent.change(screen.getByLabelText("y축"), { target: { value: "값" } });
    expect(onchange).toHaveBeenCalledWith(expect.objectContaining({ y: "값" }));
  });

  it("차트 타입을 막대로 바꾸면 chart_type bar를 emit한다", async () => {
    const onchange = vi.fn();
    render(ChartBlockFields, { block: base(), fields, onchange });
    await fireEvent.change(screen.getByLabelText("차트 타입"), { target: { value: "bar" } });
    expect(onchange).toHaveBeenCalledWith(expect.objectContaining({ chart_type: "bar" }));
  });
});
