import { describe, it, expect, vi, afterEach } from "vitest";
import { render, fireEvent, waitFor } from "@testing-library/svelte";
import EntityTable from "./EntityTable.svelte";
import type { ResolvedSchema, Entity } from "./types";
import * as api from "./api";

afterEach(() => vi.restoreAllMocks());

const schema: ResolvedSchema = {
  name: "물건", extends: null,
  fields: { 이름: { kind: "text", required: true }, 상태: { kind: "enum", required: false, options: ["위시", "보유"] } },
};
const entities: Entity[] = [
  { id: "e1", type: "물건", data: { 이름: "A", 상태: "위시" }, created_at: "", updated_at: "" },
];

describe("EntityTable 인라인 편집", () => {
  it("셀 클릭 → 위젯 전환 → 값 변경·Enter → updateEntity 호출·낙관적 갱신", async () => {
    const spy = vi.spyOn(api, "updateEntity").mockResolvedValue({ ...entities[0], data: { 이름: "A", 상태: "보유" } });
    const { getByText, getByRole } = render(EntityTable, { schema, entities });
    await fireEvent.click(getByText("위시")); // 상태 셀
    const select = getByRole("combobox");
    await fireEvent.change(select, { target: { value: "보유" } });
    await fireEvent.keyDown(select, { key: "Enter" });
    await waitFor(() => expect(spy).toHaveBeenCalledWith("e1", { 상태: "보유" }));
    expect(await (async () => getByText("보유"))()).toBeInTheDocument();
  });
});
