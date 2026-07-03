<script lang="ts">
  import type { ResolvedField } from "../types";
  import { parseKind } from "../kind";

  let {
    field,
    value,
    onchange,
  }: {
    field: ResolvedField;
    value: unknown;
    onchange: (v: unknown) => void;
  } = $props();

  const parsed = $derived(parseKind(field.kind));
  const kind = $derived(parsed.base);
</script>

{#if !parsed.list && (kind === "text" || kind === "image" || kind === "url")}
  <input
    type={kind === "url" ? "url" : "text"}
    value={(value as string) ?? ""}
    oninput={(e) => onchange((e.currentTarget as HTMLInputElement).value)}
  />
{:else if !parsed.list && kind === "number"}
  <span>
    <input
      type="number"
      value={value === null || value === undefined ? "" : (value as number)}
      oninput={(e) => {
        const v = (e.currentTarget as HTMLInputElement).value;
        onchange(v === "" ? null : Number(v));
      }}
    />
    {#if field.unit}<span class="unit">{field.unit}</span>{/if}
  </span>
{:else if !parsed.list && kind === "date"}
  <input
    type="date"
    value={(value as string) ?? ""}
    oninput={(e) => onchange((e.currentTarget as HTMLInputElement).value || null)}
  />
{:else if !parsed.list && kind === "bool"}
  <input
    type="checkbox"
    checked={value === true}
    onchange={(e) => onchange((e.currentTarget as HTMLInputElement).checked)}
  />
{:else if !parsed.list && kind === "enum"}
  <select
    value={(value as string) ?? ""}
    onchange={(e) => onchange((e.currentTarget as HTMLSelectElement).value || null)}
  >
    <option value=""></option>
    {#each field.options ?? [] as opt}
      <option value={opt}>{opt}</option>
    {/each}
  </select>
{:else}
  <!-- money/ref/richtext/list 는 이후 태스크에서 분기 추가 -->
  <input
    value={value === null || value === undefined ? "" : String(value)}
    oninput={(e) => onchange((e.currentTarget as HTMLInputElement).value)}
  />
{/if}
