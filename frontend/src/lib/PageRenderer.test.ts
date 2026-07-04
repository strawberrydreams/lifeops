import { describe, it, expect } from "vitest";
import { render, screen } from "@testing-library/svelte";
import PageRenderer from "./PageRenderer.svelte";
import type { PageBlock } from "./api";
import type { SchemaMap } from "./types";

const schemas: SchemaMap = {
  물건: {
    name: "물건",
    extends: null,
    fields: {
      이름: { kind: "text", required: true },
      가격: { kind: "money", required: false },
    },
  },
};

describe("PageRenderer", () => {
  it("layout이 card면 카드 레이아웃을 렌더링한다 (table 아님)", () => {
    const blocks: PageBlock[] = [
      {
        view: "카드뷰",
        source: "물건",
        layout: "card",
        columns: ["이름"],
        entities: [{ id: "e1", type: "물건", data: { 이름: "A" }, created_at: "", updated_at: "" }],
        aggregates: {},
      },
    ];
    const { container, getByText } = render(PageRenderer, { page: "테스트", blocks, schemas });

    expect(container.querySelector(".card")).toBeInTheDocument();
    expect(container.querySelector("table")).not.toBeInTheDocument();
    expect(getByText(/A/)).toBeInTheDocument();
  });

  it("money 필드는 formatValue로 포맷되어 렌더링된다 ([object Object] 아님)", () => {
    const blocks: PageBlock[] = [
      {
        view: "카드뷰",
        source: "물건",
        layout: "card",
        columns: ["이름", "가격"],
        entities: [
          { id: "e1", type: "물건", data: { 이름: "A", 가격: { amount: 1000, currency: "KRW" } }, created_at: "", updated_at: "" },
        ],
        aggregates: {},
      },
    ];
    const { container, getByText } = render(PageRenderer, { page: "테스트", blocks, schemas });

    expect(getByText(/KRW/)).toBeInTheDocument();
    expect(container.textContent).not.toContain("[object Object]");
  });

  it("table 블록 제목이 browse 링크가 된다", () => {
    const blocks: PageBlock[] = [{
      view: "다가오는", source: "물건",
      filter: { 상태: "주문됨", 배송예정일: { lte: "$today+7d" } }, sort: "배송예정일",
      layout: "table" as const, columns: ["이름"], entities: [], aggregates: {},
    }];
    render(PageRenderer, { page: "홈", blocks, schemas: {} });
    const link = screen.getByRole("link", { name: /다가오는/ });
    const href = link.getAttribute("href")!;
    expect(href).toContain("/browse/%EB%AC%BC%EA%B1%B4");
    expect(decodeURIComponent(href)).toContain("상태=주문됨");
    expect(decodeURIComponent(href)).toContain("배송예정일=lte:$today+7d");
    expect(decodeURIComponent(href)).toContain("sort=배송예정일");
  });
});
