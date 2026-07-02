use crate::entity::EntityStore;
use crate::error::ViewError;
use crate::schema::SchemaSet;
use crate::view::model::{PageDef, PageResult};
use crate::view::query::run_view;
use indexmap::IndexMap;
use std::path::Path;

#[derive(Debug)]
pub struct PageSet {
    pages: IndexMap<String, PageDef>,
}

impl PageSet {
    pub fn load_dir(dir: &Path) -> Result<PageSet, ViewError> {
        let mut pages = IndexMap::new();
        let mut page_files: IndexMap<String, String> = IndexMap::new();
        if !dir.exists() {
            return Ok(PageSet { pages });
        }

        let mut files = Vec::new();
        for entry in std::fs::read_dir(dir)? {
            let path = entry?.path();
            if path.is_file()
                && path
                    .extension()
                    .is_some_and(|extension| extension == "yaml" || extension == "yml")
            {
                files.push(path);
            }
        }
        files.sort();

        for path in files {
            let file = path
                .file_name()
                .expect("read_dir entry has a filename")
                .to_string_lossy()
                .to_string();
            let text = std::fs::read_to_string(&path)?;
            let def: PageDef =
                serde_yaml::from_str(&text).map_err(|source| ViewError::Parse {
                    file: file.clone(),
                    source,
                })?;
            if let Some(first) = page_files.get(&def.page) {
                return Err(ViewError::DuplicatePage {
                    file,
                    page: def.page,
                    first: first.clone(),
                });
            }
            page_files.insert(def.page.clone(), file);
            pages.insert(def.page.clone(), def);
        }

        Ok(PageSet { pages })
    }

    pub fn get(&self, name: &str) -> Option<&PageDef> {
        self.pages.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.pages.keys().map(String::as_str).collect()
    }
}

pub async fn run_page(
    store: &EntityStore,
    schemas: &SchemaSet,
    page: &PageDef,
) -> Result<PageResult, ViewError> {
    let mut blocks = Vec::with_capacity(page.blocks.len());
    for block in &page.blocks {
        blocks.push(run_view(store, schemas, block).await?);
    }

    Ok(PageResult {
        page: page.page.clone(),
        blocks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::EntityStore;
    use crate::schema::SchemaSet;
    use serde_json::{json, Map, Value};

    fn schemas() -> SchemaSet {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("할일.yaml"),
            "type: 할일\nfields:\n  내용: { kind: text, required: true }\n  완료: { kind: bool }\n",
        )
        .unwrap();
        std::fs::write(
            dir.path().join("물건.yaml"),
            "type: 물건\nfields:\n  이름: { kind: text, required: true }\n  상태: { kind: enum, options: [위시, 주문됨] }\n  가격: { kind: money }\n",
        )
        .unwrap();
        SchemaSet::load_dir(dir.path()).unwrap()
    }
    fn obj(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[tokio::test]
    async fn 페이지_디렉터리_로드_및_실행() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(
            vdir.path().join("대시보드.yaml"),
            "page: 데일리 대시보드\nblocks:\n  - view: 할 일\n    source: 할일\n    filter: { 완료: false }\n    layout: checklist\n  - view: 지출\n    source: 물건\n    filter: { 상태: 주문됨 }\n    aggregate: { 합계: sum(가격) }\n",
        )
        .unwrap();
        let pages = PageSet::load_dir(vdir.path()).unwrap();
        assert!(pages.get("데일리 대시보드").is_some());
        assert_eq!(pages.names(), ["데일리 대시보드"]);

        let s = schemas();
        let store = EntityStore::open_in_memory().await.unwrap();
        store
            .create(&s, "할일", obj(json!({ "내용": "청소", "완료": false })))
            .await
            .unwrap();
        store
            .create(
                &s,
                "물건",
                obj(json!({ "이름": "A", "상태": "주문됨", "가격": {"amount": 5000.0, "currency":"KRW"} })),
            )
            .await
            .unwrap();

        let result = run_page(&store, &s, pages.get("데일리 대시보드").unwrap())
            .await
            .unwrap();
        assert_eq!(result.page, "데일리 대시보드");
        assert_eq!(result.blocks.len(), 2);
        assert_eq!(result.blocks[0].entities.len(), 1);
        assert_eq!(result.blocks[1].aggregates["합계"], json!(5000.0));
    }

    #[test]
    fn 깨진_페이지_yaml은_파일명_포함_에러() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(vdir.path().join("bad.yaml"), "page: [broken").unwrap();
        let err = PageSet::load_dir(vdir.path()).unwrap_err();
        assert!(err.to_string().contains("bad.yaml"));
    }

    #[test]
    fn 없는_디렉터리는_빈_pageset() {
        let vdir = tempfile::tempdir().unwrap();
        let missing = vdir.path().join("없음");
        let pages = PageSet::load_dir(&missing).unwrap();
        assert!(pages.names().is_empty());
    }

    #[test]
    fn 중복_페이지명은_현재파일과_첫파일을_포함한_에러() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(vdir.path().join("01-first.yaml"), "page: 홈\nblocks: []\n").unwrap();
        std::fs::write(vdir.path().join("02-second.yaml"), "page: 홈\nblocks: []\n").unwrap();

        let err = PageSet::load_dir(vdir.path()).unwrap_err();
        let message = err.to_string();
        assert!(message.contains("홈"));
        assert!(message.contains("01-first.yaml"));
        assert!(message.contains("02-second.yaml"));
    }

    #[test]
    fn 여러_페이지는_파일명_순서대로_names에_나온다() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(vdir.path().join("02-second.yaml"), "page: 두번째\nblocks: []\n").unwrap();
        std::fs::write(vdir.path().join("01-first.yaml"), "page: 첫번째\nblocks: []\n").unwrap();

        let pages = PageSet::load_dir(vdir.path()).unwrap();
        assert_eq!(pages.names(), ["첫번째", "두번째"]);
    }
}
