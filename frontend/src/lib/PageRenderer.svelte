<script lang="ts">
  import type { PageBlock } from "./api";
  import type { SchemaMap } from "./types";
  import { formatValue } from "./format";
  import { navigate } from "./router.svelte";
  import ChecklistWidget from "./widgets/ChecklistWidget.svelte";

  let { page, blocks, schemas }: { page: string; blocks: PageBlock[]; schemas: SchemaMap } = $props();

  function display(e: PageBlock["entities"][number], c: string): string {
    const field = schemas[e.type]?.fields?.[c];
    if (field) return formatValue(field, e.data[c]);
    const v = e.data[c];
    return typeof v === "object" && v !== null ? JSON.stringify(v) : String(v ?? "");
  }

  function browseUrl(block: PageBlock): string {
    const params = new URLSearchParams();
    for (const [field, cond] of Object.entries(block.filter ?? {})) {
      if (cond !== null && typeof cond === "object") {
        const [op, v] = Object.entries(cond as Record<string, unknown>)[0] ?? [];
        if (op) params.set(field, `${op}:${String(v)}`);
      } else {
        params.set(field, String(cond));
      }
    }
    if (block.sort) params.set("sort", block.sort);
    const q = params.toString();
    return `/browse/${encodeURIComponent(block.source)}${q ? `?${q}` : ""}`;
  }
</script>

<div class="page">
  <h1>{page}</h1>
  {#each blocks as block}
    <section class="block">
      {#if block.layout === "checklist"}
        <h2>{block.view}</h2>
      {:else}
        <h2><a href={browseUrl(block)} onclick={(e) => { e.preventDefault(); navigate(browseUrl(block)); }}>{block.view} ›</a></h2>
      {/if}
      {#if Object.keys(block.aggregates).length > 0}
        <div class="aggregates">
          {#each Object.entries(block.aggregates) as [k, v]}<span class="agg">{k}: {String(v)}</span>{/each}
        </div>
      {/if}
      {#if block.layout === "checklist"}
        <ChecklistWidget block={block} schemas={schemas} />
      {:else if block.layout === "card"}
        <div class="cards">
          {#each block.entities as e}
            <div class="card">
              {#each block.columns ?? Object.keys(e.data) as c}
                <div class="card-field">{c}: {display(e, c)}</div>
              {/each}
            </div>
          {/each}
        </div>
      {:else}
        <table>
          <thead><tr>{#each block.columns ?? [] as c}<th>{c}</th>{/each}</tr></thead>
          <tbody>
            {#each block.entities as e}
              <tr>{#each block.columns ?? [] as c}<td>{display(e, c)}</td>{/each}</tr>
            {/each}
          </tbody>
        </table>
      {/if}
    </section>
  {/each}
</div>
