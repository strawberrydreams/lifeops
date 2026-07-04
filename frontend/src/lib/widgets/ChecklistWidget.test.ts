import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import ChecklistWidget from "./ChecklistWidget.svelte";
import { clearRefLabelCache } from "../reflabel";
import type { Entity, SchemaMap } from "../types";

const schemas: SchemaMap = {
  할일: {
    name: "할일",
    behaviors: { recurrence: { flag: "완료", rule: "반복", date: "마감일" } },
    fields: {
      내용: { kind: "text", required: true },
      완료: { kind: "bool", required: false },
      마감일: { kind: "date", required: false },
      우선순위: { kind: "enum", required: false, options: ["높음", "보통", "낮음"] },
      반복: { kind: "text", required: false },
      관련: { kind: "list<ref>", required: false },
    },
  },
};

function block(entities: Entity[]) {
  return { view: "할 일", source: "할일", filter: { 완료: false }, sort: "마감일", layout: "checklist" as const, columns: null, entities, aggregates: {} };
}

function entity(id: string, data: Record<string, unknown>): Entity {
  return { id, type: "할일", data, created_at: "", updated_at: "" };
}

beforeEach(() => clearRefLabelCache());
afterEach(() => vi.unstubAllGlobals());

describe("ChecklistWidget", () => {
  it("체크 토글이 PATCH를 부르고 행을 제거하며 spawned를 추가한다", async () => {
    const spawned = entity("2", { 내용: "청소", 완료: false, 마감일: "2099-01-08", 반복: "매주" });
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true, status: 200,
      json: () => Promise.resolve({ ...entity("1", { 내용: "청소", 완료: true }), spawned }),
    });
    vi.stubGlobal("fetch", fetchMock);
    render(ChecklistWidget, { block: block([entity("1", { 내용: "청소", 완료: false, 반복: "매주" })]), schemas });
    await fireEvent.click(screen.getByRole("checkbox"));
    expect(fetchMock).toHaveBeenCalled();
    const [url, init] = fetchMock.mock.calls[0];
    expect(String(url)).toContain("/api/entities/1");
    expect(init.method).toBe("PATCH");
    expect(JSON.parse(init.body)).toEqual({ 완료: true });
    expect(await screen.findByText(/2099-01-08/)).toBeInTheDocument(); // spawned 표시
  });

  it("빠른 추가가 POST(내용+완료:false)를 부른다", async () => {
    const fetchMock = vi.fn().mockResolvedValue({
      ok: true, status: 201,
      json: () => Promise.resolve(entity("9", { 내용: "새 일", 완료: false })),
    });
    vi.stubGlobal("fetch", fetchMock);
    render(ChecklistWidget, { block: block([]), schemas });
    expect(screen.getByText(/할 일이 없어요/)).toBeInTheDocument(); // 빈 상태
    const input = screen.getByPlaceholderText(/빠른 추가/);
    await fireEvent.input(input, { target: { value: "새 일" } });
    await fireEvent.keyDown(input, { key: "Enter" });
    const [, init] = fetchMock.mock.calls[0];
    expect(JSON.parse(init.body)).toEqual({ type: "할일", data: { 내용: "새 일", 완료: false } });
    expect(await screen.findByText("새 일")).toBeInTheDocument();
  });
});
