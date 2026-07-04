import { getEntity } from "./api";
import type { SchemaMap } from "./types";

const cache = new Map<string, Promise<string>>();

/** ref id → 대상의 첫 text 필드 값 (실패/미존재 시 id 축약) */
export function refLabel(id: string, schemas: SchemaMap): Promise<string> {
  if (!cache.has(id)) {
    cache.set(
      id,
      getEntity(id)
        .then(({ entity }) => {
          const fields = schemas[entity.type]?.fields ?? {};
          const first = Object.entries(fields).find(([, f]) => f.kind === "text")?.[0];
          const v = first ? entity.data[first] : null;
          return typeof v === "string" && v ? v : id.slice(0, 8);
        })
        .catch(() => id.slice(0, 8))
    );
  }
  return cache.get(id)!;
}

export function clearRefLabelCache() {
  cache.clear();
}
