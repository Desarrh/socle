//! Dossiers communs (DIRECTION.md §4) :
//! - réglages, `crash.log` : `%APPDATA%\<App>\` ;
//! - caches et données lourdes : `%LOCALAPPDATA%\<App>\`.
//!
//! Lus depuis les variables d'environnement (aucune dépendance) ; repli sur le dossier courant.

use std::path::PathBuf;

fn env_dir(var: &str) -> PathBuf {
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// `%APPDATA%\<app>` (réglages, crash.log). Non créé.
pub fn config_dir(app: &str) -> PathBuf {
    env_dir("APPDATA").join(app)
}

/// `%LOCALAPPDATA%\<app>` (caches, index, miniatures). Non créé.
pub fn cache_dir(app: &str) -> PathBuf {
    env_dir("LOCALAPPDATA").join(app)
}

/// Écriture atomique : `<fichier>.<pid>.<n>.tmp` puis renommage (un fichier n'est jamais à moitié écrit,
/// même si l'app plante pendant l'écriture). Crée le dossier parent au besoin.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("dossier {} : {e}", dir.display()))?;
    }
    // `.tmp` unique (PID + compteur) : deux écritures simultanées (deux threads, deux
    // instances) ne se marchent jamais dessus.
    static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(format!(".{}.{n}.tmp", std::process::id()));
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, bytes).map_err(|e| format!("écriture de {} : {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("remplacement de {} : {e}", path.display())
    })
}
