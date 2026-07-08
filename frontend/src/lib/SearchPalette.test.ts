import { describe, it, expect, vi, afterEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import SearchPalette from "./SearchPalette.svelte";
import * as api from "./api";
import { navigate } from "./router.svelte";
import type { Category, SchemaMap } from "./types";

vi.mock("./router.svelte", () => ({ navigate: vi.fn() }));

const categories: Category[] = [{ name: "컬렉션", icon: "📦" }, { name: "메모", icon: "📝" }];
const schemas: SchemaMap = {};

function hit(over: Partial<api.SearchHit>): api.SearchHit {
  return {
    id: "x", type: "시계", category: "컬렉션", label: "세이코 미쿠", field: "이름",
    snippet: "세이코 미쿠", match: { start: 0, len: 3 }, singleton: false, href: "/entity/x", ...over,
  };
}

afterEach(() => vi.restoreAllMocks());

describe("SearchPalette", () => {
  it("입력하면 카테고리별 결과와 하이라이트를 보여준다", async () => {
    vi.spyOn(api, "search").mockResolvedValue({
      query: "세이코",
      results: [
        hit({ id: "a" }),
        hit({ id: "b", type: "회고", category: "메모", label: "여름 회고", snippet: "…세이코를 팔았다", match: { start: 1, len: 3 } }),
      ],
      total: 2, truncated: false,
    });
    render(SearchPalette, { open: true, schemas, categories, onclose: vi.fn() });
    await fireEvent.input(screen.getByLabelText("검색어"), { target: { value: "세이코" } });

    expect(await screen.findByText("컬렉션")).toBeInTheDocument();
    expect(screen.getByText("메모")).toBeInTheDocument();
    expect(screen.getAllByText("세이코").length).toBeGreaterThan(0); // <mark>
  });

  it("Enter로 선택 결과 href로 이동하고 닫는다", async () => {
    const onclose = vi.fn();
    vi.spyOn(api, "search").mockResolvedValue({
      query: "세이코", results: [hit({ id: "a", href: "/entity/a" })], total: 1, truncated: false,
    });
    render(SearchPalette, { open: true, schemas, categories, onclose });
    const input = screen.getByLabelText("검색어");
    await fireEvent.input(input, { target: { value: "세이코" } });
    await screen.findByText("세이코 미쿠");
    await fireEvent.keyDown(input, { key: "Enter" });

    expect(navigate).toHaveBeenCalledWith("/entity/a");
    expect(onclose).toHaveBeenCalled();
  });

  it("Escape로 닫는다", async () => {
    const onclose = vi.fn();
    vi.spyOn(api, "search").mockResolvedValue({ query: "", results: [], total: 0, truncated: false });
    render(SearchPalette, { open: true, schemas, categories, onclose });
    await fireEvent.keyDown(screen.getByLabelText("검색어"), { key: "Escape" });
    expect(onclose).toHaveBeenCalled();
  });

  it("무결과면 안내를 보여준다", async () => {
    vi.spyOn(api, "search").mockResolvedValue({ query: "없어요없어요", results: [], total: 0, truncated: false });
    render(SearchPalette, { open: true, schemas, categories, onclose: vi.fn() });
    await fireEvent.input(screen.getByLabelText("검색어"), { target: { value: "없어요없어요" } });
    expect(await screen.findByText("일치하는 항목이 없어요")).toBeInTheDocument();
  });
});
