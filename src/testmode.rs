//! **Mode test** (règle n° 0 de DIRECTION.md) : pendant les tests, l'utilisateur doit pouvoir se
//! servir de son PC comme si de rien n'était. Ce module donne aux apps les briques communes :
//!
//! - [`enabled`] : mode test demandé par `--test-hors-ecran` ou la variable `SOCLE_TEST=1` ;
//! - [`hide`] : fenêtre déplacée hors écran, **sans activation** (`WS_EX_NOACTIVATE`), hors de la
//!   barre des tâches (`WS_EX_TOOLWINDOW`) ; elle ne prend jamais le focus ;
//! - [`capture`] : capture de la fenêtre par `PrintWindow` (`PW_RENDERFULLCONTENT`), qui marche
//!   hors écran, sans toucher à l'écran ni au presse-papiers ;
//! - [`low_priority`] : processus en priorité basse (tests, compilations lancées par une app).
//!
//! Aucune entrée simulée n'est fournie, volontairement.
//!
//! `PrintWindow` ne capture qu'une fenêtre **affichée** : en mode test, la fenêtre est donc
//! affichée, mais hors écran, sans activation et hors de la barre des tâches. Avec winit :
//! ```ignore
//! let test = socle::testmode::enabled();
//! let mut attrs = Window::default_attributes().with_active(!test);
//! if test {
//!     attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(-32000, -32000));
//! }
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
    GWL_EXSTYLE, GetWindowLongPtrW, GetWindowRect, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, WS_EX_APPWINDOW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

/// Drapeau de ligne de commande du mode test.
pub const FLAG: &str = "--test-hors-ecran";

/// Mode test demandé ? (`--test-hors-ecran` ou `SOCLE_TEST=1`)
pub fn enabled() -> bool {
    std::env::args().any(|a| a == FLAG) || std::env::var("SOCLE_TEST").is_ok_and(|v| v == "1")
}

/// Position hors écran utilisée par `hide` (loin de tous les moniteurs).
pub const OFFSCREEN: (i32, i32) = (-32000, -32000);

/// Déplace la fenêtre hors écran, l'empêche de prendre le focus et la retire de la barre des
/// tâches. `hwnd` : handle Win32 (winit : `RawWindowHandle::Win32(h) => h.hwnd.get()`).
pub fn hide(hwnd: isize) {
    let h = hwnd as HWND;
    unsafe {
        let ex = GetWindowLongPtrW(h, GWL_EXSTYLE);
        let ex = (ex | (WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize) & !(WS_EX_APPWINDOW as isize);
        SetWindowLongPtrW(h, GWL_EXSTYLE, ex);
        SetWindowPos(h, std::ptr::null_mut(), OFFSCREEN.0, OFFSCREEN.1, 0, 0, SWP_NOACTIVATE | SWP_NOSIZE | SWP_NOZORDER);
    }
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
    use windows_sys::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, GetForegroundWindow, SW_SHOWNOACTIVATE, ShowWindow, WS_POPUP};

    /// Fenêtre de test hors écran (-32000), sans activation, hors de la barre des tâches :
    /// rien n'apparaît à l'écran et le focus de l'utilisateur ne bouge pas.
    #[test]
    fn fenetre_cachee_et_capture() {
        let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
        let h = unsafe {
            CreateWindowExW(WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW, class.as_ptr(), std::ptr::null(), WS_POPUP, OFFSCREEN.0, OFFSCREEN.1, 120, 80, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null())
        };
        assert!(!h.is_null());
        hide(h as isize);
        let focus = unsafe { GetForegroundWindow() };
        unsafe { ShowWindow(h, SW_SHOWNOACTIVATE) };
        assert_eq!(unsafe { GetForegroundWindow() }, focus, "le focus n'a pas bougé");
        let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) };
        assert!(ex & WS_EX_NOACTIVATE as isize != 0);
        let shot = capture(h as isize);
        unsafe { DestroyWindow(h) };
        let (w, ht, px) = shot.expect("capture PrintWindow");
        assert_eq!((w, ht, px.len()), (120, 80, 120 * 80 * 4));
        assert!(capture(0).is_none());
    }
}
