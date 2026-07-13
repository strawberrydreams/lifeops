import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/svelte";
import BlockEditor from "./BlockEditor.svelte";
import type { ViewBlockDef } from "./api";
import type { SchemaMap } from "./types";

const schemas: SchemaMap = {
  할일: { name: "할일", fields: { 내용: { kind: "text", required: true }, 완료: { kind: "bool", required: false } } },
  측정: { name: "측정", fields: { 지표: { kind: "enum", required: false }, 값: { kind: "number", required: false }, 시각: { kind: "date", required: false } } },
  프로필: { name: "프로필", fields: { 이름: { kind: "text", required: false }, 거주지: { kind: "text", required: false } } },
};
function base(over: Partial<ViewBlockDef> = {}): ViewBlockDef {
  return { view: "블록", source: "할일", layout: "checklist", ...over };
}
const noop = () => {};

describe("BlockEditor", () => {
  it("source 변경은 새 필드 후보를 표시하고 필드 의존 상태를 초기화한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, {
      block: base({ layout: "chart", columns: ["내용"], filter: { 완료: true }, sort: "-내용", aggregate: { 개수: "count()" }, x: "내용", chart_type: "bar" }),
      schemas, onchange, onremove: noop, onmove: noop,
    });
    await fireEvent.change(screen.getByLabelText("source"), { target: { value: "측정" } });
    const last = onchange.mock.calls.at(-1)![0] as ViewBlockDef;
    expect(last).toEqual({ view: "블록", source: "측정", layout: "chart" });
    await fireEvent.change(screen.getByLabelText("레이아웃"), { target: { value: "table" } });
    expect(screen.getByLabelText("지표")).toBeInTheDocument();
    expect(screen.queryByLabelText("내용")).not.toBeInTheDocument();
  });

  it("열 체크박스는 columns를 추가하고 제거한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ layout: "table", columns: ["내용"] }), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.click(screen.getByLabelText("완료"));
    expect(onchange.mock.calls.at(-1)![0].columns).toEqual(["내용", "완료"]);
    await fireEvent.click(screen.getByLabelText("내용"));
    expect(onchange.mock.calls.at(-1)![0].columns).toEqual(["완료"]);
  });

  it("필터 값은 숫자로 강제하고 비-eq 연산자는 객체 계약으로 emit한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base(), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.click(screen.getByRole("button", { name: "+ 필터" }));
    await fireEvent.change(screen.getByLabelText("필터 연산자"), { target: { value: "gte" } });
    await fireEvent.input(screen.getByLabelText("필터 값"), { target: { value: "42" } });
    expect(onchange.mock.calls.at(-1)![0].filter).toEqual({ 내용: { gte: 42 } });
  });

  it("기존 boolean scalar 필터는 무관한 편집 뒤에도 boolean 타입을 보존한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ filter: { 완료: true } }), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.input(screen.getByLabelText("블록 제목"), { target: { value: "바뀐 제목" } });
    expect(onchange.mock.calls.at(-1)![0].filter).toEqual({ 완료: true });
  });

  it("정렬 방향과 집계 표현식을 직렬화한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base(), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.change(screen.getByLabelText("정렬 필드"), { target: { value: "updated_at" } });
    await fireEvent.click(screen.getByLabelText("내림차순"));
    expect(onchange.mock.calls.at(-1)![0].sort).toBe("-updated_at");
    await fireEvent.click(screen.getByRole("button", { name: "+ 집계" }));
    await fireEvent.input(screen.getByLabelText("집계 이름"), { target: { value: "합계" } });
    await fireEvent.change(screen.getByLabelText("집계 함수"), { target: { value: "sum" } });
    await fireEvent.change(screen.getByLabelText("집계 필드"), { target: { value: "내용" } });
    expect(onchange.mock.calls.at(-1)![0].aggregate).toEqual({ 합계: "sum(내용)" });
  });

  it("count 집계도 필드를 포함하고 기존 count(field)를 보존한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ aggregate: { 개수: "count(내용)" } }), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.input(screen.getByLabelText("블록 제목"), { target: { value: "바뀐 제목" } });
    expect(onchange.mock.calls.at(-1)![0].aggregate).toEqual({ 개수: "count(내용)" });
  });

  it("limit은 0 이상의 정수만 emit한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ layout: "table" }), schemas, onchange, onremove: noop, onmove: noop });
    const limit = screen.getByLabelText("limit");
    await fireEvent.input(limit, { target: { value: "-1" } });
    expect(onchange.mock.calls.at(-1)![0]).not.toHaveProperty("limit");
    await fireEvent.input(limit, { target: { value: "3" } });
    expect(onchange.mock.calls.at(-1)![0].limit).toBe(3);
  });

  it("중복 필터 필드와 집계 이름은 뒤 행을 무시하고 경고한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ filter: { 내용: "원본" }, aggregate: { 개수: "count(내용)" } }), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.click(screen.getByRole("button", { name: "+ 필터" }));
    await fireEvent.input(screen.getAllByLabelText("필터 값")[1], { target: { value: "덮어쓰기" } });
    await fireEvent.click(screen.getByRole("button", { name: "+ 집계" }));
    await fireEvent.input(screen.getAllByLabelText("집계 이름")[1], { target: { value: "개수" } });
    await fireEvent.change(screen.getAllByLabelText("집계 필드")[1], { target: { value: "완료" } });
    const last = onchange.mock.calls.at(-1)![0] as ViewBlockDef;
    expect(last.filter).toEqual({ 내용: "원본" });
    expect(last.aggregate).toEqual({ 개수: "count(내용)" });
    expect(screen.getAllByText("중복 값은 무시됩니다")).toHaveLength(2);
    expect(screen.getByRole("button", { name: "필터 2 삭제" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "집계 2 삭제" })).toBeInTheDocument();
  });

  it("레이아웃 변경은 무관 필드를 생략하고 chart 하위 편집기를 표시한다", async () => {
    const onchange = vi.fn();
    render(BlockEditor, { block: base({ source: "측정", layout: "table", columns: ["지표"], limit: 10 }), schemas, onchange, onremove: noop, onmove: noop });
    await fireEvent.change(screen.getByLabelText("레이아웃"), { target: { value: "chart" } });
    expect(onchange.mock.calls.at(-1)![0]).toEqual({ view: "블록", source: "측정", layout: "chart" });
    expect(screen.getByLabelText("x축")).toBeInTheDocument();
    expect(screen.getByLabelText("차트 타입")).toBeInTheDocument();
  });

  it("profile 레이아웃은 섹션 빌더를 표시한다", async () => {
    render(BlockEditor, { block: base({ source: "프로필" }), schemas, onchange: noop, onremove: noop, onmove: noop });
    await fireEvent.change(screen.getByLabelText("레이아웃"), { target: { value: "profile" } });
    expect(screen.getByRole("button", { name: "+ 섹션 추가" })).toBeInTheDocument();
  });

  it("이동과 삭제 콜백을 전달한다", async () => {
    const onmove = vi.fn();
    const onremove = vi.fn();
    render(BlockEditor, { block: base(), schemas, onchange: noop, onremove, onmove });
    await fireEvent.click(screen.getByLabelText("위로"));
    await fireEvent.click(screen.getByLabelText("아래로"));
    await fireEvent.click(screen.getByLabelText("블록 삭제"));
    expect(onmove.mock.calls).toEqual([[-1], [1]]);
    expect(onremove).toHaveBeenCalledOnce();
  });
});
