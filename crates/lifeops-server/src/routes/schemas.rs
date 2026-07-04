use crate::error::ApiError;
use crate::state::AppState;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use indexmap::IndexMap;
use lifeops_core::error::SchemaError;
use lifeops_core::schema::{
    load_raw_dir, to_yaml, RawBehaviors, RawFieldDef, RawSchema, SchemaSet,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static SCHEMA_TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize)]
pub struct SchemaInput {
    #[serde(rename = "type", default)]
    pub name: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub extends: Option<String>,
    #[serde(default)]
    pub behaviors: Option<RawBehaviors>,
    #[serde(default)]
    pub fields: IndexMap<String, RawFieldDef>,
    #[serde(default)]
    pub field_order: Option<Vec<String>>,
    #[serde(default)]
    pub renames: IndexMap<String, String>,
}

pub async fn create(
    State(st): State<AppState>,
    Json(input): Json<SchemaInput>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let name = input
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .ok_or_else(|| {
            ApiError(
                StatusCode::BAD_REQUEST,
                json!({ "error": { "code": "bad_schema_name", "message": "type은 필수입니다" } }),
            )
        })?
        .to_string();
    let filename = safe_filename(&name)?;
    let raw = to_raw_schema(name.clone(), &input);
    apply_and_persist(&st, &name, &filename, raw).await?;

    Ok((StatusCode::CREATED, Json(json!({ "ok": true }))))
}

pub async fn get_one(
    State(_st): State<AppState>,
    Path(_name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(not_implemented())
}

pub async fn update(
    State(_st): State<AppState>,
    Path(_name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(not_implemented())
}

pub async fn delete(
    State(_st): State<AppState>,
    Path(_name): Path<String>,
) -> Result<Json<serde_json::Value>, ApiError> {
    Err(not_implemented())
}

fn not_implemented() -> ApiError {
    ApiError(
        StatusCode::BAD_REQUEST,
        json!({ "error": { "code": "not_implemented", "message": "아직 구현되지 않음" } }),
    )
}

fn to_raw_schema(name: String, input: &SchemaInput) -> RawSchema {
    let _ = &input.renames;
    RawSchema {
        name,
        extends: input.extends.clone(),
        category: input.category.clone(),
        behaviors: input.behaviors.clone(),
        fields: input.fields.clone(),
        field_order: input.field_order.clone(),
    }
}

fn safe_filename(name: &str) -> Result<String, ApiError> {
    if name.trim().is_empty() || name.contains('/') || name.contains('\\') || name.contains("..") {
        return Err(ApiError(
            StatusCode::BAD_REQUEST,
            json!({ "error": { "code": "bad_schema_name", "message": "안전하지 않은 타입명입니다" } }),
        ));
    }
    Ok(format!("{name}.yaml"))
}

fn schema_error_to_api(err: SchemaError) -> ApiError {
    match err {
        SchemaError::Io(_) | SchemaError::Parse { .. } | SchemaError::DuplicateType { .. } => {
            tracing::error!("스키마 로드 오류: {err}");
            ApiError(
                StatusCode::INTERNAL_SERVER_ERROR,
                json!({ "error": { "code": "schema_load", "message": "내부 서버 오류" } }),
            )
        }
        SchemaError::UnknownParent { .. }
        | SchemaError::Cycle { .. }
        | SchemaError::BadKind { .. }
        | SchemaError::EnumWithoutOptions { .. }
        | SchemaError::UnknownRefTarget { .. }
        | SchemaError::UnknownFieldInOrder { .. }
        | SchemaError::BadBehavior { .. } => ApiError(
            StatusCode::BAD_REQUEST,
            json!({ "error": { "code": "schema_validation", "message": err.to_string() } }),
        ),
    }
}

async fn apply_and_persist(
    st: &AppState,
    name: &str,
    filename: &str,
    raw: RawSchema,
) -> Result<(), ApiError> {
    let mut schemas = st.schemas.write().await;
    if schemas.get(name).is_some() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            json!({ "error": { "code": "schema_exists", "message": format!("이미 존재하는 타입입니다: {name}") } }),
        ));
    }

    let schemas_dir = st.schemas_dir.as_path();
    let mut candidates = load_raw_dir(schemas_dir).map_err(schema_error_to_api)?;
    if candidates.contains_key(name) || schemas_dir.join(filename).exists() {
        return Err(ApiError(
            StatusCode::CONFLICT,
            json!({ "error": { "code": "schema_exists", "message": format!("이미 존재하는 타입입니다: {name}") } }),
        ));
    }

    candidates.insert(name.to_string(), (raw.clone(), filename.to_string()));
    let new_set = SchemaSet::from_raw(&candidates).map_err(schema_error_to_api)?;
    let yaml = to_yaml(&raw).map_err(|err| {
        tracing::error!("스키마 직렬화 오류: {err}");
        ApiError(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": { "code": "schema_serialize", "message": "내부 서버 오류" } }),
        )
    })?;

    let tmp_path = schemas_dir.join(unique_tmp_filename(filename));
    let final_path = schemas_dir.join(filename);
    std::fs::write(&tmp_path, yaml).map_err(|err| io_api_error("schema_write", err))?;
    if let Err(err) = std::fs::rename(&tmp_path, &final_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return Err(io_api_error("schema_write", err));
    }

    *schemas = new_set;
    Ok(())
}

