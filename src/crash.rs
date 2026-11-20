//! Journal de plantage : `%APPDATA%\<App>\crash.log` (date, version, thread, message, pile
//! d'appels), et signalement **au lancement suivant** (`pending`).
//!
//! Fusion des versions de l'explorateur (date + thread), de Vigie et de Tube.
//! Note : avec `panic = "abort"` (Vigie), le crochet tourne quand même avant l'arrêt.

use std::io::Write;
use std::path::PathBuf;

/// Fichier témoin créé à chaque plantage, effacé quand l'app l'a signalé.
fn flag(dir: &std::path::Path) -> PathBuf {
    dir.join("crash.new")
}

/// Installe le crochet de panique. `app` = nom du dossier (`Explorateur`, `Vigie`…),
/// `version` = `env!("CARGO_PKG_VERSION")`.
pub fn install(app: &'static str, version: &'static str) {
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let dir = crate::paths::config_dir(app);
        let _ = std::fs::create_dir_all(&dir);
        let bt = std::backtrace::Backtrace::force_capture();
        let thread = std::thread::current().name().unwrap_or("?").to_string();
        // Les décodeurs d'images du socle rattrapent leurs paniques (`catch_unwind`) : l'app ne
        // plante pas. On garde la trace dans le journal, sans la signaler comme un plantage.
        // Seulement à l'intérieur du `catch_unwind` (pas dans le rappel de l'app ni ailleurs dans
        // le thread), et jamais avec `panic = "abort"`.
        let caught = crate::panic_is_caught();
        let note = if caught { " · rattrapée, l'app a continué" } else { "" };
        let entry = format!("==== {} · v{version} · thread {thread}{note}\n{info}\n{bt}\n", stamp());
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(dir.join("crash.log")) {
            let _ = f.write_all(entry.as_bytes());
        }
        if caught {
            return;
        }
        let _ = std::fs::write(flag(&dir), info.to_string());
        default(info);
    }));
}

/// Au lancement : message du dernier plantage pas encore signalé (à montrer en toast), puis
/// oublie le témoin. `None` si rien à signaler.
pub fn pending(app: &str) -> Option<String> {
    let f = flag(&crate::paths::config_dir(app));
    let msg = std::fs::read_to_string(&f).ok()?;
    let _ = std::fs::remove_file(&f);
    let first = msg.lines().find(|l| !l.trim().is_empty()).unwrap_or("plantage").to_string();
    Some(first)
}

/// Chemin du journal (pour un bouton « Ouvrir le journal »).
pub fn log_path(app: &str) -> PathBuf {
    crate::paths::config_dir(app).join("crash.log")
}

/// Date locale approximative sans dépendance (UTC, format AAAA-MM-JJ HH:MM:SS).
fn stamp() -> String {
    let s = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (days, rem) = (s / 86_400, s % 86_400);
    // conversion jours → date civile (algorithme de Howard Hinnant)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!("{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02} UTC", rem / 3600, rem / 60 % 60, rem % 60)
}

#[cfg(test)]
mod tests {
    #[test]
    fn panique_rattrapee_pas_signalee() {
        assert!(!crate::panic_is_caught(), "hors catch_unwind : signalée");
        let r = crate::catch(|| crate::panic_is_caught());
        assert_eq!(r.ok(), Some(cfg!(panic = "unwind")), "dans catch_unwind : rattrapée");
        assert!(!crate::panic_is_caught(), "marque retirée après");
    }

    #[test]
    fn date() {
        let s = super::stamp();
        assert_eq!(s.len(), 23, "{s}");
        assert!(s.starts_with("20"));
    }
}
