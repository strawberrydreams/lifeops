use crate::backup::{backup_dir_accessible, create_snapshot_with_meta, load_last_success};
use crate::config::{load_config, save_config, AppConfig, BindScope};
use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::net::{IpAddr, Ipv4Addr};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

static CONFIG_WRITE_LOCK: Mutex<()> = Mutex::new(());

/// GET /api/system/info → 데이터 경로·포트·LAN 접속 주소·현재 실효 바인드 범위.
pub async fn info(State(state): State<AppState>) -> Json<Value> {
    let port = state.bound_addr.port();
    let bind_scope = if state.bound_addr.ip().is_unspecified() {
        BindScope::Lan
    } else {
        BindScope::Localhost
    };
    Json(json!({
        "data_dir": state.data_dir.display().to_string(),
        "port": port,
        "lan_addrs": lan_addresses(port),
        "bind_scope": bind_scope,
    }))
}

#[derive(serde::Deserialize)]
pub struct ConfigPatch {
    pub bind_scope: Option<BindScope>,
    #[serde(default, deserialize_with = "double_option")]
    pub backup_dir: Option<Option<PathBuf>>,
    pub backup_keep: Option<usize>,
}

fn double_option<'de, D>(deserializer: D) -> Result<Option<Option<PathBuf>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(deserializer).map(Some)
}

/// GET /api/system/config → 저장된 다음 시작 설정(없거나 손상되면 기본값).
pub async fn config_get(State(state): State<AppState>) -> Json<AppConfig> {
    Json(load_config(&state.data_dir))
}

/// PUT /api/system/config → 제공된 필드만 원자적으로 갱신한다.
pub async fn config_put(
    State(state): State<AppState>,
    Json(patch): Json<ConfigPatch>,
) -> Result<Json<AppConfig>, (StatusCode, Json<Value>)> {
    let _guard = CONFIG_WRITE_LOCK.lock().map_err(|_| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "config",
            "설정 저장 잠금을 사용할 수 없습니다",
        )
    })?;
    let mut config = load_config(&state.data_dir);
    if let Some(scope) = patch.bind_scope {
        config.bind_scope = scope;
    }
    if let Some(dir) = patch.backup_dir {
        config.backup_dir = dir;
    }
    if let Some(keep) = patch.backup_keep {
        if keep == 0 {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "invalid",
                "보존 개수는 1 이상이어야 합니다",
            ));
        }
        config.backup_keep = keep;
    }
    save_config(&state.data_dir, &config)
        .map_err(|error| api_error(StatusCode::INTERNAL_SERVER_ERROR, "io", &error.to_string()))?;
    Ok(Json(config))
}

#[derive(serde::Serialize)]
pub struct SnapshotMeta {
    pub name: String,
    pub created_at: String,
    pub size: u64,
}

#[derive(serde::Serialize)]
pub struct BackupsList {
    pub backup_dir: String,
    pub accessible: bool,
    pub last_success: Option<String>,
    pub snapshots: Vec<SnapshotMeta>,
}

/// POST /api/system/backup → 현재 설정 위치에 즉시 zip 스냅샷을 만든다.
pub async fn backup_create(
    State(state): State<AppState>,
) -> Result<Json<SnapshotMeta>, (StatusCode, Json<Value>)> {
    let config = load_config(&state.data_dir);
    let paths = crate::resolve_paths(&state.data_dir);
    let backup_dir = config.resolved_backup_dir(&state.data_dir);
    let created = create_snapshot_with_meta(&paths, &backup_dir, config.backup_keep)
        .await
        .map_err(|error| {
            let status = match error.kind() {
                std::io::ErrorKind::NotFound
                | std::io::ErrorKind::PermissionDenied
                | std::io::ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            api_error(status, "backup", &error.to_string())
        })?;
    Ok(Json(SnapshotMeta {
        name: created.name,
        created_at: created.created_at,
        size: created.size,
    }))
}

/// GET /api/system/backups → 대상 폴더의 스냅샷을 최신순으로 반환한다.
pub async fn backups_list(State(state): State<AppState>) -> Json<BackupsList> {
    let config = load_config(&state.data_dir);
    let backup_dir = config.resolved_backup_dir(&state.data_dir);
    let mut snapshots = Vec::new();
    let accessible = backup_dir_accessible(&backup_dir);
    if let Ok(entries) = std::fs::read_dir(&backup_dir) {
        snapshots = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .map(|entry| entry.path())
            .filter(|path| is_snapshot_name(path))
            .filter_map(|path| snapshot_meta(&path))
            .collect();
        snapshots.sort_by(|a, b| b.name.cmp(&a.name));
    }
    Json(BackupsList {
        backup_dir: backup_dir.display().to_string(),
        accessible,
        last_success: load_last_success(&state.data_dir),
        snapshots,
    })
}

fn is_snapshot_name(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with("lifeops-") && name.ends_with(".zip"))
}

