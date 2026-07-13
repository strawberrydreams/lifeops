<script lang="ts">
  import type { SchemaMap } from "../types";
  import {
    getPages,
    previewPage,
    createPage,
    updatePage,
    deletePage,
    ApiError,
    type ViewBlockDef,
    type PageBlock,
  } from "../api";
  import { takePageSeed } from "../viewseed.svelte";
  import BlockEditor from "../BlockEditor.svelte";
  import PageRenderer from "../PageRenderer.svelte";

  let { pageName, schemas, onsaved, ondeleted }: {
    pageName?: string;
    schemas: SchemaMap;
    onsaved: (name: string) => void;
    ondeleted: () => void;
  } = $props();

  const isEdit = $derived(pageName != null);
  const firstType = $derived(Object.keys(schemas)[0] ?? "");

  function initialName() { return pageName ?? ""; }
  let name = $state(initialName());
  let items = $state<{ id: number; def: ViewBlockDef }[]>([]);
  let nextId = 0;
  let preview = $state<PageBlock[]>([]);
  let previewError = $state<string | null>(null);
  let saveError = $state<string | null>(null);
  let loading = $state(false);
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let deleting = $state(false);
  let loadGeneration = 0;

  function wrap(def: ViewBlockDef) {
    return { id: nextId++, def };
  }

  $effect(() => {
    const requestedName = pageName;
    const generation = ++loadGeneration;
    name = requestedName ?? "";
    items = [];
    loadError = null;
    saveError = null;
    saving = false;
    deleting = false;
    if (requestedName != null) {
      loading = true;
      getPages().then((result) => {
        if (generation !== loadGeneration) return;
        const page = result.pages.find((candidate) => candidate.page === requestedName);
        if (!page) {
          loadError = `페이지를 찾을 수 없습니다: ${requestedName}`;
          loading = false;
          return;
        }
        items = page.blocks.map(wrap);
        loading = false;
      }).catch((error) => {
        if (generation !== loadGeneration) return;
        loadError = error instanceof ApiError ? error.message : "페이지 불러오기 실패";
        loading = false;
      });
    } else {
      const seed = takePageSeed();
      items = seed ? [wrap(seed)] : [];
      loading = false;
    }
    return () => { loadGeneration++; };
  });

  function newBlock(): ViewBlockDef {
    return { view: "새 블록", source: firstType, layout: "table" };
  }

  function addBlock() {
    items = [...items, wrap(newBlock())];
  }

  function updateBlock(id: number, def: ViewBlockDef) {
    items = items.map((item) => item.id === id ? { id: item.id, def } : item);
  }

  function removeBlock(id: number) {
    items = items.filter((item) => item.id !== id);
  }

  function moveBlock(id: number, direction: -1 | 1) {
    const from = items.findIndex((item) => item.id === id);
    const to = from + direction;
    if (from < 0 || to < 0 || to >= items.length) return;
    const next = [...items];
    [next[from], next[to]] = [next[to], next[from]];
    items = next;
  }

  let previewGeneration = 0;
  $effect(() => {
    if (loading || loadError) return;
    const snapshot = {
      page: name || "미리보기",
      blocks: $state.snapshot(items).map((item) => item.def) as ViewBlockDef[],
    };
    const generation = ++previewGeneration;
    let active = true;
    const timer = setTimeout(() => {
      previewPage(snapshot)
        .then((result) => {
          if (!active || generation !== previewGeneration) return;
          preview = result.blocks;
          previewError = null;
        })
        .catch((error) => {
          if (!active || generation !== previewGeneration) return;
          previewError = error instanceof ApiError ? error.message : "미리보기 실패";
        });
    }, 300);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  });

  async function save() {
    if (loading || loadError || saving || deleting || !name.trim()) return;
    saveError = null;
    saving = true;
    const generation = loadGeneration;
    const def = {
      page: name.trim(),
      blocks: $state.snapshot(items).map((item) => item.def) as ViewBlockDef[],
    };
    try {
      if (pageName != null) await updatePage(pageName, def);
      else await createPage(def);
      if (generation === loadGeneration) onsaved(def.page);
    } catch (error) {
      if (generation !== loadGeneration) return;
      saveError = error instanceof ApiError ? error.message : "저장 실패";
    } finally {
      if (generation === loadGeneration) saving = false;
    }
  }

  async function remove() {
    if (pageName == null || loading || loadError || saving || deleting) return;
    saveError = null;
    deleting = true;
    const generation = loadGeneration;
    try {
      await deletePage(pageName);
      if (generation === loadGeneration) ondeleted();
    } catch (error) {
      if (generation !== loadGeneration) return;
      saveError = error instanceof ApiError ? error.message : "삭제 실패";
    } finally {
      if (generation === loadGeneration) deleting = false;
    }
  }
</script>

<div class="page-editor">
  <header class="editor-header">
    <input aria-label="페이지 이름" placeholder="페이지 이름" value={name} oninput={(event) => name = event.currentTarget.value} />
    <button type="button" class="save" onclick={save} disabled={!name.trim() || loading || !!loadError || saving || deleting}>저장</button>
    {#if isEdit}<button type="button" class="delete" onclick={remove} disabled={loading || !!loadError || saving || deleting}>페이지 삭제</button>{/if}
  </header>
  {#if loading}<p aria-live="polite">페이지 불러오는 중…</p>{/if}
  {#if loadError}<p class="error" role="alert">{loadError}</p>{/if}
  {#if saveError}<p class="error" role="alert">{saveError}</p>{/if}

  <div class="editor-body">
    <div class="blocks">
      {#each items as item (item.id)}
        <BlockEditor
          block={item.def}
          {schemas}
          onchange={(def) => updateBlock(item.id, def)}
          onremove={() => removeBlock(item.id)}
          onmove={(direction) => moveBlock(item.id, direction)}
        />
      {/each}
      <button type="button" class="add-block" onclick={addBlock}>+ 블록 추가</button>
    </div>

    <div class="preview">
      <h2>미리보기</h2>
      {#if previewError}
        <p class="error preview-error" role="alert" aria-live="polite">{previewError}</p>
      {:else}
        <PageRenderer page={name || "미리보기"} blocks={preview} {schemas} />
      {/if}
    </div>
  </div>
</div>
