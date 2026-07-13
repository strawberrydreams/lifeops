<script lang="ts">
  import type { ViewBlockDef, ProfileSection } from "./api";
  import type { SchemaMap } from "./types";
  import ChartBlockFields from "./ChartBlockFields.svelte";
  import ProfileSectionsEditor from "./ProfileSectionsEditor.svelte";

  let { block, schemas, onchange, onremove, onmove }: {
    block: ViewBlockDef; schemas: SchemaMap; onchange: (block: ViewBlockDef) => void;
    onremove: () => void; onmove: (dir: -1 | 1) => void;
  } = $props();

  const LAYOUTS = ["table", "checklist", "card", "chart", "record", "profile"] as const;
  const OPS = ["eq", "lt", "gt", "lte", "gte", "month"] as const;
  const FUNCS = ["count", "sum", "min", "max", "avg"] as const;
  const SORT_SYSTEM = ["created_at", "updated_at"];
  type FilterRow = { field: string; op: string; value: string };
  type AggRow = { name: string; func: string; field: string };

  function initialBlock() { return block; }
  const initial = initialBlock();
  let view = $state(initial.view);
  let source = $state(initial.source);
  let layout = $state<ViewBlockDef["layout"]>(initial.layout);
  let columns = $state<string[]>(initial.columns ?? []);
  let sortField = $state(sortFieldOf(initial.sort));
  let sortDesc = $state((initial.sort ?? "").startsWith("-"));
  let limit = $state<number | null>(initial.limit ?? null);
  let filterRows = $state<FilterRow[]>(rowsFromFilter(initial.filter));
  let aggRows = $state<AggRow[]>(rowsFromAgg(initial.aggregate));
  let chart = $state({ x: initial.x ?? null, y: initial.y ?? null, series: initial.series ?? null, chart_type: initial.chart_type });
  let sections = $state<ProfileSection[] | null>(initial.sections ?? null);

  const fields = $derived(Object.keys(schemas[source]?.fields ?? {}));
  const chartBlock = $derived({ view, source, layout: "chart" as const, ...chart });
  const profileBlock = $derived({ view, source, layout: "profile" as const, sections: sections ?? undefined });

  function sortFieldOf(sort?: string | null) { return !sort ? "" : sort.startsWith("-") ? sort.slice(1) : sort; }
  function rowsFromFilter(filter?: Record<string, unknown> | null): FilterRow[] {
    if (!filter) return [];
    return Object.entries(filter).map(([field, cond]) => {
      if (cond !== null && typeof cond === "object") {
        const [op, value] = Object.entries(cond as Record<string, unknown>)[0] ?? ["eq", ""];
        return { field, op, value: String(value) };
      }
      return { field, op: "eq", value: String(cond) };
    });
  }
  function rowsFromAgg(aggregate?: Record<string, string> | null): AggRow[] {
    if (!aggregate) return [];
    return Object.entries(aggregate).map(([name, expression]) => {
      const match = /^(\w+)\((.*)\)$/.exec(expression);
      return { name, func: match?.[1] ?? "count", field: match?.[2] ?? "" };
    });
  }
  function coerce(field: string, value: string): unknown {
    if (value === "") return "";
    if (schemas[source]?.fields[field]?.kind === "bool") {
      if (value === "true") return true;
      if (value === "false") return false;
    }
    const number = Number(value);
    return Number.isNaN(number) ? value : number;
  }
  function buildFilter(): Record<string, unknown> | null {
    const result: Record<string, unknown> = {};
    for (const row of filterRows) {
      if (!row.field || row.field in result) continue;
      result[row.field] = row.op === "eq" ? coerce(row.field, row.value) : { [row.op]: coerce(row.field, row.value) };
    }
    return Object.keys(result).length ? result : null;
  }
  function buildAggregate(): Record<string, string> | null {
    const result: Record<string, string> = {};
    for (const row of aggRows) if (row.name && row.field && !(row.name in result)) result[row.name] = `${row.func}(${row.field})`;
    return Object.keys(result).length ? result : null;
  }
  function emit() {
    const next: ViewBlockDef = { view, source, layout };
    const filter = buildFilter();
    if (filter) next.filter = filter;
    if (sortField) next.sort = sortDesc ? `-${sortField}` : sortField;
    if ((layout === "table" || layout === "card" || layout === "checklist") && columns.length) next.columns = columns;
    if ((layout === "table" || layout === "card") && limit !== null) next.limit = limit;
    const aggregate = buildAggregate();
    if (aggregate) next.aggregate = aggregate;
    if (layout === "chart") {
      if (chart.x) next.x = chart.x;
      if (chart.y) next.y = chart.y;
      if (chart.series) next.series = chart.series;
      if (chart.chart_type) next.chart_type = chart.chart_type;
    }
    if (layout === "profile" && sections?.length) next.sections = sections;
    onchange(next);
  }
  function changeSource(next: string) {
    source = next; columns = []; filterRows = []; aggRows = []; sortField = ""; sortDesc = false; limit = null;
    chart = { x: null, y: null, series: null, chart_type: undefined }; sections = null; emit();
  }
  function toggleColumn(field: string, checked: boolean) { columns = checked ? [...columns, field] : columns.filter((item) => item !== field); emit(); }
  function duplicateFilter(index: number) { return !!filterRows[index].field && filterRows.findIndex((row) => row.field === filterRows[index].field) !== index; }
  function duplicateAggregate(index: number) { return !!aggRows[index].name && aggRows.findIndex((row) => row.name === aggRows[index].name) !== index; }
  function parseLimit(value: string): number | null {
    const number = Number(value);
    return value !== "" && Number.isInteger(number) && number >= 0 ? number : null;
  }
  function onSub(patch: Partial<ViewBlockDef>) {
    if ("x" in patch) chart.x = patch.x ?? null;
    if ("y" in patch) chart.y = patch.y ?? null;
    if ("series" in patch) chart.series = patch.series ?? null;
    if ("chart_type" in patch) chart.chart_type = patch.chart_type;
    if ("sections" in patch) sections = patch.sections ?? null;
    emit();
  }
