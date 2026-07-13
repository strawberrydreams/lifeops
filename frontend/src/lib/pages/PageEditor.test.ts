import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen, waitFor } from "@testing-library/svelte";
import type { SchemaMap } from "../types";

const { getPages, previewPage, createPage, updatePage, deletePage } = vi.hoisted(() => ({
  getPages: vi.fn(),
  previewPage: vi.fn(),
  createPage: vi.fn(),
  updatePage: vi.fn(),
  deletePage: vi.fn(),
}));

vi.mock("../api", async (importOriginal) => {
  const actual = await importOriginal<typeof import("../api")>();
  return { ...actual, getPages, previewPage, createPage, updatePage, deletePage };
});
vi.mock("../viewseed.svelte", () => ({ takePageSeed: () => null }));

import PageEditor from "./PageEditor.svelte";
import { ApiError } from "../api";

const schemas: SchemaMap = {
  할일: { name: "할일", fields: { 내용: { kind: "text", required: true }, 완료: { kind: "bool", required: false } } },
};

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => { resolve = res; reject = rej; });
  return { promise, resolve, reject };
}

function renderedBlock(view: string) {
  return { view, source: "할일", layout: "table" as const, columns: [], entities: [], aggregates: {} };
}

beforeEach(() => {
  vi.clearAllMocks();
  previewPage.mockResolvedValue({ page: "p", blocks: [] });
  getPages.mockResolvedValue({ pages: [] });
  createPage.mockResolvedValue({ ok: true });
  updatePage.mockResolvedValue({ ok: true });
  deletePage.mockResolvedValue(undefined);
});

