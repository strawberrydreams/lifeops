import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen } from "@testing-library/svelte";
import Home from "./Home.svelte";

function mockFetch(body: unknown, status = 200) {
  vi.stubGlobal("fetch", vi.fn().mockResolvedValue({
    ok: status < 400,
    status,
    json: () => Promise.resolve(body),
  }));
}

afterEach(() => vi.unstubAllGlobals());

describe("Home", () => {
  it("페이지 '홈'이 있으면 블록을 렌더한다", async () => {
    mockFetch({ page: "홈", blocks: [{ view: "할 일", source: "할일", layout: "checklist", columns: null, entities: [], aggregates: {} }] });
    render(Home, { schemas: {} });
    expect(await screen.findByText("할 일")).toBeInTheDocument();
  });

  it("홈 페이지가 없으면 폴백 안내를 보여준다", async () => {
    mockFetch({ error: { code: "not_found", message: "페이지 없음: 홈" } }, 404);
    render(Home, { schemas: { 노트: { name: "노트", fields: {} } } });
    expect(await screen.findByText(/왼쪽에서 타입을 선택/)).toBeInTheDocument();
  });
});
