<script lang="ts">
  import type { ViewBlockDef, ProfileSection } from "./api";

  let { block, fields, onchange }: {
    block: ViewBlockDef;
    fields: string[];
    onchange: (patch: Partial<ViewBlockDef>) => void;
  } = $props();

  const sections = $derived(block.sections ?? []);

  function emit(next: ProfileSection[]) {
    onchange({ sections: next.length ? next : null });
  }

  function addSection() {
    emit([...sections, { title: "새 섹션", fields: [] }]);
  }

  function removeSection(i: number) {
    emit(sections.filter((_, idx) => idx !== i));
  }

  function setTitle(i: number, title: string) {
    emit(sections.map((s, idx) => (idx === i ? { ...s, title } : s)));
  }

  function toggleField(i: number, field: string, on: boolean) {
    emit(
      sections.map((s, idx) => {
        if (idx !== i) return s;
        const has = s.fields.includes(field);
        if (on && !has) return { ...s, fields: [...s.fields, field] };
        if (!on && has) return { ...s, fields: s.fields.filter((f) => f !== field) };
        return s;
      })
    );
  }
</script>

<div class="profile-sections">
  {#each sections as section, i (i)}
    <fieldset class="section">
      <legend>섹션 {i + 1}: {section.title}</legend>
      <input
        aria-label="섹션 제목"
        value={section.title}
        oninput={(e) => setTitle(i, (e.currentTarget as HTMLInputElement).value)}
      />
      <button type="button" aria-label={`${section.title} 섹션 삭제`} onclick={() => removeSection(i)}>섹션 삭제</button>
      <div class="section-fields">
        {#each fields as f (f)}
          <label>
            <input
              type="checkbox"
              checked={section.fields.includes(f)}
              onchange={(e) => toggleField(i, f, (e.currentTarget as HTMLInputElement).checked)}
            />
            {f}
          </label>
        {/each}
      </div>
    </fieldset>
  {/each}
  <button type="button" class="add-section" onclick={addSection}>+ 섹션 추가</button>
</div>