describe("PageEditor", () => {
  it("블록 추가 버튼이 블록 편집기를 늘린다", async () => {
    render(PageEditor, { schemas, onsaved: () => {}, ondeleted: () => {} });
    expect(screen.queryByLabelText("source")).not.toBeInTheDocument();
    await fireEvent.click(screen.getByRole("button", { name: "+ 블록 추가" }));
    expect(screen.getByLabelText("source")).toBeInTheDocument();
  });

  it("이름을 넣고 저장하면 createPage를 호출하고 onsaved를 부른다", async () => {
    const onsaved = vi.fn();
    render(PageEditor, { schemas, onsaved, ondeleted: () => {} });
    await fireEvent.input(screen.getByLabelText("페이지 이름"), { target: { value: " 대시보드 " } });
    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(createPage).toHaveBeenCalled());
    expect(createPage.mock.calls[0][0]).toMatchObject({ page: "대시보드" });
    expect(onsaved).toHaveBeenCalledWith("대시보드");
  });

  it("편집 모드는 원시 페이지 정의를 한 번 불러와 update와 delete를 수행한다", async () => {
    const onsaved = vi.fn();
    const ondeleted = vi.fn();
    getPages.mockResolvedValue({ pages: [{ page: "홈", blocks: [{ view: "할 일", source: "할일", layout: "checklist" }] }] });
    render(PageEditor, { pageName: "홈", schemas, onsaved, ondeleted });
    await waitFor(() => expect(screen.getByLabelText("source")).toBeInTheDocument());
    expect((screen.getByLabelText("페이지 이름") as HTMLInputElement).value).toBe("홈");
    expect(getPages).toHaveBeenCalledOnce();

    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(updatePage).toHaveBeenCalledWith("홈", expect.objectContaining({ page: "홈" })));
    expect(onsaved).toHaveBeenCalledWith("홈");
    await fireEvent.click(screen.getByRole("button", { name: "페이지 삭제" }));
    await waitFor(() => expect(deletePage).toHaveBeenCalledWith("홈"));
    expect(ondeleted).toHaveBeenCalledOnce();
  });

  it("블록 이동과 삭제가 안정적인 id 기준으로 동작한다", async () => {
    getPages.mockResolvedValue({ pages: [{ page: "홈", blocks: [
      { view: "첫째", source: "할일", layout: "table" },
      { view: "둘째", source: "할일", layout: "table" },
    ] }] });
    render(PageEditor, { pageName: "홈", schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(screen.getAllByLabelText("블록 제목")).toHaveLength(2));
    await fireEvent.click(screen.getAllByLabelText("아래로")[0]);
    expect((screen.getAllByLabelText("블록 제목")[0] as HTMLInputElement).value).toBe("둘째");
    await fireEvent.click(screen.getAllByLabelText("블록 삭제")[0]);
    expect(screen.getAllByLabelText("블록 제목")).toHaveLength(1);
    expect((screen.getByLabelText("블록 제목") as HTMLInputElement).value).toBe("첫째");
  });

  it("미리보기와 저장 오류를 사용자에게 표시한다", async () => {
    previewPage.mockRejectedValue(new Error("nope"));
    createPage.mockRejectedValue(new Error("nope"));
    render(PageEditor, { schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(screen.getByText("미리보기 실패")).toBeInTheDocument(), { timeout: 1000 });
    await fireEvent.input(screen.getByLabelText("페이지 이름"), { target: { value: "실패" } });
    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    await waitFor(() => expect(screen.getByText("저장 실패")).toBeInTheDocument());
  });

  it("늦게 도착한 이전 미리보기 응답을 무시한다", async () => {
    const first = deferred<{ page: string; blocks: ReturnType<typeof renderedBlock>[] }>();
    const second = deferred<{ page: string; blocks: ReturnType<typeof renderedBlock>[] }>();
    previewPage.mockImplementationOnce(() => first.promise).mockImplementationOnce(() => second.promise);
    render(PageEditor, { schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(previewPage).toHaveBeenCalledTimes(1), { timeout: 1000 });
    await fireEvent.input(screen.getByLabelText("페이지 이름"), { target: { value: "최신" } });
    await waitFor(() => expect(previewPage).toHaveBeenCalledTimes(2), { timeout: 1000 });

    second.resolve({ page: "최신", blocks: [renderedBlock("최신 결과")] });
    await waitFor(() => expect(screen.getByText(/최신 결과/)).toBeInTheDocument());
    first.resolve({ page: "이전", blocks: [renderedBlock("이전 결과")] });
    await Promise.resolve();
    expect(screen.queryByText(/이전 결과/)).not.toBeInTheDocument();
  });

  it("pageName 변경 시 새 페이지를 다시 로드하고 이전 load 응답을 무시한다", async () => {
    const oldLoad = deferred<{ pages: { page: string; blocks: { view: string; source: string; layout: "table" }[] }[] }>();
    const newLoad = deferred<{ pages: { page: string; blocks: { view: string; source: string; layout: "table" }[] }[] }>();
    getPages.mockImplementationOnce(() => oldLoad.promise).mockImplementationOnce(() => newLoad.promise);
    const view = render(PageEditor, { pageName: "이전", schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(getPages).toHaveBeenCalledTimes(1));
    await view.rerender({ pageName: "최신", schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(getPages).toHaveBeenCalledTimes(2));
    newLoad.resolve({ pages: [{ page: "최신", blocks: [{ view: "최신 블록", source: "할일", layout: "table" }] }] });
    await waitFor(() => expect(screen.getByDisplayValue("최신 블록")).toBeInTheDocument());
    oldLoad.resolve({ pages: [{ page: "이전", blocks: [{ view: "이전 블록", source: "할일", layout: "table" }] }] });
    await Promise.resolve();
    expect(screen.queryByDisplayValue("이전 블록")).not.toBeInTheDocument();
    expect(screen.getByLabelText("페이지 이름")).toHaveValue("최신");
  });

  it("로드 실패를 표시하고 저장을 차단한다", async () => {
    getPages.mockRejectedValue(new ApiError(500, "load", "불러올 수 없음"));
    render(PageEditor, { pageName: "홈", schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("불러올 수 없음"));
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(updatePage).not.toHaveBeenCalled();
  });

  it("삭제 실패를 표시하고 중복 요청과 ondeleted를 막는다", async () => {
    const deletion = deferred<void>();
    const ondeleted = vi.fn();
    deletePage.mockReturnValue(deletion.promise);
    getPages.mockResolvedValue({ pages: [{ page: "홈", blocks: [] }] });
    render(PageEditor, { pageName: "홈", schemas, onsaved: () => {}, ondeleted });
    const button = await screen.findByRole("button", { name: "페이지 삭제" });
    await waitFor(() => expect(button).not.toBeDisabled());
    await fireEvent.click(button);
    await fireEvent.click(button);
    expect(deletePage).toHaveBeenCalledOnce();
    deletion.reject(new ApiError(500, "delete", "삭제할 수 없음"));
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("삭제할 수 없음"));
    expect(ondeleted).not.toHaveBeenCalled();
  });

  it("저장 중 pageName 전환 후 이전 요청의 callback과 오류를 무시하고 busy를 초기화한다", async () => {
    const creation = deferred<{ ok: boolean }>();
    const onsaved = vi.fn();
    createPage.mockReturnValue(creation.promise);
    getPages.mockResolvedValue({ pages: [{ page: "홈", blocks: [] }] });
    const view = render(PageEditor, { schemas, onsaved, ondeleted: () => {} });
    await fireEvent.input(screen.getByLabelText("페이지 이름"), { target: { value: "새 페이지" } });
    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();

    await view.rerender({ pageName: "홈", schemas, onsaved, ondeleted: () => {} });
    await waitFor(() => expect(screen.getByRole("button", { name: "저장" })).not.toBeDisabled());
    creation.reject(new ApiError(500, "save", "오래된 저장 실패"));
    await Promise.resolve();
    expect(onsaved).not.toHaveBeenCalled();
    expect(screen.queryByText("오래된 저장 실패")).not.toBeInTheDocument();
  });

  it("삭제 중 unmount 뒤 완료된 요청은 ondeleted를 호출하지 않는다", async () => {
    const deletion = deferred<void>();
    const ondeleted = vi.fn();
    deletePage.mockReturnValue(deletion.promise);
    getPages.mockResolvedValue({ pages: [{ page: "홈", blocks: [] }] });
    const view = render(PageEditor, { pageName: "홈", schemas, onsaved: () => {}, ondeleted });
    const button = await screen.findByRole("button", { name: "페이지 삭제" });
    await waitFor(() => expect(button).not.toBeDisabled());
    await fireEvent.click(button);
    view.unmount();
    deletion.resolve();
    await Promise.resolve();
    expect(ondeleted).not.toHaveBeenCalled();
  });

  it("목록에 pageName이 없으면 load 오류를 표시하고 저장을 차단한다", async () => {
    getPages.mockResolvedValue({ pages: [] });
    render(PageEditor, { pageName: "사라진 페이지", schemas, onsaved: () => {}, ondeleted: () => {} });
    await waitFor(() => expect(screen.getByRole("alert")).toHaveTextContent("페이지를 찾을 수 없습니다: 사라진 페이지"));
    expect(screen.getByRole("button", { name: "저장" })).toBeDisabled();
    await fireEvent.click(screen.getByRole("button", { name: "저장" }));
    expect(updatePage).not.toHaveBeenCalled();
  });
});
