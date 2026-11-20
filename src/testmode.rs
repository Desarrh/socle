//! **Mode test** (règle n° 0 de DIRECTION.md) : pendant les tests, l'utilisateur doit pouvoir se
//! servir de son PC comme si de rien n'était. Ce module donne aux apps les briques communes :
//!
//! - [`enabled`] : mode test demandé par `--test-hors-ecran` ou la variable `SOCLE_TEST=1` ;
//! - [`hide`] : fenêtre à l'écran mais **invisible** (opacité 0), traversée par la souris,
//!   sans activation, hors de la barre des tâches, tout au fond de la pile ; elle continue à se
//!   dessiner normalement ;
//! - [`capture`] : capture de la fenêtre par `PrintWindow` (`PW_RENDERFULLCONTENT`), même
//!   invisible, sans toucher à l'écran ni au presse-papiers ;
//! - [`foreground`] : pour vérifier qu'un test n'a pas pris le focus ;
//! - [`prepare_input`] : fenêtre jamais au premier plan, donc egui croit ne pas avoir le focus ;
//!   en mode test, on force `RawInput.focused` pour que les champs gardent le focus ;
//! - [`low_priority`] : processus en priorité basse (tests, compilations lancées par une app).
//!
//! Aucune entrée simulée n'est fournie, volontairement.
//!
//! Pourquoi pas hors écran (v0.1.1) : Windows ne demande plus de dessin à une fenêtre placée à
//! (-32000, -32000) ; avec winit/egui l'app se fige et `PrintWindow` rend du blanc.
//! Avec winit, créer la fenêtre **invisible et sans focus**, puis appeler `hide` (qui l'affiche) :
//! ```ignore
//! let test = socle::testmode::enabled();
//! let attrs = Window::default_attributes().with_active(!test).with_visible(!test);
//! // … juste après la création : if test { socle::testmode::hide(hwnd) }
//! ```
//! Pour une simple capture d'interface, le rendu **sans fenêtre** reste préférable (voir le mode
//! `--capture` de `examples/demo.rs` : egui + rendu CPU dans un tampon, enregistré en PNG).

use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleBitmap, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, GetDIBits, ReleaseDC, SelectObject,
};
use windows_sys::Win32::Storage::Xps::{PRINT_WINDOW_FLAGS, PrintWindow};
use windows_sys::Win32::System::Threading::{BELOW_NORMAL_PRIORITY_CLASS, GetCurrentProcess, SetPriorityClass};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, HWND_BOTTOM, LWA_ALPHA, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetLayeredWindowAttributes, SetWindowLongPtrW, SetWindowPos, ShowWindow, WS_EX_APPWINDOW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

/// Drapeau de ligne de commande du mode test.
pub const FLAG: &str = "--test-hors-ecran";

/// Mode test demandé ? (`--test-hors-ecran` ou `SOCLE_TEST=1`)
pub fn enabled() -> bool {
    std::env::args().any(|a| a == FLAG) || std::env::var("SOCLE_TEST").is_ok_and(|v| v == "1")
}

/// Rend la fenêtre **invisible sans la sortir de l'écran**, et l'affiche sans activation :
/// opacité 0 (`WS_EX_LAYERED`), la souris la traverse (`WS_EX_TRANSPARENT`), jamais de focus
/// (`WS_EX_NOACTIVATE`), hors de la barre des tâches (`WS_EX_TOOLWINDOW`), tout au fond de la
/// pile (`HWND_BOTTOM`). Windows continue à lui demander de se dessiner, donc l'app tourne
/// normalement et [`capture`] voit l'interface (méthode de Tube, vérifiée dans le socle).
/// `hwnd` : handle Win32 (winit : `RawWindowHandle::Win32(h) => h.hwnd.get()`).
pub fn hide(hwnd: isize) {
    let h = hwnd as HWND;
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let ex = (ex | (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize) & !(WS_EX_APPWINDOW as isize);
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex);
        SetLayeredWindowAttributes(h, 0, 0, LWA_ALPHA);
        SetWindowPos(h, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOMOVE);
        ShowWindow(h, SW_SHOWNOACTIVATE);
    }
}

/// À appeler sur l'entrée egui de chaque image, juste après `take_egui_input`, quand le mode
/// test est actif : une fenêtre `hide` n'est jamais au premier plan (`WS_EX_NOACTIVATE`), donc
/// egui-winit transmet `focused = false` et aucun champ ne garde le focus (`has_focus` exige
/// que la fenêtre l'ait). On force `focused = true` pour que les tests scriptés puissent taper
/// dans un champ. Sans effet hors mode test.
pub fn prepare_input(raw: &mut egui::RawInput) {
    if enabled() {
        force_focus(raw);
    }
}

/// Force `focused = true` sans condition (tests qui savent déjà qu'ils sont en mode test).
pub fn force_focus(raw: &mut egui::RawInput) {
    raw.focused = true;
}