fn snapshot_meta(path: &Path) -> Option<SnapshotMeta> {
    let metadata = std::fs::metadata(path).ok()?;
    let created_at = metadata
        .modified()
        .ok()
        .map(|time| chrono::DateTime::<chrono::Local>::from(time).to_rfc3339())
        .unwrap_or_default();
    Some(SnapshotMeta {
        name: path.file_name()?.to_str()?.to_owned(),
        created_at,
        size: metadata.len(),
    })
}

fn api_error(status: StatusCode, code: &str, message: &str) -> (StatusCode, Json<Value>) {
    (
        status,
        Json(json!({ "error": { "code": code, "message": message } })),
    )
}

/// 현재 호스트의 사설 IPv4 인터페이스를 중복 없는 결정적 URL 목록으로 만든다.
pub fn lan_addresses(port: u16) -> Vec<String> {
    let interfaces = local_ip_address::list_afinet_netifas().unwrap_or_default();
    lan_addresses_from(port, interfaces.into_iter().map(|(_, ip)| ip))
}

fn lan_addresses_from(port: u16, addresses: impl IntoIterator<Item = IpAddr>) -> Vec<String> {
    addresses
        .into_iter()
        .filter_map(|ip| match ip {
            IpAddr::V4(ip) if is_lan_ipv4(ip) => Some(ip),
            _ => None,
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|ip| format!("http://{ip}:{port}"))
        .collect()
}

fn is_lan_ipv4(ip: Ipv4Addr) -> bool {
    ip.is_private() && !ip.is_loopback()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::build_app;
    use crate::state::test_state;
    use axum::body::{to_bytes, Body};
    use axum::http::{Request, StatusCode};
    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    use tower::ServiceExt;

    #[tokio::test]
    async fn system_info_형태와_state_값() {
        let (state, _dir) = test_state().await;
        let expected_data_dir = state.data_dir.display().to_string();
        let expected_port = state.bound_addr.port();
        let app = build_app(state);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/api/system/info")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["data_dir"], expected_data_dir);
        assert_eq!(value["port"], expected_port);
        assert!(value["lan_addrs"].is_array());
        assert_eq!(value["bind_scope"], "localhost");
    }

    #[tokio::test]
    async fn config_get은_기본값_localhost() {
        let (state, _dir) = test_state().await;
        let response = build_app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/system/config")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["bind_scope"], "localhost");
        assert_eq!(value["backup_keep"], 7);
    }

    #[tokio::test]
    async fn config_put은_부분갱신과_null_초기화를_저장한다() {
        let (state, _dir) = test_state().await;
        let app = build_app(state.clone());
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/system/config")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"bind_scope":"lan","backup_dir":"/tmp/b","backup_keep":3}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = app
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/system/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"backup_dir":null}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let value: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["bind_scope"], "lan");
        assert_eq!(value["backup_dir"], Value::Null);
        assert_eq!(value["backup_keep"], 3);
        assert_eq!(load_config(&state.data_dir).bind_scope, BindScope::Lan);
    }

    #[tokio::test]
    async fn config_put_keep_0은_거부하고_기존값을_보존한다() {
        let (state, _dir) = test_state().await;
        let response = build_app(state.clone())
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri("/api/system/config")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"backup_keep":0}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(load_config(&state.data_dir), AppConfig::default());
    }

    async fn state_with_db() -> (AppState, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let paths = crate::resolve_paths(dir.path());
        crate::install_seed_if_empty(&paths).unwrap();
        lifeops_core::entity::EntityStore::open(&paths.db_path)
            .await
            .unwrap();
        let run_config = crate::RunConfig {
            data_dir: dir.path().to_path_buf(),
            bind_addr: "127.0.0.1".parse().unwrap(),
            port: 0,
        };
        let state = crate::build_state(&run_config, "127.0.0.1:0".parse().unwrap())
            .await
            .unwrap();
        (state, dir)
    }

    #[tokio::test]
    async fn backup_생성하면_zip_메타를_반환하고_목록에_뜬다() {
        let (state, _dir) = state_with_db().await;
        let created = build_app(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/backup")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        let body = to_bytes(created.into_body(), 1 << 20).await.unwrap();
        let meta: Value = serde_json::from_slice(&body).unwrap();
        assert!(meta["name"].as_str().unwrap().ends_with(".zip"));
        assert!(meta["size"].as_u64().unwrap() > 0);

        let listed = build_app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/system/backups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(listed.into_body(), 1 << 20).await.unwrap();
        let list: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list["accessible"], true);
        assert!(list["last_success"].as_str().is_some());
        assert_eq!(list["snapshots"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn keep_1_동시_backup도_둘다_성공하고_최신_하나만_유지() {
        let (state, _dir) = state_with_db().await;
        save_config(
            &state.data_dir,
            &AppConfig {
                backup_keep: 1,
                ..Default::default()
            },
        )
        .unwrap();
        let app = build_app(state.clone());
        let request = || {
            Request::builder()
                .method("POST")
                .uri("/api/system/backup")
                .body(Body::empty())
                .unwrap()
        };

        let (first, second) = tokio::join!(
            app.clone().oneshot(request()),
            app.clone().oneshot(request())
        );
        let first = first.unwrap();
        let second = second.unwrap();
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        let first_body = to_bytes(first.into_body(), 1 << 20).await.unwrap();
        let second_body = to_bytes(second.into_body(), 1 << 20).await.unwrap();
        let first_meta: Value = serde_json::from_slice(&first_body).unwrap();
        let second_meta: Value = serde_json::from_slice(&second_body).unwrap();
        assert_ne!(first_meta["name"], second_meta["name"]);
        assert!(first_meta["size"].as_u64().is_some_and(|size| size > 0));
        assert!(second_meta["size"].as_u64().is_some_and(|size| size > 0));

        let listed = app
            .oneshot(
                Request::builder()
                    .uri("/api/system/backups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(listed.into_body(), 1 << 20).await.unwrap();
        let list: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list["snapshots"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn backups_목록은_폴더_없어도_안전하다() {
        let (state, _dir) = test_state().await;
        let response = build_app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/system/backups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let list: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list["accessible"], false);
        assert_eq!(list["last_success"], Value::Null);
        assert_eq!(list["snapshots"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn backup_dir가_사라져도_last_success는_data_dir에서_반환() {
        let (state, _dir) = state_with_db().await;
        let external = state.data_dir.join("external-backups");
        save_config(
            &state.data_dir,
            &AppConfig {
                backup_dir: Some(external.clone()),
                ..Default::default()
            },
        )
        .unwrap();
        let created = build_app(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/backup")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::OK);
        std::fs::remove_dir_all(&external).unwrap();

        let response = build_app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/system/backups")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let body = to_bytes(response.into_body(), 1 << 20).await.unwrap();
        let list: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(list["accessible"], false);
        assert!(list["last_success"].as_str().is_some());
        assert!(list["snapshots"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn backup_dir가_파일이면_수동_backup은_400() {
        let (state, _dir) = state_with_db().await;
        let file = state.data_dir.join("not-a-directory");
        std::fs::write(&file, b"file").unwrap();
        save_config(
            &state.data_dir,
            &AppConfig {
                backup_dir: Some(file),
                ..Default::default()
            },
        )
        .unwrap();

        let response = build_app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/system/backup")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn system_info_추가가_기존_health_route를_가리지_않는다() {
        let (state, _dir) = test_state().await;
        let res = build_app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(res.status(), StatusCode::OK);
        let body = to_bytes(res.into_body(), 1 << 20).await.unwrap();
        assert_eq!(&body[..], b"ok");
    }

    #[test]
    fn lan_주소는_사설_ipv4만_중복없이_결정적_순서로_만든다() {
        let interfaces = [
            ("z", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))),
            ("loopback", IpAddr::V4(Ipv4Addr::LOCALHOST)),
            ("a", IpAddr::V4(Ipv4Addr::new(10, 0, 0, 2))),
            ("duplicate", IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))),
            ("public", IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))),
            ("ipv6", IpAddr::V6(Ipv6Addr::LOCALHOST)),
        ];

        assert_eq!(
            lan_addresses_from(3000, interfaces.into_iter().map(|(_, ip)| ip)),
            vec!["http://10.0.0.2:3000", "http://192.168.1.20:3000",]
        );
    }
}
