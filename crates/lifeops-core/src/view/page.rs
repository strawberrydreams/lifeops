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
        Ok(PageSet::from_files(load_page_files(dir)?))
    }

    pub fn from_files(files: IndexMap<String, (PageDef, String)>) -> PageSet {
        let pages = files
            .into_iter()
            .map(|(name, (def, _file))| (name, def))
            .collect();
        PageSet { pages }
    }

    pub fn get(&self, name: &str) -> Option<&PageDef> {
        self.pages.get(name)
    }

    pub fn names(&self) -> Vec<&str> {
        self.pages.keys().map(String::as_str).collect()
    }

    pub fn all(&self) -> Vec<&PageDef> {
        self.pages.values().collect()
    }
}

/// 페이지 디렉터리를 읽어 페이지명 -> (정의, 파일명) 맵을 만든다.
/// 없는 디렉터리는 빈 맵, 깨진 YAML은 파일명 포함 Parse 에러, 중복 페이지명은 DuplicatePage.
pub fn load_page_files(dir: &Path) -> Result<IndexMap<String, (PageDef, String)>, ViewError> {
    let mut out: IndexMap<String, (PageDef, String)> = IndexMap::new();
    if !dir.exists() {
        return Ok(out);
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
        let def: PageDef = serde_yaml::from_str(&text).map_err(|source| ViewError::Parse {
            file: file.clone(),
            source,
        })?;
        if let Some((_, first)) = out.get(&def.page) {
            return Err(ViewError::DuplicatePage {
                file,
                page: def.page,
                first: first.clone(),
            });
        }
        out.insert(def.page.clone(), (def, file));
    }
    Ok(out)
}

/// PageDef를 YAML 문자열로 직렬화(원자적 저장용).
pub fn to_yaml(page: &PageDef) -> Result<String, serde_yaml::Error> {
    serde_yaml::to_string(page)
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
        std::fs::write(
            vdir.path().join("02-second.yaml"),
            "page: 두번째\nblocks: []\n",
        )
        .unwrap();
        std::fs::write(
            vdir.path().join("01-first.yaml"),
            "page: 첫번째\nblocks: []\n",
        )
        .unwrap();

        let pages = PageSet::load_dir(vdir.path()).unwrap();
        assert_eq!(pages.names(), ["첫번째", "두번째"]);
    }

    #[test]
    fn 차트_프로필_블록_yaml_왕복_보존() {
        let yaml_in = "page: 대시보드\nblocks:\n  - view: 체중\n    source: 측정\n    filter: { 지표: 체중 }\n    layout: chart\n    x: 시각\n    y: 값\n    series: 지표\n    chart_type: bar\n  - view: 내 프로필\n    source: 프로필\n    layout: profile\n    sections:\n      - { title: 기본, fields: [이름, 생년] }\n";
        let def: PageDef = serde_yaml::from_str(yaml_in).unwrap();
        let yaml_out = super::to_yaml(&def).unwrap();
        assert!(yaml_out.contains("layout: chart"), "{yaml_out}");
        assert!(
            yaml_out.contains("x: 시각")
                && yaml_out.contains("y: 값")
                && yaml_out.contains("series: 지표"),
            "차트 축 보존:\n{yaml_out}"
        );
        assert!(yaml_out.contains("chart_type: bar"), "{yaml_out}");
        assert!(yaml_out.contains("layout: profile"), "{yaml_out}");
        assert!(yaml_out.contains("title: 기본"), "{yaml_out}");

        let reparsed: PageDef = serde_yaml::from_str(&yaml_out).unwrap();
        assert_eq!(reparsed.blocks.len(), 2);
        assert_eq!(reparsed.blocks[0].x.as_deref(), Some("시각"));
        assert_eq!(reparsed.blocks[0].chart_type, crate::view::ChartType::Bar);
        assert_eq!(
            reparsed.blocks[1].sections.as_ref().unwrap()[0].title,
            "기본"
        );
    }

    #[test]
    fn 체크리스트_블록은_불필요_필드를_생략한다() {
        let yaml_in = "page: 홈\nblocks:\n  - view: 할 일\n    source: 할일\n    filter: { 완료: false }\n    layout: checklist\n";
        let def: PageDef = serde_yaml::from_str(yaml_in).unwrap();
        let yaml_out = super::to_yaml(&def).unwrap();
        assert!(yaml_out.contains("layout: checklist"), "{yaml_out}");
        assert!(!yaml_out.contains("sort"), "None sort 생략:\n{yaml_out}");
        assert!(
            !yaml_out.contains("aggregate"),
            "None aggregate 생략:\n{yaml_out}"
        );
        assert!(
            !yaml_out.contains("chart_type"),
            "기본 chart_type(line) 생략:\n{yaml_out}"
        );
        assert!(
            !yaml_out.contains("sections"),
            "None sections 생략:\n{yaml_out}"
        );
    }

    #[test]
    fn load_page_files는_페이지명과_파일명을_매핑한다() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(
            vdir.path().join("대시보드.yaml"),
            "page: 대시보드\nblocks: []\n",
        )
        .unwrap();
        let files = super::load_page_files(vdir.path()).unwrap();
        let (def, file) = &files["대시보드"];
        assert_eq!(file, "대시보드.yaml");
        assert_eq!(def.page, "대시보드");
    }

    #[test]
    fn all은_로드_순서대로_정의를_돌려준다() {
        let vdir = tempfile::tempdir().unwrap();
        std::fs::write(
            vdir.path().join("02-second.yaml"),
            "page: 두번째\nblocks: []\n",
        )
        .unwrap();
        std::fs::write(
            vdir.path().join("01-first.yaml"),
            "page: 첫번째\nblocks: []\n",
        )
        .unwrap();
        let pages = PageSet::load_dir(vdir.path()).unwrap();
        let names: Vec<&str> = pages.all().iter().map(|d| d.page.as_str()).collect();
        assert_eq!(names, ["첫번째", "두번째"]);
    }
}
