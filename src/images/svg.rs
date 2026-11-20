//! SVG / SVGZ par `resvg` (Rust pur). Sans la feature `text` de resvg : les `<text>` ne sont
//! pas dessinés (évite ~1 Mo et le chargement des polices système, ~50 ms au 1er SVG).

use super::RgbaImage;
use resvg::{tiny_skia, usvg};

/// Rendu à `max` px de côté (0 = taille propre du SVG, bornée à 512–4096 px).
pub fn render(bytes: &[u8], max: u32) -> Result<RgbaImage, String> {
    let tree = usvg::Tree::from_data(bytes, &usvg::Options::default()).map_err(|e| format!("SVG : {e}"))?;
    let size = tree.size();
    let (sw, sh) = (size.width().max(1.0), size.height().max(1.0));
    let target = if max > 0 { max as f32 } else { sw.max(sh).clamp(512.0, 4096.0) };
    let k = target / sw.max(sh);
    let (w, h) = (((sw * k).round() as u32).max(1), ((sh * k).round() as u32).max(1));
    let mut pm = tiny_skia::Pixmap::new(w, h).ok_or("SVG : taille invalide")?;
    resvg::render(&tree, tiny_skia::Transform::from_scale(k, k), &mut pm.as_mut());
    // tiny-skia travaille en alpha prémultiplié
    let mut px = pm.take();
    for p in px.chunks_exact_mut(4) {
        let a = p[3] as u32;
        if a > 0 && a < 255 {
            for c in &mut p[..3] {
                *c = ((*c as u32 * 255 + a / 2) / a).min(255) as u8;
            }
        }
    }
    RgbaImage::from_raw(w, h, px).ok_or_else(|| "SVG : image vide".into())
}
