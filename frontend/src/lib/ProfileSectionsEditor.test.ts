import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent, within } from "@testing-library/svelte";
import ProfileSectionsEditor from "./ProfileSectionsEditor.svelte";
import type { ViewBlockDef } from "./api";

const fields = ["이름", "생년", "거주지"];
function base(over: Partial<ViewBlockDef> = {}): ViewBlockDef {
  return { view: "내 프로필", source: "프로필", layout: "profile", ...over };
}

describe("ProfileSectionsEditor", () => {
  it("섹션 추가 버튼이 빈 섹션을 emit한다", async () => {
    const onchange = vi.fn();
    render(ProfileSectionsEditor, { block: base(), fields, onchange });
    await fireEvent.click(screen.getByRole("button", { name: "+ 섹션 추가" }));
    expect(onchange).toHaveBeenCalledWith({ sections: [{ title: "새 섹션", fields: [] }] });
  });

  it("섹션의 필드 체크박스를 켜면 sections에 반영한다", async () => {
    const onchange = vi.fn();
    render(ProfileSectionsEditor, { block: base({ sections: [{ title: "기본", fields: [] }] }), fields, onchange });
    await fireEvent.click(screen.getByLabelText("이름"));
    expect(onchange).toHaveBeenCalledWith({ sections: [{ title: "기본", fields: ["이름"] }] });
  });

  it("섹션 제목을 바꾸면 반영한다", async () => {
    const onchange = vi.fn();
    render(ProfileSectionsEditor, { block: base({ sections: [{ title: "기본", fields: [] }] }), fields, onchange });
    await fireEvent.input(screen.getByLabelText("섹션 제목"), { target: { value: "생활" } });
    expect(onchange).toHaveBeenCalledWith({ sections: [{ title: "생활", fields: [] }] });
  });

  it("마지막 섹션을 삭제하면 sections를 null로 emit한다", async () => {
    const onchange = vi.fn();
    render(ProfileSectionsEditor, { block: base({ sections: [{ title: "기본", fields: [] }] }), fields, onchange });

    await fireEvent.click(screen.getByRole("button", { name: "기본 섹션 삭제" }));

    expect(onchange).toHaveBeenCalledWith({ sections: null });
  });

  it("선택한 필드를 해제하면 해당 섹션에서 제거한다", async () => {
    const onchange = vi.fn();
    render(ProfileSectionsEditor, {
      block: base({ sections: [{ title: "기본", fields: ["이름", "생년"] }] }),
      fields,
      onchange,
    });

    await fireEvent.click(screen.getByLabelText("이름"));

    expect(onchange).toHaveBeenCalledWith({ sections: [{ title: "기본", fields: ["생년"] }] });
  });

  it("복수 섹션에서는 선택한 섹션만 변경하고 원본을 변경하지 않는다", async () => {
    const sections = [
      { title: "기본", fields: ["이름"] },
      { title: "생활", fields: ["거주지"] },
    ];
    const block = base({ sections });
    const onchange = vi.fn();
    render(ProfileSectionsEditor, { block, fields, onchange });

    const second = screen.getByRole("group", { name: "섹션 2: 생활" });
    await fireEvent.click(within(second).getByLabelText("생년"));

    expect(onchange).toHaveBeenCalledWith({
      sections: [
        { title: "기본", fields: ["이름"] },
        { title: "생활", fields: ["거주지", "생년"] },
      ],
    });
    expect(block.sections).toEqual(sections);
    expect(block.sections?.[1].fields).toEqual(["거주지"]);
  });

  it("각 섹션은 제목과 순번으로 구분되는 접근성 그룹을 제공한다", () => {
    render(ProfileSectionsEditor, {
      block: base({
        sections: [
          { title: "기본", fields: [] },
          { title: "생활", fields: [] },
        ],
      }),
      fields,
      onchange: () => {},
    });

    const first = screen.getByRole("group", { name: "섹션 1: 기본" });
    const second = screen.getByRole("group", { name: "섹션 2: 생활" });
    expect(within(first).getByLabelText("섹션 제목")).toHaveValue("기본");
    expect(within(second).getByLabelText("섹션 제목")).toHaveValue("생활");
    expect(within(first).getByRole("button", { name: "기본 섹션 삭제" })).toBeInTheDocument();
    expect(within(second).getByRole("button", { name: "생활 섹션 삭제" })).toBeInTheDocument();
  });
});
