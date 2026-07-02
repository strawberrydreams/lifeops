use std::path::{Path, PathBuf};
use std::time::Duration;

/// data/lifeops.db → backups/lifeops-YYYYMMDD-HHMMSS.db 복사 후 keep일 초과분 삭제.
/// 반환: 만들어진 스냅샷 경로.
pub fn run_backup_once(db: &Path, backups: &Path, keep: usize) -> std::io::Result<PathBuf> {
    if !db.exists() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("백업할 DB 없음: {}", db.display()),
        ));
    }
    std::fs::create_dir_all(backups)?;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let dest = backups.join(format!("lifeops-{stamp}.db"));
    std::fs::copy(db, &dest)?;
    prune_backups(backups, keep)?;
    Ok(dest)
}

/// 이름순(=시간순) 정렬 후 최신 keep개만 남기고 삭제.
pub fn prune_backups(backups: &Path, keep: usize) -> std::io::Result<()> {
    let mut snaps: Vec<PathBuf> = std::fs::read_dir(backups)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("lifeops-") && n.ends_with(".db"))
        })
        .collect();
    snaps.sort();
    if snaps.len() > keep {
        for old in &snaps[..snaps.len() - keep] {
            let _ = std::fs::remove_file(old);
        }
    }
    Ok(())
}

/// 24시간 간격으로 백업하는 백그라운드 태스크. 실패는 로그만.
pub fn spawn_daily_backup(db: PathBuf, backups: PathBuf, keep: usize) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(24 * 60 * 60));
        loop {
            ticker.tick().await;
            match run_backup_once(&db, &backups, keep) {
                Ok(p) => tracing::info!("백업 생성: {}", p.display()),
                Err(e) => tracing::error!("백업 실패: {e}"),
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 백업_복사하고_keep_초과분_삭제() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("lifeops.db");
        std::fs::write(&db, b"DB0").unwrap();
        let backups = dir.path().join("backups");

        // 오래된 스냅샷 5개를 미리 만들어 둔다(이름순=시간순 가정)
        std::fs::create_dir_all(&backups).unwrap();
        for i in 0..5 {
            std::fs::write(backups.join(format!("lifeops-2026010{i}-000000.db")), b"old").unwrap();
        }
        // keep=3으로 새 백업 → 총 6개 중 최신 3개만 남아야
        let made = run_backup_once(&db, &backups, 3).unwrap();
        assert!(made.exists());
        let mut names: Vec<_> = std::fs::read_dir(&backups).unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().to_string()).collect();
        names.sort();
        assert_eq!(names.len(), 3, "keep=3인데 {names:?}");
        // 가장 최신(새로 만든 것)이 포함
        assert!(names.iter().any(|n| n == made.file_name().unwrap().to_string_lossy().as_ref()));
    }

    #[test]
    fn db_없으면_에러() {
        let dir = tempfile::tempdir().unwrap();
        let r = run_backup_once(&dir.path().join("none.db"), &dir.path().join("b"), 3);
        assert!(r.is_err());
    }
}
