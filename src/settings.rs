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
/// l'écriture a lieu sur un thread (l'interface ne bloque jamais). Deux réglages :
/// - [`Saver::new`]`(delay)` : au plus une écriture par `delay` après le premier changement ;
/// - [`Saver::debounced`]`(quiet, max_wait)` : **au fil de l'eau**. On écrit dès que les
///   changements s'arrêtent pendant `quiet` (ex. 300 ms), et au plus tard `max_wait` après le
///   premier changement non écrit (ex. 2 s), même si l'utilisateur ne s'arrête pas.
///
/// À la fermeture de l'app, appeler `flush` : il écrit ce qui reste et attend l'écriture en
/// cours (sinon le thread pourrait être tué avant la fin et la dernière modification perdue).
pub struct Saver {
    /// Silence après le dernier changement avant d'écrire.
    quiet: std::time::Duration,
    /// Attente maximale après le premier changement non écrit.
    max_wait: std::time::Duration,
    /// (premier changement non écrit, dernier changement)
    dirty: Option<(std::time::Instant, std::time::Instant)>,
    writing: Option<std::thread::JoinHandle<Result<(), String>>>,
    /// Dernière erreur d'écriture, à montrer en toast (`take_error`).
    error: Option<String>,
}

impl Saver {
    /// Écrit `delay` après le premier changement (comportement d'avant la v0.1.6).
    pub fn new(delay: std::time::Duration) -> Self {
        Self::debounced(delay, delay)
    }

    /// Écrit au fil de l'eau : après `quiet` sans changement, ou au plus tard `max_wait` après
    /// le premier changement non écrit.
    pub fn debounced(quiet: std::time::Duration, max_wait: std::time::Duration) -> Self {
        Saver { quiet, max_wait: max_wait.max(quiet), dirty: None, writing: None, error: None }
    }

    pub fn touch(&mut self) {
        let now = std::time::Instant::now();
        let first = self.dirty.map_or(now, |d| d.0);
        self.dirty = Some((first, now));
    }

    /// Temps restant avant l'écriture (`None` = rien à écrire, `Some(0)` = maintenant).
    fn due_in(&self, now: std::time::Instant) -> Option<std::time::Duration> {
        let (first, last) = self.dirty?;
        let a = (last + self.quiet).saturating_duration_since(now);
        let b = (first + self.max_wait).saturating_duration_since(now);
        Some(a.min(b))
    }
    /// Si l'échéance est passée, sérialise maintenant et écrit sur un thread. Renvoie le temps
    /// restant avant la prochaine échéance (à passer à `request_repaint_after`).
    pub fn tick<T: Serialize>(&mut self, app: &str, value: &T) -> Option<std::time::Duration> {
        if self.writing.as_ref().is_some_and(|h| h.is_finished()) {
            self.collect();
        }
        let left = self.due_in(std::time::Instant::now())?;
        if !left.is_zero() {
            return Some(left);
        }
        self.dirty = None;
        if let Ok(json) = serde_json::to_vec_pretty(value) {
            let p = path(app);
            // une écriture à la fois, dans l'ordre : on attend la précédente (déjà finie en pratique)
            self.collect();
            self.writing = Some(std::thread::spawn(move || crate::paths::write_atomic(&p, &json)));
        }
        None
    }

    /// Écrit tout de suite ce qui reste à écrire et attend l'écriture en cours (fermeture).
    pub fn flush<T: Serialize>(&mut self, app: &str, value: &T) -> Result<(), String> {
        self.collect();
        if self.dirty.take().is_some() {
            save(app, value)?;
        }
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }

    /// Erreur de la dernière écriture en arrière-plan (disque plein, droits…), une seule fois.
    pub fn take_error(&mut self) -> Option<String> {
        if self.writing.as_ref().is_some_and(|h| h.is_finished()) {
            self.collect();
        }
        self.error.take()
    }

    /// Attend l'écriture en cours et garde son erreur éventuelle.
    fn collect(&mut self) {
        if let Some(h) = self.writing.take() {
            match h.join() {
                Ok(Err(e)) => self.error = Some(e),
                Err(_) => self.error = Some("écriture des réglages interrompue".into()),
                Ok(Ok(())) => {}
            }
        }
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

    /// Au fil de l'eau : on attend la fin de la frappe (`quiet`), sans dépasser `max_wait`.
    #[test]
    fn ecriture_au_fil_de_l_eau() {
        use std::time::{Duration as D, Instant};
        let mut s = Saver::debounced(D::from_millis(300), D::from_secs(2));
        let t0 = Instant::now();
        assert_eq!(s.due_in(t0), None);
        s.dirty = Some((t0, t0));
        assert_eq!(s.due_in(t0), Some(D::from_millis(300)));
        // frappe continue : chaque changement repousse, mais jamais au-delà de 2 s
        s.dirty = Some((t0, t0 + D::from_millis(1900)));
        assert_eq!(s.due_in(t0 + D::from_millis(1900)), Some(D::from_millis(100)));
        assert_eq!(s.due_in(t0 + D::from_millis(2000)), Some(D::ZERO));
        // comportement historique : `delay` après le premier changement
        let mut h = Saver::new(D::from_secs(1));
        h.dirty = Some((t0, t0 + D::from_millis(900)));
        assert_eq!(h.due_in(t0 + D::from_millis(900)), Some(D::from_millis(100)));
        // touch : le premier changement reste la référence de max_wait
        let mut t = Saver::debounced(D::from_millis(300), D::from_secs(2));
        t.touch();
        let first = t.dirty.unwrap().0;
        t.touch();
        assert_eq!(t.dirty.unwrap().0, first);
    }

    #[test]
    fn aller_retour_et_fichier_abime() {
        let dir = std::env::temp_dir().join(format!("socle-settings-{}", std::process::id()));
        let p = dir.join("settings.json");
        save_to(&p, &S { dark: true, n: 3 }).unwrap();
        assert_eq!(load_from::<S>(&p), S { dark: true, n: 3 });
        assert!(std::fs::read_dir(&dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().ends_with(".tmp")), "aucun .tmp laissé");
        // écritures simultanées : jamais de fichier partiel ni d'erreur de renommage
        let hs: Vec<_> = (0..8).map(|i| { let p = p.clone(); std::thread::spawn(move || save_to(&p, &S { dark: false, n: i })) }).collect();
        for h in hs {
            h.join().unwrap().unwrap();
        }
        assert!(load_from::<S>(&p).n < 8);
        // erreur d'écriture remontée par flush (dossier parent = un fichier)
        let mut sv = Saver::new(std::time::Duration::ZERO);
        sv.touch();
        let bloque = dir.join("fichier");
        std::fs::write(&bloque, "x").unwrap();
        assert!(save_to(&bloque.join("settings.json"), &S::default()).is_err());
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
