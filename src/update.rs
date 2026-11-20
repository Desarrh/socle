//! Mise à jour depuis GitHub Releases (fusion de `explorateur/src/updater.rs` et
//! `lecteur-universel/src/update.rs`, même mécanisme) :
//! - au plus une fois par 24 h (`due`), en arrière-plan : `curl.exe` (fourni avec Windows 10/11)
//!   interroge l'API ; aucune pile réseau dans l'exe ;
//! - si `tag_name` est plus récent que la version compilée → bandeau « Installer » ;
//! - installation : zip dans %TEMP%, exe extrait, l'exe en cours est renommé `.old` (Windows
//!   l'autorise), le nouveau prend sa place, relance avec `--apres-maj`. Le `.old` est supprimé
//!   au démarrage suivant (`cleanup_old`).
//!
//! Drapeau de relance unifié : `--apres-maj`. `is_after_update()` reconnaît aussi l'ancien
//! `--after-update` de l'explorateur (une ancienne version installée relance la nouvelle avec).

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

const NO_WINDOW: u32 = 0x0800_0000;
pub const RELAUNCH_FLAG: &str = "--apres-maj";

/// Paramètres propres à chaque app.
#[derive(Clone, Debug)]
pub struct Updater {
    /// `Desarrh/explorateur`
    pub repo: &'static str,
    /// Version compilée : `env!("CARGO_PKG_VERSION")`.
    pub current: &'static str,
    /// Fin du nom de l'asset zip, ex. `windows-x64.zip`.
    pub asset_suffix: &'static str,
    /// Nom de l'exe dans le zip (`None` = le premier `.exe`).
    pub exe_name: Option<&'static str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// Version sans le `v` (`0.3.1`).
    pub version: String,
    pub tag: String,
    pub url: String,
    /// Page de la release (« nouveautés »).
    pub page: String,
    pub notes: String,
    /// Empreinte SHA-256 de l'asset (champ `digest` de l'API GitHub), en hexadécimal.
    pub sha256: String,
}

/// Taille maximale de l'exe extrait du zip.
const MAX_EXE: u64 = 500 * 1024 * 1024;

/// `curl.exe` de Windows par son chemin complet (jamais un `curl.exe` posé dans le dossier
/// courant ou le PATH), HTTPS obligatoire, redirections comprises.
fn curl() -> Command {
    let root = std::env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
    let mut c = Command::new(root.join("System32").join("curl.exe"));
    c.args(["--proto", "=https", "--proto-redir", "=https"]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(NO_WINDOW);
    }
    c
}

fn parse_ver(s: &str) -> Option<(u32, u32, u32)> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    let mut it = s.split(['.', '-']).map(|x| x.parse::<u32>().ok());
    Some((it.next()??, it.next().flatten().unwrap_or(0), it.next().flatten().unwrap_or(0)))
}

/// `v0.3.0` est-il plus récent que `0.2.9` ?
pub fn is_newer(tag: &str, current: &str) -> bool {
    matches!((parse_ver(tag), parse_ver(current)), (Some(a), Some(b)) if a > b)
}

/// Faut-il vérifier ? (dernière vérification il y a plus de 24 h ; secondes Unix, 0 = jamais).
pub fn due(last_check: u64) -> bool {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    now.saturating_sub(last_check) >= 24 * 3600
}

/// Relancé par une mise à jour ? (l'ancienne instance est en train de se fermer)
pub fn is_after_update() -> bool {
    std::env::args().any(|a| a == RELAUNCH_FLAG || a == "--after-update")
}

impl Updater {
    /// Interroge GitHub (bloquant, ~0,3–1 s : à appeler sur un thread). `Ok(None)` = à jour.
    pub fn check(&self) -> Result<Option<Release>, String> {
        let ua = format!("User-Agent: {}-updater", self.repo.rsplit('/').next().unwrap_or("socle"));
        let out = curl()
            .args(["-sL", "--max-time", "15", "-H", &ua, "-H", "Accept: application/vnd.github+json", "--"])
            .arg(format!("https://api.github.com/repos/{}/releases/latest", self.repo))
            .output()
            .map_err(|e| format!("curl introuvable : {e}"))?;
        if !out.status.success() {
            return Err("pas de connexion".into());
        }
        self.parse(&out.stdout)
    }

