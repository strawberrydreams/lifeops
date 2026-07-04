use lifeops_core::entity::EntityStore;
use lifeops_core::schema::SchemaSet;
use serde_json::{json, Map, Value};

fn obj(v: Value) -> Map<String, Value> {
    v.as_object().unwrap().clone()
}

#[tokio::test]
async fn 시드_스키마로_전체_시나리오() {
    // 저장소 루트의 schemas/ 디렉터리를 그대로 로드한다
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../schemas");
    let schemas = SchemaSet::load_dir(&dir).expect("시드 스키마가 유효해야 함");
    for ty in ["물건", "시계", "노트", "할일"] {
        assert!(schemas.get(ty).is_some(), "{ty} 스키마가 있어야 함");
    }
    // 시계는 물건을 상속
    assert!(schemas.get("시계").unwrap().fields.contains_key("가격"));

    let store = EntityStore::open_in_memory().await.unwrap();
    let watch = store
        .create(
            &schemas,
            "시계",
            obj(json!({
                "이름": "SEIKO x Hatsune Miku LAWSON 50주년",
                "상태": "위시",
                "가격": { "amount": 650000.0, "currency": "KRW" },
                "콜라보": "하츠네 미쿠"
            })),
        )
        .await
        .unwrap();

    // 위시 → 주문됨 (한 곳만 수정)
    store.update(&schemas, &watch.id, obj(json!({ "상태": "주문됨", "배송예정일": "2026-09-15" })))
        .await
        .unwrap();

    // 할일이 시계를 참조
    let todo = store
        .create(&schemas, "할일", obj(json!({ "내용": "배송대행 신청", "관련": [watch.id.clone()] })))
        .await
        .unwrap();

    // 역링크로 연결 확인, 삭제 보호 동작
    let back = store.backlinks(&watch.id).await.unwrap();
    assert_eq!(back[0].from_id, todo.id);
    assert!(store.delete(&watch.id).await.is_err());

    // 물건 family 조회에 시계가 포함된다
    let items = store.list(&schemas.family_of("물건")).await.unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].data["상태"], "주문됨");
}

#[test]
fn 실제_시드_스키마와_카테고리와_홈페이지가_로드된다() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let schemas = lifeops_core::schema::SchemaSet::load_dir(&root.join("schemas")).unwrap();
    for ty in ["노트", "물건", "시계", "할일", "계좌", "거래", "구독", "공간", "소모품", "활동"] {
        assert!(schemas.get(ty).is_some(), "시드 타입 누락: {ty}");
    }
    assert_eq!(schemas.get("시계").unwrap().category.as_deref(), Some("컬렉션")); // 상속
    assert!(schemas.get("할일").unwrap().behaviors.as_ref().unwrap().recurrence.is_some());

    let cats = lifeops_core::schema::load_categories(&root.join("categories.yaml")).unwrap();
    let names: Vec<&str> = cats.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["할일", "메모", "컬렉션", "재무", "환경", "취미"]);

    let pages = lifeops_core::view::PageSet::load_dir(&root.join("views")).unwrap();
    let home = pages.get("홈").expect("views/홈.yaml 필요");
    assert_eq!(home.blocks.len(), 3);
}
