//! Instance unique, **un seul mécanisme** pour toutes les apps : mutex nommé (détection) +
//! tube nommé (transmission des arguments).
//!
//! Pourquoi celui-là parmi les trois existants :
//! - la boîte aux lettres de fichiers de l'explorateur se sonde toutes les 250 ms (CPU non nul
//!   au repos, latence jusqu'à 250 ms) ;
//! - FindWindow (Vigie, Tube) ramène la fenêtre mais ne transmet aucun argument ;
//! - le tube nommé (lecteur) ne coûte rien au repos (le thread dort dans `ConnectNamedPipe`)
//!   et livre le message en < 1 ms. On y ajoute le mutex pour couvrir la fenêtre de course au
//!   démarrage (1ʳᵉ instance lancée mais tube pas encore ouvert).
//!
//! ```ignore
//! match socle::instance::start("Explorateur", &args) {
//!     Start::Forwarded => return,                       // l'autre fenêtre a reçu les arguments
//!     Start::Primary(inst) => inst.listen(move |args| { /* canal + request_repaint */ }),
//!     Start::Alone => {}                                // impossible de joindre l'autre : on démarre quand même
//! }
//! ```
//! Le thread d'écoute appelle `on_msg` hors du fil de l'interface : envoyer dans un canal puis
//! `ctx.request_repaint()`, et appeler `focus_window()` pour passer au premier plan (la 2ᵉ
//! instance a déjà autorisé ce passage avec `AllowSetForegroundWindow`).

use std::io::Write;
use std::time::{Duration, Instant};
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{PIPE_ACCESS_INBOUND, ReadFile};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, WaitNamedPipeW,
};
use windows_sys::Win32::System::Threading::CreateMutexW;

const MAGIC: &str = "SOCLE1";

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn names(app: &str) -> (String, String) {
    // Les tubes sont globaux à la machine : on y met l'utilisateur pour ne pas se croiser
    // entre sessions. Le mutex `Local\` est déjà propre à la session.
    let user = std::env::var("USERNAME").unwrap_or_default().replace(['\\', '/', ' '], "_");
    (format!("Local\\{app}.Socle.Instance"), format!(r"\\.\pipe\{app}-{user}-socle-v1"))
}

/// Instance principale : garde le mutex et peut écouter les suivantes.
pub struct Instance {
    mutex: HANDLE,
    pipe: String,
}

// Le HANDLE n'est qu'un entier côté noyau.
unsafe impl Send for Instance {}

pub enum Start {
    /// Première instance : appeler `listen`.
    Primary(Instance),
    /// Une instance tournait : elle a reçu les arguments ; quitter.
    Forwarded,
    /// Une instance existe mais ne répond pas (plantée, bloquée) : démarrer normalement.
    Alone,
}

/// Détermine le rôle de ce processus et transmet `args` si une instance tourne déjà.
/// Coût : ~0,1 ms en instance principale ; jusqu'à 2 s d'attente seulement si l'autre instance
/// est en train de démarrer.
pub fn start(app: &str, args: &[String]) -> Start {
    let (mname, pipe) = names(app);
    let w = wide(&mname);
    let h = unsafe { CreateMutexW(std::ptr::null(), 0, w.as_ptr()) };
    let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    if h.is_null() || !exists {
        return Start::Primary(Instance { mutex: h, pipe });
    }
    unsafe { CloseHandle(h) };
    if forward(&pipe, args, Duration::from_secs(2)) { Start::Forwarded } else { Start::Alone }
}

/// Envoie les arguments à l'instance qui écoute sur `pipe`.
fn forward(pipe: &str, args: &[String], timeout: Duration) -> bool {
    let t0 = Instant::now();
    let mut msg = String::from(MAGIC);
    for a in args {
        msg.push('\0');
        msg.push_str(a);
    }
    let wp = wide(pipe);
    while t0.elapsed() < timeout {
        match std::fs::OpenOptions::new().write(true).open(pipe) {
            Ok(mut f) => {
                // autorise l'instance principale à passer au premier plan
                unsafe { windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(u32::MAX) };
                return f.write_all(msg.as_bytes()).is_ok();
            }
            Err(e) if e.raw_os_error() == Some(231) => unsafe {
                // ERROR_PIPE_BUSY : une autre écriture est en cours
                WaitNamedPipeW(wp.as_ptr(), 200);
            },
            Err(_) => std::thread::sleep(Duration::from_millis(40)), // tube pas encore créé
        }
    }
    false
}

/// Décode un message reçu (`None` s'il ne vient pas du socle).
fn decode(data: &[u8]) -> Option<Vec<String>> {
    let s = String::from_utf8_lossy(data);
    let mut it = s.split('\0');
    (it.next()? == MAGIC).then(|| it.map(String::from).collect())
}

impl Instance {
    /// Lance le thread d'écoute (il dort tant que personne n'écrit : 0 % CPU).
    /// `on_msg` reçoit les arguments de chaque instance suivante (vide = « au premier plan »).
    pub fn listen(self, on_msg: impl Fn(Vec<String>) + Send + 'static) {
        let pipe = self.pipe.clone();
        std::mem::forget(self); // le mutex vit jusqu'à la fin du processus
        let _ = std::thread::Builder::new().name("socle-instance".into()).spawn(move || {
            let w = wide(&pipe);
            loop {
                let h = unsafe {
                    CreateNamedPipeW(w.as_ptr(), PIPE_ACCESS_INBOUND, PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, PIPE_UNLIMITED_INSTANCES, 0, 65536, 0, std::ptr::null())
                };
                if h == INVALID_HANDLE_VALUE {
                    return;
                }
                let ok = unsafe { ConnectNamedPipe(h, std::ptr::null_mut()) } != 0 || unsafe { GetLastError() } == ERROR_PIPE_CONNECTED;
                if ok {
                    let mut data = Vec::new();
                    let mut buf = [0u8; 4096];
                    loop {
                        let mut n = 0u32;
                        let r = unsafe { ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut n, std::ptr::null_mut()) };
                        if r == 0 || n == 0 {
                            break;
                        }
                        data.extend_from_slice(&buf[..n as usize]);
                        if data.len() > 1 << 20 {
                            break; // 1 Mo : message aberrant
                        }
                    }
                    if let Some(args) = decode(&data) {
                        on_msg(args);
                    }
                }
                unsafe {
                    DisconnectNamedPipe(h);
                    CloseHandle(h);
                }
            }
        });
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.mutex.is_null() {
            unsafe { CloseHandle(self.mutex) };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn transmission_entre_instances() {
        let app = format!("SocleTest{}", std::process::id());
        let Start::Primary(inst) = start(&app, &[]) else { panic!("1ʳᵉ instance attendue") };
        let (tx, rx) = mpsc::channel();
        inst.listen(move |a| {
            let _ = tx.send(a);
        });
        let args = vec!["C:\\Dev".to_string(), "--onglet".to_string(), String::new()];
        let t = Instant::now();
        assert!(matches!(start(&app, &args), Start::Forwarded));
        let got = rx.recv_timeout(Duration::from_secs(3)).expect("message reçu");
        assert_eq!(got, args);
        assert!(t.elapsed() < Duration::from_millis(500), "transmission en {:?}", t.elapsed());
        assert_eq!(decode(b"autre chose"), None);
    }
}
