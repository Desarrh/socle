//! Mesure la RAM occupée par chaque police facultative (`theme::FontOptions`), sans fenêtre :
//! mémoire privée du processus avant et après l'installation des polices et le rendu d'une page
//! de texte (atlas de glyphes compris).
//! `cargo run --release --example polices --features theme -- [aucune|defaut|gras|emoji|symboles-seuls|tout]`

use socle::theme::{FontOptions, install_fonts_with};

fn prive() -> usize {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS_EX};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    unsafe {
        let mut c: PROCESS_MEMORY_COUNTERS_EX = std::mem::zeroed();
        c.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32;
        GetProcessMemoryInfo(GetCurrentProcess(), (&mut c as *mut PROCESS_MEMORY_COUNTERS_EX).cast(), c.cb);
        c.PrivateUsage
    }
}

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_else(|| "defaut".into());
    let opt = match mode.as_str() {
        "aucune" => None,
        "gras" => Some(FontOptions { bold: true, ..Default::default() }),
        "emoji" => Some(FontOptions { emoji: true, ..Default::default() }),
        "symboles-seuls" => Some(FontOptions { bold: false, emoji: false, symbols: true }),
        "sans-symboles" => Some(FontOptions { bold: false, emoji: false, symbols: false }),
        "tout" => Some(FontOptions { bold: true, emoji: true, symbols: true }),
        _ => Some(FontOptions::default()),
    };
    let ctx = egui::Context::default();
    let input = || egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1200.0, 800.0))), ..Default::default() };
    let _ = ctx.run_ui(input(), |_| {});
    let avant = prive();
    if let Some(o) = opt {
        install_fonts_with(&ctx, o);
        for _ in 0..3 {
            let _ = ctx.run_ui(input(), |ui| {
                for t in ["Explorateur — Téléchargements", "Ça marche : 12,5 Mo · 3 s", "ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789 ←↑→↓ ⋯ ●"] {
                    ui.label(t);
                    ui.label(egui::RichText::new(t).family(socle::theme::bold()).size(15.0));
                    ui.monospace(t);
                }
            });
        }
    }
    let apres = prive();
    println!("{mode:<14} +{:.1} Mo de mémoire privée", (apres as f64 - avant as f64) / 1_048_576.0);
}
