//! Réglages JSON : `%APPDATA%\<App>\settings.json`, `#[serde(default)]` côté app (un champ
//! ajouté ou retiré ne casse jamais la lecture), écriture atomique (`.tmp` + renommage).
//!
//! ```ignore
//! #[derive(serde::Serialize, serde::Deserialize, Default)]
//! #[serde(default)]
//! struct Settings { dark: bool }
//! let s: Settings = socle::settings::load("Explorateur");
//! socle::settings::save("Explorateur", &s)?;
//! ```

use serde::{Serialize, de::DeserializeOwned};
use std::path::{Path, PathBuf};

/// `%APPDATA%\<app>\settings.json`.
pub fn path(app: &str) -> PathBuf {
    crate::paths::config_dir(app).join("settings.json")
}

/// Charge les réglages ; fichier absent ou illisible → `T::default()`.
/// Un fichier illisible est gardé à côté (`settings.json.bad`) pour ne rien perdre.
pub fn load<T: DeserializeOwned + Default>(app: &str) -> T {
    load_from(&path(app))
}

pub fn load_from<T: DeserializeOwned + Default>(p: &Path) -> T {
    let Ok(bytes) = std::fs::read(p) else { return T::default() };
    match serde_json::from_slice(&bytes) {
        Ok(v) => v,
        Err(_) => {
            let mut bad = p.as_os_str().to_owned();
            bad.push(".bad");
            let _ = std::fs::copy(p, PathBuf::from(bad));
            T::default()
        }
    }
}

/// Enregistre les réglages (JSON indenté, écriture atomique).
pub fn save<T: Serialize>(app: &str, value: &T) -> Result<(), String> {
    save_to(&path(app), value)
}

pub fn save_to<T: Serialize>(p: &Path, value: &T) -> Result<(), String> {
    let json = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    crate::paths::write_atomic(p, &json)
}

/// Enregistrement différé : appelez `touch()` à chaque changement et `tick()` à chaque image ;
/// l'écriture a lieu au plus une fois par `delay`, sur un thread (l'interface ne bloque jamais).
pub struct Saver {
    delay: std::time::Duration,
    dirty: Option<std::time::Instant>,
}

impl Saver {
    pub fn new(delay: std::time::Duration) -> Self {
        Saver { delay, dirty: None }
    }
    pub fn touch(&mut self) {
        self.dirty.get_or_insert_with(std::time::Instant::now);
    }
    /// Si l'échéance est passée, sérialise maintenant et écrit sur un thread. Renvoie le temps
    /// restant avant la prochaine échéance (à passer à `request_repaint_after`).
    pub fn tick<T: Serialize>(&mut self, app: &str, value: &T) -> Option<std::time::Duration> {
        let t = self.dirty?;
        let el = t.elapsed();
        if el < self.delay {
            return Some(self.delay - el);
        }
        self.dirty = None;
        if let Ok(json) = serde_json::to_vec_pretty(value) {
            let p = path(app);
            std::thread::spawn(move || {
                let _ = crate::paths::write_atomic(&p, &json);
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize, serde::Deserialize, Default, PartialEq, Debug)]
    #[serde(default)]
    struct S {
        dark: bool,
        n: u32,
    }

    #[test]
    fn aller_retour_et_fichier_abime() {
        let dir = std::env::temp_dir().join(format!("socle-settings-{}", std::process::id()));
        let p = dir.join("settings.json");
        save_to(&p, &S { dark: true, n: 3 }).unwrap();
        assert_eq!(load_from::<S>(&p), S { dark: true, n: 3 });
        assert!(!p.with_extension("json.tmp").exists());
        // champ inconnu / manquant : tolérés
        std::fs::write(&p, r#"{"dark":true,"ancien":1}"#).unwrap();
        assert_eq!(load_from::<S>(&p), S { dark: true, n: 0 });
        // fichier abîmé : défaut + copie .bad
        std::fs::write(&p, "{pas du json").unwrap();
        assert_eq!(load_from::<S>(&p), S::default());
        assert!(dir.join("settings.json.bad").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