    /// Lit la réponse JSON de l'API (séparé pour les tests).
    pub fn parse(&self, json: &[u8]) -> Result<Option<Release>, String> {
        let v: serde_json::Value = serde_json::from_slice(json).map_err(|_| "réponse GitHub illisible".to_string())?;
        let Some(tag) = v.get("tag_name").and_then(|t| t.as_str()) else {
            return Err(v.get("message").and_then(|m| m.as_str()).unwrap_or("aucune release").to_string());
        };
        if !is_newer(tag, self.current) {
            return Ok(None);
        }
        let assets = v.get("assets").and_then(|a| a.as_array()).cloned().unwrap_or_default();
        let name = |a: &serde_json::Value| a.get("name").and_then(|n| n.as_str()).map(str::to_lowercase);
        let suffix = self.asset_suffix.to_lowercase();
        let asset = assets
            .iter()
            .find(|a| name(a).is_some_and(|n| n.ends_with(&suffix)))
            .or_else(|| assets.iter().find(|a| name(a).is_some_and(|n| n.ends_with(".zip"))))
            .ok_or("aucun zip Windows dans la release")?;
        let url = asset.get("browser_download_url").and_then(|u| u.as_str()).ok_or("asset sans adresse")?.to_string();
        // Seulement depuis GitHub, en HTTPS.
        if !url.starts_with("https://github.com/") {
            return Err(format!("adresse de téléchargement refusée : {url}"));
        }
        let sha256 = asset
            .get("digest")
            .and_then(|d| d.as_str())
            .and_then(|d| d.strip_prefix("sha256:"))
            .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or("release sans empreinte SHA-256 : mise à jour refusée")?
            .to_ascii_lowercase();
        Ok(Some(Release {
            version: tag.trim_start_matches(['v', 'V']).to_string(),
            tag: tag.to_string(),
            url,
            page: v.get("html_url").and_then(|b| b.as_str()).unwrap_or("").to_string(),
            notes: v.get("body").and_then(|b| b.as_str()).unwrap_or("").to_string(),
            sha256,
        }))
    }