/// Fenêtre au premier plan (pour vérifier dans un test que le focus de l'utilisateur n'a pas bougé).
pub fn foreground() -> isize {
    unsafe { GetForegroundWindow() as isize }
}

/// Capture le contenu de la fenêtre (même hors écran ou cachée derrière d'autres) en RGBA.
/// Renvoie (largeur, hauteur, pixels). `None` si la fenêtre n'existe pas ou n'a pas de taille.
pub fn capture(hwnd: isize) -> Option<(u32, u32, Vec<u8>)> {
    let h = hwnd as HWND;
    unsafe {
        let mut r = std::mem::zeroed();
        if GetWindowRect(h, &mut r) == 0 {
            return None;
        }
        let (w, ht) = (r.right - r.left, r.bottom - r.top);
        if w <= 0 || ht <= 0 {
            return None;
        }
        let screen = GetDC(std::ptr::null_mut());
        let dc = CreateCompatibleDC(screen);
        let bmp = CreateCompatibleBitmap(screen, w, ht);
        let old = SelectObject(dc, bmp);
        // PW_RENDERFULLCONTENT (2) : capture aussi le contenu dessiné par DWM / DirectComposition.
        // Repli sans ce drapeau pour les fenêtres que DWM ne compose pas (jamais affichées).
        let ok = PrintWindow(h, dc, 2 as PRINT_WINDOW_FLAGS) != 0 || PrintWindow(h, dc, 0) != 0;
        let mut info: BITMAPINFO = std::mem::zeroed();
        info.bmiHeader = BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w,
            biHeight: -ht, // de haut en bas
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            ..std::mem::zeroed()
        };
        let mut px = vec![0u8; (w * ht * 4) as usize];
        // GetDIBits exige que le bitmap ne soit plus sélectionné dans un DC.
        SelectObject(dc, old);
        let lines = GetDIBits(dc, bmp, 0, ht as u32, px.as_mut_ptr().cast(), &mut info, DIB_RGB_COLORS);
        DeleteObject(bmp);
        DeleteDC(dc);
        ReleaseDC(std::ptr::null_mut(), screen);
        if !ok || lines == 0 {
            return None;
        }
        // BGRA → RGBA, alpha opaque (GDI ne le remplit pas)
        for p in px.chunks_exact_mut(4) {
            p.swap(0, 2);
            p[3] = 255;
        }
        Some((w as u32, ht as u32, px))
    }
}

/// Passe le processus courant en priorité « inférieure à la normale » : un test ou une
/// compilation ne ralentit pas ce que l'utilisateur fait.
pub fn low_priority() {
    unsafe {
        SetPriorityClass(GetCurrentProcess(), BELOW_NORMAL_PRIORITY_CLASS);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WS_POPUP};

    /// Fenêtre de test invisible (opacité 0), sans activation, hors de la barre des tâches :
    /// rien n'apparaît à l'écran et le focus de l'utilisateur ne bouge pas.
    /// Fenêtre jamais au premier plan : sans `force_focus`, un champ perd le focus ; avec, il le garde.
    #[test]
    fn focus_force_pour_les_tests() {
        let run = |force: bool| {
            let ctx = egui::Context::default();
            let id = egui::Id::unique("champ");
            let mut txt = String::new();
            let mut focus = false;
            for k in 0..3 {
                let mut raw = egui::RawInput { focused: false, ..Default::default() };
                if force {
                    force_focus(&mut raw);
                }
                if k == 2 {
                    raw.events.push(egui::Event::Text("ok".into()));
                }
                let _ = ctx.run_ui(raw, |ui| {
                    let r = ui.add(egui::TextEdit::singleline(&mut txt).id(id));
                    if k == 0 {
                        r.request_focus();
                    }
                    focus = r.has_focus();
                });
            }
            (focus, txt)
        };
        assert!(!run(false).0, "le problème signalé par le lecteur : has_focus() reste faux");
        assert_eq!(run(true), (true, "ok".to_string()));
    }

    #[test]
    fn fenetre_cachee_et_capture() {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let h = unsafe {
            CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, 100, 100, 120, 80, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null())
        };
        assert!(!h.is_null());
        let focus = foreground();
        hide(h as isize);
        assert_eq!(foreground(), focus, "le focus n'a pas bougé");
        let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) };
        assert!(ex & WS_EX_NOACTIVATE as isize != 0 && ex & WS_EX_LAYERED as isize != 0 && ex & WS_EX_TRANSPARENT as isize != 0);
        let shot = capture(h as isize);
        unsafe { DestroyWindow(h) };
        let (w, ht, px) = shot.expect("capture PrintWindow");
        assert_eq!((w, ht, px.len()), (120, 80, 120 * 80 * 4));
        assert!(capture(0).is_none());
    }
}
