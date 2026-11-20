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
use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ALREADY_EXISTS, ERROR_IO_PENDING, ERROR_PIPE_CONNECTED, GetLastError, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE, FILE_FLAG_OVERLAPPED, PIPE_ACCESS_INBOUND, ReadFile};
use windows_sys::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
use windows_sys::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, DisconnectNamedPipe, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT, WaitNamedPipeW,
};
use windows_sys::Win32::System::RemoteDesktop::ProcessIdToSessionId;
use windows_sys::Win32::System::Threading::{CreateEventW, CreateMutexW, GetCurrentProcessId, INFINITE, WaitForSingleObject};

const MAGIC: &str = "SOCLE1";
/// Une instance suivante a ce délai pour écrire son message ; au-delà, on la lâche
/// (un client qui se connecte sans rien écrire ne bloque plus l'écoute).
const READ_TIMEOUT_MS: u32 = 1000;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(Some(0)).collect()
}

fn session_id() -> u32 {
    let mut sid = 0u32;
    unsafe { ProcessIdToSessionId(GetCurrentProcessId(), &mut sid) };
    sid
}

fn names(app: &str) -> (String, String) {
    // Les tubes sont globaux à la machine : on y met l'utilisateur et le numéro de session pour
    // ne pas se croiser entre sessions. Le mutex `Local\` est déjà propre à la session.
    let user = std::env::var("USERNAME").unwrap_or_default().replace(['\\', '/', ' '], "_");
    (format!("Local\\{app}.Socle.Instance"), format!(r"\\.\pipe\{app}-{user}-s{}-socle-v2", session_id()))
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

/// Relancé par une mise à jour (`--apres-maj`, ou l'ancien `--after-update`) ?
fn after_update() -> bool {
    std::env::args().any(|a| a == "--apres-maj" || a == "--retour-maj" || a == "--after-update")
}

/// Détermine le rôle de ce processus et transmet `args` si une instance tourne déjà.
/// Coût : ~0,1 ms en instance principale ; une attente seulement si l'autre instance
/// est en train de démarrer (3 s au plus). Après une mise à jour, l'ancienne instance est en train de se
/// fermer : on réessaie de prendre le mutex pendant 3 s au plus avant de lui transmettre quoi
/// que ce soit.
pub fn start(app: &str, args: &[String]) -> Start {
    let (mname, pipe) = names(app);
    let w = wide(&mname);
    let deadline = Instant::now() + if after_update() { Duration::from_secs(3) } else { Duration::ZERO };
    loop {
        let h = unsafe { CreateMutexW(std::ptr::null(), 0, w.as_ptr()) };
        let exists = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
        if h.is_null() || !exists {
            return Start::Primary(Instance { mutex: h, pipe });
        }
        unsafe { CloseHandle(h) };
        if Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    if forward(&pipe, args, Duration::from_secs(3)) { Start::Forwarded } else { Start::Alone }
}

/// Envoie les arguments à l'instance qui écoute sur `pipe`.
fn forward(pipe: &str, args: &[String], timeout: Duration) -> bool {
    use std::os::windows::fs::OpenOptionsExt;
    // SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION : le serveur du tube ne peut pas se
    // faire passer pour nous (pas d'usurpation au-delà de l'identification).
    const SQOS_IDENTIFICATION: u32 = 0x0010_0000 | (1 << 16);
    let t0 = Instant::now();
    let mut msg = String::from(MAGIC);
    for a in args {
        msg.push('\0');
        msg.push_str(a);
    }
    let wp = wide(pipe);
    while t0.elapsed() < timeout {
        match std::fs::OpenOptions::new().write(true).security_qos_flags(SQOS_IDENTIFICATION).open(pipe) {
            Ok(mut f) => {
                // autorise l'instance principale (et elle seule) à passer au premier plan
                use std::os::windows::io::AsRawHandle;
                let mut pid = 0u32;
                unsafe {
                    if windows_sys::Win32::System::Pipes::GetNamedPipeServerProcessId(f.as_raw_handle() as HANDLE, &mut pid) != 0 && pid != 0 {
                        windows_sys::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(pid);
                    }
                }
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

/// Attend la fin d'une opération asynchrone ; `None` si le délai expire (l'opération est annulée).
unsafe fn finish(h: HANDLE, ov: &mut OVERLAPPED, timeout_ms: u32) -> Option<u32> {
    unsafe {
        if WaitForSingleObject(ov.hEvent, timeout_ms) != WAIT_OBJECT_0 {
            CancelIoEx(h, ov);
            let mut n = 0;
            GetOverlappedResult(h, ov, &mut n, 1); // attend la fin de l'annulation
            return None;
        }
        let mut n = 0u32;
        (GetOverlappedResult(h, ov, &mut n, 0) != 0).then_some(n)
    }
}

/// Lit un message (au plus 1 Mo, `READ_TIMEOUT_MS` au total) sur un tube connecté.
unsafe fn read_message(h: HANDLE, event: HANDLE) -> Vec<u8> {
    let t0 = Instant::now();
    let mut data = Vec::new();
    let mut buf = [0u8; 4096];
    loop {
        let left = READ_TIMEOUT_MS.saturating_sub(t0.elapsed().as_millis() as u32);
        if left == 0 || data.len() > 1 << 20 {
            break;
        }
        let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
        ov.hEvent = event;
        let mut n = 0u32;
        let r = unsafe { ReadFile(h, buf.as_mut_ptr(), buf.len() as u32, &mut n, &mut ov) };
        let got = if r != 0 {
            Some(n)
        } else if unsafe { GetLastError() } == ERROR_IO_PENDING {
            unsafe { finish(h, &mut ov, left) }
        } else {
            None // fin du message (client fermé) ou erreur
        };
        match got {
            Some(n) if n > 0 => data.extend_from_slice(&buf[..n as usize]),
            _ => break,
        }
    }
    data
}

impl Instance {
    /// Lance le thread d'écoute (il dort tant que personne n'écrit : 0 % CPU).
    /// `on_msg` reçoit les arguments de chaque instance suivante (vide = « au premier plan »).
    pub fn listen(self, on_msg: impl Fn(Vec<String>) + Send + 'static) {
        let pipe = self.pipe.clone();
        // Le mutex appartient au thread d'écoute : il vit tant que l'écoute tourne. Si elle
        // s'arrête, il est relâché, et le lancement suivant devient l'instance principale au lieu
        // d'attendre 3 s une réponse qui ne viendra pas.
        let mutex = self.mutex as isize;
        std::mem::forget(self);
        let _ = std::thread::Builder::new().name("socle-instance".into()).spawn(move || {
            struct Release(isize);
            impl Drop for Release {
                fn drop(&mut self) {
                    if self.0 != 0 {
                        unsafe { CloseHandle(self.0 as HANDLE) };
                    }
                }
            }
            let _mutex = Release(mutex);
            let w = wide(&pipe);
            let event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if event.is_null() {
                return;
            }
            let mut fails = 0u32;
            loop {
                // FILE_FLAG_FIRST_PIPE_INSTANCE : si un autre processus a déjà pris ce nom (tube
                // squatté), la création échoue et on n'écoute pas, plutôt que de partager le tube.
                let h = unsafe {
                    CreateNamedPipeW(
                        w.as_ptr(),
                        PIPE_ACCESS_INBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE | FILE_FLAG_OVERLAPPED,
                        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
                        PIPE_UNLIMITED_INSTANCES,
                        0,
                        65536,
                        0,
                        std::ptr::null(),
                    )
                };
                if h == INVALID_HANDLE_VALUE {
                    // Le nom est libre entre deux clients : un échec passager est réessayé
                    // (1 s au total) avant d'abandonner l'écoute.
                    fails += 1;
                    if fails > 20 {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                    continue;
                }
                fails = 0;
                // Attente d'un client : sans délai (le thread dort), en asynchrone pour pouvoir
                // borner la lecture ensuite.
                let mut ov: OVERLAPPED = unsafe { std::mem::zeroed() };
                ov.hEvent = event;
                let connected = unsafe {
                    if ConnectNamedPipe(h, &mut ov) != 0 {
                        true
                    } else {
                        match GetLastError() {
                            ERROR_PIPE_CONNECTED => true,
                            ERROR_IO_PENDING => finish(h, &mut ov, INFINITE).is_some(),
                            _ => false,
                        }
                    }
                };
                if connected {
                    let data = unsafe { read_message(h, event) };
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
        // un client qui se connecte sans rien écrire ne bloque l'écoute que 1 s au plus
        let (_, pipe) = names(&app);
        let t0 = Instant::now();
        let muet = loop {
            match std::fs::OpenOptions::new().write(true).open(&pipe) {
                Ok(f) => break f,
                Err(_) if t0.elapsed() < Duration::from_secs(2) => std::thread::sleep(Duration::from_millis(10)),
                Err(e) => panic!("connexion : {e}"),
            }
        };
        let t = Instant::now();
        assert!(matches!(start(&app, &["après".to_string()]), Start::Forwarded));
        let got = rx.recv_timeout(Duration::from_secs(5)).expect("message reçu malgré le client muet");
        assert_eq!(got, vec!["après".to_string()]);
        assert!(t.elapsed() < Duration::from_millis(3500), "{:?}", t.elapsed());
        drop(muet);
    }
}