</script>

<fieldset class="block-editor">
  <div class="block-toolbar">
    <input aria-label="블록 제목" value={view} oninput={(event) => { view = event.currentTarget.value; emit(); }} />
    <button type="button" aria-label="위로" onclick={() => onmove(-1)}>↑</button>
    <button type="button" aria-label="아래로" onclick={() => onmove(1)}>↓</button>
    <button type="button" aria-label="블록 삭제" onclick={onremove}>삭제</button>
  </div>
  <label>source <select aria-label="source" value={source} onchange={(event) => changeSource(event.currentTarget.value)}>{#each Object.keys(schemas) as type (type)}<option value={type}>{type}</option>{/each}</select></label>
  <label>레이아웃 <select aria-label="레이아웃" value={layout} onchange={(event) => { layout = event.currentTarget.value as ViewBlockDef["layout"]; emit(); }}>{#each LAYOUTS as item (item)}<option value={item}>{item}</option>{/each}</select></label>

  {#if layout === "table" || layout === "card" || layout === "checklist"}
    <div class="columns"><span>열</span>{#each fields as field (field)}<label><input type="checkbox" checked={columns.includes(field)} onchange={(event) => toggleColumn(field, event.currentTarget.checked)} /> {field}</label>{/each}</div>
  {/if}

  <div class="filters"><span>필터</span>
    {#each filterRows as row, index (index)}<div class="filter-row">
      <select aria-label="필터 필드" value={row.field} onchange={(event) => { filterRows[index].field = event.currentTarget.value; emit(); }}><option value="">(필드)</option>{#each fields as field (field)}<option value={field}>{field}</option>{/each}</select>
      <select aria-label="필터 연산자" value={row.op} onchange={(event) => { filterRows[index].op = event.currentTarget.value; emit(); }}>{#each OPS as op (op)}<option value={op}>{op}</option>{/each}</select>
      <input aria-label="필터 값" placeholder="값 또는 $today-7d" value={row.value} oninput={(event) => { filterRows[index].value = event.currentTarget.value; emit(); }} />
      <button type="button" aria-label={`필터 ${index + 1} 삭제`} onclick={() => { filterRows = filterRows.filter((_, i) => i !== index); emit(); }}>×</button>
      {#if duplicateFilter(index)}<span role="alert">중복 값은 무시됩니다</span>{/if}
    </div>{/each}
    <button type="button" class="add-filter" onclick={() => { filterRows = [...filterRows, { field: fields[0] ?? "", op: "eq", value: "" }]; emit(); }}>+ 필터</button>
  </div>

  <label>정렬 <select aria-label="정렬 필드" value={sortField} onchange={(event) => { sortField = event.currentTarget.value; emit(); }}><option value="">(없음)</option>{#each fields as field (field)}<option value={field}>{field}</option>{/each}{#each SORT_SYSTEM as field (field)}<option value={field}>{field}</option>{/each}</select></label>
  <label><input aria-label="내림차순" type="checkbox" checked={sortDesc} onchange={(event) => { sortDesc = event.currentTarget.checked; emit(); }} /> 내림차순</label>
  {#if layout === "table" || layout === "card"}<label>limit <input type="number" min="0" step="1" aria-label="limit" value={limit ?? ""} oninput={(event) => { limit = parseLimit(event.currentTarget.value); emit(); }} /></label>{/if}

  <div class="agg"><span>집계</span>
    {#each aggRows as row, index (index)}<div class="agg-row">
      <input aria-label="집계 이름" value={row.name} oninput={(event) => { aggRows[index].name = event.currentTarget.value; emit(); }} />
      <select aria-label="집계 함수" value={row.func} onchange={(event) => { aggRows[index].func = event.currentTarget.value; emit(); }}>{#each FUNCS as func (func)}<option value={func}>{func}</option>{/each}</select>
      <select aria-label="집계 필드" value={row.field} onchange={(event) => { aggRows[index].field = event.currentTarget.value; emit(); }}><option value="">(필드)</option>{#each fields as field (field)}<option value={field}>{field}</option>{/each}</select>
      <button type="button" aria-label={`집계 ${index + 1} 삭제`} onclick={() => { aggRows = aggRows.filter((_, i) => i !== index); emit(); }}>×</button>
      {#if duplicateAggregate(index)}<span role="alert">중복 값은 무시됩니다</span>{/if}
    </div>{/each}
    <button type="button" class="add-agg" onclick={() => { aggRows = [...aggRows, { name: "", func: "count", field: "" }]; emit(); }}>+ 집계</button>
  </div>

  {#if layout === "chart"}<ChartBlockFields block={chartBlock} fields={fields} onchange={onSub} />{:else if layout === "profile"}<ProfileSectionsEditor block={profileBlock} fields={fields} onchange={onSub} />{/if}
</fieldset>