fn unique_tmp_filename(filename: &str) -> String {
    let seq = SCHEMA_TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{filename}.{}.{}.{}.tmp", std::process::id(), nanos, seq)
}

fn io_api_error(code: &str, err: std::io::Error) -> ApiError {
    tracing::error!("스키마 파일 쓰기 오류: {err}");
    ApiError(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": { "code": code, "message": "내부 서버 오류" } }),
    )
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
        let bytes = axum::body::to_bytes(res.into_body(), 1 << 20)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn post(uri: &str, body: Value) -> Request<Body> {
        Request::builder()
            .method("POST")
            .uri(uri)
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    #[tokio::test]
    async fn 새_타입_생성후_schemas에_반영된다() {
        let (state, dir) = test_state().await;
        let sdir = state.schemas_dir.clone();
        let app = build_app(state);
        let res = app
            .clone()
            .oneshot(post(
                "/api/schemas",
                json!({
                    "type": "북마크", "category": "메모",
                    "fields": { "제목": { "kind": "text", "required": true }, "링크": { "kind": "url" } }
                }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);

        assert!(sdir.join("북마크.yaml").exists());
        let body = body_json(
            app.oneshot(
                Request::builder()
                    .uri("/api/schemas")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert!(body["types"].get("북마크").is_some());
        assert_eq!(
            body["types"]["북마크"]["fields"]["제목"]["required"],
            json!(true)
        );
        drop(dir);
    }

    #[tokio::test]
    async fn 잘못된_kind는_400이고_파일을_남기지_않는다() {
        let (state, _dir) = test_state().await;
        let sdir = state.schemas_dir.clone();
        let app = build_app(state);
        let res = app
            .oneshot(post(
                "/api/schemas",
                json!({
                    "type": "깨진것", "fields": { "x": { "kind": "geo" } }
                }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert!(
            !sdir.join("깨진것.yaml").exists(),
            "실패 시 파일이 없어야 함"
        );
    }

    #[tokio::test]
    async fn 알수없는_ref_target은_400이고_파일을_남기지_않는다() {
        let (state, _dir) = test_state().await;
        let sdir = state.schemas_dir.clone();
        let app = build_app(state);
        let res = app
            .oneshot(post(
                "/api/schemas",
                json!({
                    "type": "방문", "fields": { "곳": { "kind": "ref", "target": "유령" } }
                }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        assert!(!sdir.join("방문.yaml").exists());
    }

    #[tokio::test]
    async fn 이미_있는_타입_생성은_409() {
        let (state, _dir) = test_state().await;
        let app = build_app(state);
        let res = app
            .oneshot(post(
                "/api/schemas",
                json!({
                    "type": "물건", "fields": { "이름": { "kind": "text" } }
                }),
            ))
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn 동시_다른_타입_생성은_둘다_schemas에_반영된다() {
        let (state, _dir) = test_state().await;
        let sdir = state.schemas_dir.clone();
        let app = build_app(state);

        let (one, two) = tokio::join!(
            app.clone().oneshot(post(
                "/api/schemas",
                json!({
                    "type": "책갈피", "fields": { "제목": { "kind": "text" } }
                }),
            )),
            app.clone().oneshot(post(
                "/api/schemas",
                json!({
                    "type": "레시피", "fields": { "이름": { "kind": "text" } }
                }),
            )),
        );

        let one = one.unwrap();
        let two = two.unwrap();
        assert_eq!(one.status(), StatusCode::CREATED);
        assert_eq!(two.status(), StatusCode::CREATED);
        assert!(sdir.join("책갈피.yaml").exists());
        assert!(sdir.join("레시피.yaml").exists());

        let body = body_json(
            app.oneshot(
                Request::builder()
                    .uri("/api/schemas")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
        )
        .await;
        assert!(body["types"].get("책갈피").is_some());
        assert!(body["types"].get("레시피").is_some());
    }

    #[tokio::test]
    async fn 동시_같은_타입_생성은_하나만_성공한다() {
        let (state, _dir) = test_state().await;
        let app = build_app(state);

        let (one, two) = tokio::join!(
            app.clone().oneshot(post(
                "/api/schemas",
                json!({
                    "type": "중복타입", "fields": { "제목": { "kind": "text" } }
                }),
            )),
            app.clone().oneshot(post(
                "/api/schemas",
                json!({
                    "type": "중복타입", "fields": { "제목": { "kind": "text" } }
                }),
            )),
        );

        let mut statuses = vec![one.unwrap().status(), two.unwrap().status()];
        statuses.sort();
        assert_eq!(statuses, [StatusCode::CREATED, StatusCode::CONFLICT]);
    }
}
