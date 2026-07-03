<script lang="ts">
  import type { Entity, ResolvedSchema } from "./types";
  import { updateEntity } from "./api";
  import { formatValue } from "./format";
  import Widget from "./widgets/Widget.svelte";

  let { schema, entities, columns, onrowclick }: {
    schema: ResolvedSchema;
    entities: Entity[];
    columns?: string[];
    onrowclick?: (e: Entity) => void;
  } = $props();

  let rows = $state<Entity[]>(entities);
  $effect(() => { rows = entities; });

  const cols = $derived(columns ?? Object.keys(schema.fields));
  let editing = $state<{ id: string; field: string } | null>(null);
  let draft = $state<unknown>(null);

  function startEdit(e: Entity, field: string) {
    editing = { id: e.id, field };
    draft = e.data[field] ?? null;
  }
  async function commit(e: Entity, field: string) {
    const patch = { [field]: draft } as Record<string, unknown>;
    const updated = await updateEntity(e.id, patch);
    rows = rows.map((r) => (r.id === e.id ? updated : r)); // 낙관적 갱신(서버 반환으로 교체)
    editing = null;
  }
</script>

<table>
  <thead><tr>{#each cols as c}<th>{c}</th>{/each}</tr></thead>
  <tbody>
    {#each rows as e (e.id)}
      <tr>
        {#each cols as field}
          <td onclick={() => onrowclick?.(e)}>
            {#if editing && editing.id === e.id && editing.field === field}
              <span
                role="cell"
                onkeydown={(ev) => { if (ev.key === "Enter") commit(e, field); if (ev.key === "Escape") editing = null; }}
              >
                <Widget field={schema.fields[field]} value={draft} onchange={(v) => (draft = v)} />
              </span>
            {:else}
              <span
                class="cell"
                role="button"
                tabindex="0"
                onclick={(ev) => { ev.stopPropagation(); startEdit(e, field); }}
              >{formatValue(schema.fields[field], e.data[field])}</span>
            {/if}
          </td>
        {/each}
      </tr>
    {/each}
  </tbody>
</table>
