use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Map, Value};
use std::collections::HashMap;

/// GET /api/entities?type=물건&<필드>=<값>...
pub async fn list(
    State(st): State<AppState>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let ty = params.get("type").cloned().unwrap_or_default();
    let schemas = st.schemas.read().await;
    // type이 있으면 family 확장, 없으면 전체 타입
    let types: Vec<String> = if ty.is_empty() {
        schemas.names().iter().map(|s| s.to_string()).collect()
    } else {
        schemas.family_of(&ty)
    };
    let mut entities = st.store.list(&types).await?;
    // type 외의 쿼리 파라미터는 eq 필터(문자열 일치)
    for (k, v) in &params {
        if k == "type" || k == "sort" {
            continue;
        }
        entities.retain(|e| e.data.get(k).and_then(Value::as_str) == Some(v.as_str()));
    }
    Ok(Json(json!(entities)))
}

/// POST /api/entities  { "type": "...", "data": { ... } }
pub async fn create(
    State(st): State<AppState>,
    Json(payload): Json<Value>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let ty = payload.get("type").and_then(Value::as_str).unwrap_or("").to_string();
    let data: Map<String, Value> = payload
        .get("data")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let schemas = st.schemas.read().await;
    let entity = st.store.create(&schemas, &ty, data).await?;
    Ok((StatusCode::CREATED, Json(json!(entity))))
}

/// GET /api/entities/:id → { entity, backlinks }
pub async fn get_one(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let entity = st.store.get(&id).await?;
    let Some(entity) = entity else {
        return Err(ApiError::from(lifeops_core::error::CoreError::NotFound(id)));
    };
    let backlinks = st.store.backlinks(&id).await?;
    let backlinks: Vec<_> = backlinks
        .iter()
        .map(|r| json!({ "from_id": r.from_id, "from_type": r.from_type, "field_name": r.field_name }))
        .collect();
    Ok(Json(json!({ "entity": entity, "backlinks": backlinks })))
}

/// PATCH /api/entities/:id  { <필드>: <값>, ... }
pub async fn update(
    State(st): State<AppState>,
    Path(id): Path<String>,
    Json(patch): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let patch: Map<String, Value> = patch.as_object().cloned().unwrap_or_default();
    let schemas = st.schemas.read().await;
    let entity = st.store.update(&schemas, &id, patch).await?;
    Ok(Json(json!(entity)))
}

/// DELETE /api/entities/:id
pub async fn delete(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    st.store.delete(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use crate::app::build_app;
    use crate::state::test_state;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use serde_json::{json, Value};
    use tower::ServiceExt;

    async fn body_json(res: axum::response::Response) -> Value {
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn post(uri: &str, body: Value) -> Request<Body> {
        Request::builder().method("POST").uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string())).unwrap()
    }

    #[tokio::test]
    async fn 생성_조회_수정_목록_삭제_왕복() {
        let (state, _dir) = test_state().await;
        let app = build_app(state);

        // 생성
        let res = app.clone().oneshot(post("/api/entities",
            json!({ "type": "시계", "data": { "이름": "세이코 미쿠", "상태": "위시" } }))).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let created = body_json(res).await;
        let id = created["id"].as_str().unwrap().to_string();
        assert_eq!(created["type"], "시계");

        // 단건 조회 + 역링크 필드 존재
        let res = app.clone().oneshot(
            Request::builder().uri(format!("/api/entities/{id}")).body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let got = body_json(res).await;
        assert_eq!(got["entity"]["data"]["이름"], "세이코 미쿠");
        assert!(got["backlinks"].is_array());

        // PATCH
        let res = app.clone().oneshot(
            Request::builder().method("PATCH").uri(format!("/api/entities/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "상태": "주문됨" }).to_string())).unwrap()
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(body_json(res).await["data"]["상태"], "주문됨");

        // 목록(type=물건 → 시계 포함)
        let res = app.clone().oneshot(
            Request::builder().uri("/api/entities?type=물건").body(Body::empty()).unwrap()
        ).await.unwrap();
        let list = body_json(res).await;
        assert_eq!(list.as_array().unwrap().len(), 1);

        // 삭제
        let res = app.clone().oneshot(
            Request::builder().method("DELETE").uri(format!("/api/entities/{id}")).body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::NO_CONTENT);
    }

    #[tokio::test]
    async fn 검증_실패는_400_필드에러() {
        let (state, _dir) = test_state().await;
        let app = build_app(state);
        let res = app.oneshot(post("/api/entities",
            json!({ "type": "시계", "data": { "상태": "위시" } }))).await.unwrap(); // 이름(필수) 누락
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        let body = body_json(res).await;
        assert_eq!(body["error"]["code"], "validation");
        assert!(body["error"]["fields"].as_array().unwrap().iter().any(|f| f["field"] == "이름"));
    }

    #[tokio::test]
    async fn 삭제_차단은_409() {
        let (state, _dir) = test_state().await;
        let app = build_app(state);
        let w = body_json(app.clone().oneshot(post("/api/entities",
            json!({ "type": "시계", "data": { "이름": "미쿠" } }))).await.unwrap()).await;
        let wid = w["id"].as_str().unwrap();
        app.clone().oneshot(post("/api/entities",
            json!({ "type": "할일", "data": { "내용": "개봉", "관련물건": [wid] } }))).await.unwrap();
        let res = app.oneshot(
            Request::builder().method("DELETE").uri(format!("/api/entities/{wid}")).body(Body::empty()).unwrap()
        ).await.unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }
}
