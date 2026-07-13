<script lang="ts">
  import type { ViewBlockDef } from "./api";

  let { block, fields, onchange }: {
    block: ViewBlockDef;
    fields: string[];
    onchange: (patch: Partial<ViewBlockDef>) => void;
  } = $props();

  function pick(e: Event): string | null {
    return (e.currentTarget as HTMLSelectElement).value || null;
  }
</script>

<div class="chart-fields">
  <label>x축
    <select aria-label="x축" value={block.x ?? ""} onchange={(e) => onchange({ x: pick(e) })}>
      <option value="">(선택)</option>
      {#each fields as f (f)}<option value={f}>{f}</option>{/each}
    </select>
  </label>
  <label>y축
    <select aria-label="y축" value={block.y ?? ""} onchange={(e) => onchange({ y: pick(e) })}>
      <option value="">(선택)</option>
      {#each fields as f (f)}<option value={f}>{f}</option>{/each}
    </select>
  </label>
  <label>시리즈
    <select aria-label="시리즈" value={block.series ?? ""} onchange={(e) => onchange({ series: pick(e) })}>
      <option value="">(없음)</option>
      {#each fields as f (f)}<option value={f}>{f}</option>{/each}
    </select>
  </label>
  <label>타입
    <select aria-label="차트 타입" value={block.chart_type ?? "line"} onchange={(e) => onchange({ chart_type: (e.currentTarget as HTMLSelectElement).value as "line" | "bar" })}>
      <option value="line">선</option>
      <option value="bar">막대</option>
    </select>
  </label>
</div>