    /// Télécharge, remplace l'exe en cours et renvoie son chemin (à relancer avec `relaunch`,
    /// puis fermer l'app). `progress` reçoit 0..1. À appeler sur un thread.
    pub fn install(&self, r: &Release, progress: &dyn Fn(f32)) -> Result<PathBuf, String> {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        // Dossier nommé par le PID seulement : la version vient du réseau, elle n'entre pas dans un chemin.
        let tmp = std::env::temp_dir().join(format!("socle-maj-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
        let zip_path = tmp.join("release.zip");
        progress(0.05);
        let st = curl()
            .args(["-sL", "--fail", "--max-time", "600", "-H", "User-Agent: socle-updater", "-o"])
            .arg(&zip_path)
            .arg("--")
            .arg(&r.url)
            .status()
            .map_err(|e| e.to_string())?;
        if !st.success() {
            return Err("téléchargement échoué".into());
        }
        progress(0.6);
        let got = sha256_file(&zip_path)?;
        if got != r.sha256 {
            let _ = std::fs::remove_dir_all(&tmp);
            return Err("empreinte SHA-256 différente : téléchargement corrompu ou modifié, mise à jour annulée".into());
        }
        progress(0.7);
        let new_exe = extract_exe(&zip_path, &tmp, self.exe_name)?;
        progress(0.9);
        replace_exe(&exe, &new_exe)?;
        let _ = std::fs::remove_dir_all(&tmp);
        progress(1.0);
        Ok(exe)
    }
}

/// SHA-256 d'un fichier, en hexadécimal minuscule.
fn sha256_file(p: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut f = std::fs::File::open(p).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    std::io::copy(&mut f, &mut h).map_err(|e| e.to_string())?;
    Ok(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

fn extract_exe(zip_path: &Path, dir: &Path, want: Option<&str>) -> Result<PathBuf, String> {
    let f = std::fs::File::open(zip_path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("zip invalide : {e}"))?;
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = e.enclosed_name() else { continue };
        let Some(file) = name.file_name().map(|s| s.to_string_lossy().into_owned()) else { continue };
        let ok = match want {
            Some(w) => file.eq_ignore_ascii_case(w),
            None => file.to_lowercase().ends_with(".exe"),
        };
        if ok {
            if e.size() > MAX_EXE {
                return Err("exe du zip trop gros".into());
            }
            let out = dir.join(&file);
            let mut o = std::fs::File::create(&out).map_err(|e| e.to_string())?;
            // Taille réelle bornée aussi (la taille annoncée peut mentir).
            let n = std::io::copy(&mut (&mut e).take(MAX_EXE + 1), &mut o).map_err(|e| e.to_string())?;
            drop(o);
            if n > MAX_EXE {
                return Err("exe du zip trop gros".into());
            }
            let mut head = [0u8; 2];
            let ok_mz = std::fs::File::open(&out).and_then(|mut f| f.read_exact(&mut head)).is_ok() && &head == b"MZ";
            if !ok_mz {
                return Err("le fichier du zip n'est pas un exe Windows".into());
            }
            return Ok(out);
        }
    }
    Err(format!("{} absent du zip", want.unwrap_or("exe")))
}

/// Renomme l'exe en cours en `.old`, copie le nouveau à sa place ; remet l'ancien en cas d'échec.
fn replace_exe(exe: &Path, new_exe: &Path) -> Result<(), String> {
    let old = old_path(exe);
    let _ = std::fs::remove_file(&old);
    std::fs::rename(exe, &old).map_err(|e| format!("impossible de renommer l'exe : {e}"))?;
    if let Err(e) = std::fs::copy(new_exe, exe) {
        let _ = std::fs::rename(&old, exe);
        return Err(format!("impossible d'installer : {e}"));
    }
    Ok(())
}

fn old_path(exe: &Path) -> PathBuf {
    let mut s = exe.as_os_str().to_owned();
    s.push(".old");
    PathBuf::from(s)
}

/// Au démarrage : supprime l'exe remplacé lors d'une mise à jour précédente (les deux
/// conventions : `app.exe.old` du lecteur et `app.old` de l'explorateur).
pub fn cleanup_old() {
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::fs::remove_file(old_path(&exe));
        let _ = std::fs::remove_file(exe.with_extension("old"));
    }
}

/// Relance le nouvel exe avec `--apres-maj` (il attend ~0,8 s que l'ancien se ferme).
pub fn relaunch(exe: &Path) -> Result<(), String> {
    Command::new(exe).arg(RELAUNCH_FLAG).spawn().map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert!(is_newer("v0.3.0", "0.2.9"));
        assert!(is_newer("1.0", "0.9.9"));
        assert!(!is_newer("v0.2.0", "0.2.0"));
        assert!(!is_newer("nimporte", "0.1.0"));
        assert!(due(0));
    }

    #[test]
    fn lecture_release() {
        let u = Updater { repo: "Desarrh/explorateur", current: "0.2.0", asset_suffix: "windows-x64.zip", exe_name: None };
        let j = br#"{"tag_name":"v0.3.0","html_url":"https://x","body":"notes","assets":[
            {"name":"src.tar.gz","browser_download_url":"a"},
            {"name":"Explorateur-v0.3.0-windows-x64.zip","browser_download_url":"https://github.com/Desarrh/explorateur/releases/download/v0.3.0/E.zip","digest":"sha256:A9B5CA862645937F1EDBEAFB375DBCAF53FD7DD0976ACDB072B8A16446FE8F33"}]}"#;
        let r = u.parse(j).unwrap().unwrap();
        assert_eq!((r.version.as_str(), r.page.as_str()), ("0.3.0", "https://x"));
        assert_eq!(r.url, "https://github.com/Desarrh/explorateur/releases/download/v0.3.0/E.zip");
        assert_eq!(r.sha256, "a9b5ca862645937f1edbeafb375dbcaf53fd7dd0976acdb072b8a16446fe8f33");
        // adresse hors de GitHub : refusée
        let hors = br#"{"tag_name":"v0.3.0","assets":[{"name":"a-windows-x64.zip","browser_download_url":"http://exemple.test/a.zip","digest":"sha256:a9b5ca862645937f1edbeafb375dbcaf53fd7dd0976acdb072b8a16446fe8f33"}]}"#;
        assert!(u.parse(hors).is_err());
        // pas d'empreinte : refusée
        let sans = br#"{"tag_name":"v0.3.0","assets":[{"name":"a-windows-x64.zip","browser_download_url":"https://github.com/a/b/a.zip"}]}"#;
        assert!(u.parse(sans).unwrap_err().contains("SHA-256"));
        assert_eq!(u.parse(br#"{"tag_name":"v0.1.0","assets":[]}"#).unwrap(), None);
        assert!(u.parse(br#"{"message":"Not Found"}"#).is_err());
    }

    #[test]
    fn empreinte_et_extraction() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("socle-maj-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // SHA-256 de « abc » (vecteur de test FIPS 180-2)
        let f = dir.join("abc.txt");
        std::fs::write(&f, "abc").unwrap();
        assert_eq!(sha256_file(&f).unwrap(), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        // zip avec un « exe » qui n'en est pas un : refusé ; avec un en-tête MZ : accepté
        let make = |name: &str, body: &[u8]| {
            let p = dir.join(name);
            let mut z = zip::ZipWriter::new(std::fs::File::create(&p).unwrap());
            z.start_file("app.exe", zip::write::SimpleFileOptions::default()).unwrap();
            z.write_all(body).unwrap();
            z.finish().unwrap();
            p
        };
        let faux = make("faux.zip", b"#!/bin/sh");
        assert!(extract_exe(&faux, &dir, Some("app.exe")).unwrap_err().contains("pas un exe"));
        let vrai = make("vrai.zip", b"MZ\x90\0reste");
        assert!(extract_exe(&vrai, &dir, Some("app.exe")).is_ok());
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn remplacement_et_retour_arriere() {
        let dir = std::env::temp_dir().join(format!("socle-maj-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let exe = dir.join("app.exe");
        let new = dir.join("new.exe");
        std::fs::write(&exe, "ancien").unwrap();
        std::fs::write(&new, "nouveau").unwrap();
        replace_exe(&exe, &new).unwrap();
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "nouveau");
        assert_eq!(std::fs::read_to_string(old_path(&exe)).unwrap(), "ancien");
        // nouvel exe introuvable : l'ancien est remis
        std::fs::remove_file(old_path(&exe)).unwrap();
        assert!(replace_exe(&exe, &dir.join("absent.exe")).is_err());
        assert_eq!(std::fs::read_to_string(&exe).unwrap(), "nouveau");
        let _ = std::fs::remove_dir_all(dir);
    }
}
