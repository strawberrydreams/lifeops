use axum::response::{Html, IntoResponse, Response};
use std::path::Path;

/// frontend/dist 가 있으면 index.html(SPA), 없으면 안내 페이지.
/// (Task 9 스캐폴드: 계획 3에서 실제 정적 파일 서빙으로 확장)
pub async fn spa_fallback() -> Response {
    let index = Path::new("frontend/dist/index.html");
    if let Ok(html) = tokio::fs::read_to_string(index).await {
        Html(html).into_response()
    } else {
        Html("<!doctype html><meta charset=utf-8><h1>LifeOps</h1><p>프론트엔드(계획 3) 빌드가 아직 없습니다. API는 <code>/api/*</code>에서 동작합니다.</p>").into_response()
    }
}
