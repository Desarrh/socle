//! Aperçu des photos RAW : on ne « développe » pas le RAW (lent, des dizaines de formats),
//! on extrait le **JPEG d'aperçu** que tous les boîtiers enregistrent dans le fichier
//! (souvent en pleine résolution).
//!
//! Méthode universelle (CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2, PEF, SRW…) : chercher toutes
//! les signatures JPEG `FF D8 FF` (memchr, SIMD), lire la taille dans l'en-tête SOF de chacune
//! (quelques octets) et garder la plus grande. Pour une miniature, on prend la plus petite qui
//! couvre `max` (les boîtiers ont souvent 160 px, ~1 600 px et la pleine taille).

/// JPEG intégré le plus adapté : le plus grand (`max` = 0), sinon le plus petit ≥ `max`.
pub fn preview(bytes: &[u8], max: u32) -> Option<&[u8]> {
    let mut best: Option<(u32, usize)> = None; // (côté, position)
    let mut fits: Option<(u32, usize)> = None;
    for pos in memchr::memmem::find_iter(bytes, &[0xFF, 0xD8, 0xFF]) {
        let Some((w, h)) = sof_size(&bytes[pos..]) else { continue };
        let side = w.max(h);
        if side < 64 {
            continue;
        }
        if best.is_none_or(|b| side > b.0) {
            best = Some((side, pos));
        }
        if max > 0 && side >= max && fits.is_none_or(|f| side < f.0) {
            fits = Some((side, pos));
        }
    }
    let (_, pos) = if max > 0 { fits.or(best)? } else { best? };
    Some(&bytes[pos..])
}

/// Dimensions lues dans le segment SOF d'un JPEG (sans le décoder).
fn sof_size(j: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    // On ne parcourt que l'en-tête : au-delà de 1 Mo, ce n'est pas un vrai JPEG.
    while i + 9 < j.len().min(1 << 20) {
        if j[i] != 0xFF {
            return None;
        }
        let m = j[i + 1];
        if m == 0xFF {
            i += 1;
            continue;
        }
        let len = u16::from_be_bytes([j[i + 2], j[i + 3]]) as usize;
        // SOF0..SOF15 sauf DHT (C4), JPG (C8), DAC (CC)
        if (0xC0..=0xCF).contains(&m) && !matches!(m, 0xC4 | 0xC8 | 0xCC) {
            let h = u16::from_be_bytes([j[i + 5], j[i + 6]]) as u32;
            let w = u16::from_be_bytes([j[i + 7], j[i + 8]]) as u32;
            return (w > 0 && h > 0).then_some((w, h));
        }
        if m == 0xDA || len < 2 {
            return None; // début des données sans SOF
        }
        i += 2 + len;
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn choisit_le_bon_apercu() {
        let jpeg = |w: u32, h: u32| {
            let img = image::RgbImage::from_pixel(w, h, image::Rgb([200, 40, 40]));
            let mut v = Vec::new();
            image::codecs::jpeg::JpegEncoder::new(&mut v).encode_image(&img).unwrap();
            v
        };
        // faux RAW : en-tête TIFF + vignette 160 + aperçu 1200 + données
        let mut raw = b"II*\0\x08\0\0\0".to_vec();
        raw.extend(vec![0u8; 1000]);
        raw.extend(jpeg(160, 120));
        raw.extend(vec![7u8; 500]);
        raw.extend(jpeg(1200, 800));
        raw.extend(vec![0xFFu8, 0xD8, 0xFF, 0x00]); // fausse signature
        let full = super::preview(&raw, 0).unwrap();
        assert_eq!(super::sof_size(full), Some((1200, 800)));
        let small = super::preview(&raw, 128).unwrap();
        assert_eq!(super::sof_size(small), Some((160, 120)));
        let mid = super::preview(&raw, 256).unwrap();
        assert_eq!(super::sof_size(mid), Some((1200, 800)));
        assert!(super::preview(&[0u8; 100], 0).is_none());
    }
}
