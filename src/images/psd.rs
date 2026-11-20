//! PSD / PSB : lecture de l'**image composite** (« données d'image » en fin de fichier, que
//! Photoshop enregistre avec « Maximiser la compatibilité », activé par défaut). On ne lit
//! pas les calques : pas besoin pour un aperçu, et 50× plus simple.
//!
//! Miniature : les lignes RLE sont indépendantes (table des longueurs en tête), on ne décode
//! qu'une ligne sur `step` et une colonne sur `step` → coût proportionnel à la miniature.
//!
//! Modes pris en charge : RVB, niveaux de gris, CMJN, couleurs indexées ; 8, 16 et 32 bits.

use super::RgbaImage;

struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        let s = self.b.get(self.p..self.p + n).ok_or("PSD tronqué")?;
        self.p += n;
        Ok(s)
    }
    fn u16(&mut self) -> Result<u16, String> {
        let s = self.take(2)?;
        Ok(u16::from_be_bytes([s[0], s[1]]))
    }
    fn u32(&mut self) -> Result<u32, String> {
        let s = self.take(4)?;
        Ok(u32::from_be_bytes([s[0], s[1], s[2], s[3]]))
    }
    fn u64(&mut self) -> Result<u64, String> {
        let s = self.take(8)?;
        Ok(u64::from_be_bytes(s.try_into().unwrap_or([0; 8])))
    }
    fn skip_section(&mut self, wide: bool) -> Result<(), String> {
        let n = if wide { self.u64()? as usize } else { self.u32()? as usize };
        self.take(n).map(|_| ())
    }
}

/// `max` = 0 : pleine taille ; sinon sous-échantillonnage entier tant que l'image reste ≥ `max`.
pub fn decode(bytes: &[u8], max: u32) -> Result<RgbaImage, String> {
    let mut r = Rd { b: bytes, p: 0 };
    if r.take(4)? != b"8BPS" {
        return Err("pas un PSD".into());
    }
    let version = r.u16()?;
    let psb = version == 2;
    r.take(6)?;
    let channels = r.u16()? as usize;
    let h = r.u32()? as usize;
    let w = r.u32()? as usize;
    let depth = r.u16()? as usize;
    let mode = r.u16()?;
    if w == 0 || h == 0 || w > 300_000 || h > 300_000 {
        return Err("PSD : dimensions invalides".into());
    }
    // Données de mode couleur (palette pour l'indexé).
    let cm_len = r.u32()? as usize;
    let palette = r.take(cm_len)?.to_vec();
    r.skip_section(false)?; // ressources d'image
    r.skip_section(psb)?; // calques et masques
    let compression = r.u16()?;
    let bpc = match depth {
        8 => 1,
        16 => 2,
        32 => 4,
        1 => return Err("PSD 1 bit non pris en charge".into()),
        _ => return Err(format!("PSD : profondeur {depth} bits inconnue")),
    };
    let used = match mode {
        1 | 8 => 1,     // gris, bichromie
        2 => 1,         // indexé
        3 => 3,         // RVB
        4 => 4,         // CMJN
        9 => 3,         // Lab : affiché tel quel (approximation)
        m => return Err(format!("PSD : mode couleur {m} non pris en charge")),
    }
    .min(channels);
    let alpha = channels > used && matches!(mode, 1 | 3); // 1er canal supplémentaire = transparence
    let need = used + alpha as usize;

    let step = if max == 0 { 1 } else { (w.max(h) / max as usize).max(1) };
    let (ow, oh) = (w.div_ceil(step), h.div_ceil(step));
    let row_bytes = w * bpc;
    // planes[c][y * ow + x] : valeurs 8 bits
    let mut planes = vec![vec![0u8; ow * oh]; need];
    let sample = |row: &[u8], x: usize| -> u8 {
        match bpc {
            1 => row[x],
            2 => row[x * 2],
            _ => {
                let v = f32::from_be_bytes([row[x * 4], row[x * 4 + 1], row[x * 4 + 2], row[x * 4 + 3]]);
                (v.clamp(0.0, 1.0).powf(1.0 / 2.2) * 255.0 + 0.5) as u8
            }
        }
    };
    match compression {
        0 => {
            let data = &bytes[r.p..];
            for c in 0..need {
                for oy in 0..oh {
                    let y = oy * step;
                    let off = (c * h + y) * row_bytes;
                    let row = data.get(off..off + row_bytes).ok_or("PSD tronqué")?;
                    for ox in 0..ow {
                        planes[c][oy * ow + ox] = sample(row, ox * step);
                    }
                }
            }
        }
        1 => {
            // Table des longueurs : `channels × h` entrées (u16, ou u32 en PSB).
            let n = channels * h;
            let mut lens = Vec::with_capacity(n);
            for _ in 0..n {
                lens.push(if psb { r.u32()? as usize } else { r.u16()? as usize });
            }
            let mut offs = Vec::with_capacity(n + 1);
            let mut acc = r.p;
            for &l in &lens {
                offs.push(acc);
                acc += l;
            }
            let mut row = vec![0u8; row_bytes];
            for c in 0..need {
                for oy in 0..oh {
                    let i = c * h + oy * step;
                    let src = bytes.get(offs[i]..offs[i] + lens[i]).ok_or("PSD tronqué")?;
                    unpack_bits(src, &mut row);
                    for ox in 0..ow {
                        planes[c][oy * ow + ox] = sample(&row, ox * step);
                    }
                }
            }
        }
        2 | 3 => return Err("PSD : compression ZIP non prise en charge".into()),
        c => return Err(format!("PSD : compression {c} inconnue")),
    }

    let mut out = Vec::with_capacity(ow * oh * 4);
    for i in 0..ow * oh {
        let (rv, gv, bv) = match mode {
            3 | 9 => (planes[0][i], planes[1.min(need - 1)][i], planes[2.min(need - 1)][i]),
            4 => {
                // CMJN stocké inversé dans les PSD (0 = encre pleine)
                let k = planes[3.min(need - 1)][i] as u32;
                let f = |v: u8| ((v as u32 * k) / 255) as u8;
                (f(planes[0][i]), f(planes[1][i]), f(planes[2][i]))
            }
            2 => {
                let ix = planes[0][i] as usize;
                let g = |o: usize| palette.get(o * 256 + ix).copied().unwrap_or(0);
                (g(0), g(1), g(2))
            }
            _ => (planes[0][i], planes[0][i], planes[0][i]),
        };
        let a = if alpha { planes[used][i] } else { 255 };
        // L'image composite est « matée » sur du blanc là où elle est transparente : on retire le blanc.
        let unmatte = |c: u8| -> u8 {
            if a == 255 || a == 0 {
                c
            } else {
                let af = a as f32 / 255.0;
                (((c as f32 - 255.0 * (1.0 - af)) / af).clamp(0.0, 255.0)) as u8
            }
        };
        out.extend_from_slice(&[unmatte(rv), unmatte(gv), unmatte(bv), a]);
    }
    RgbaImage::from_raw(ow as u32, oh as u32, out).ok_or_else(|| "PSD : image vide".into())
}

/// PackBits (Apple) : n ≥ 0 → n+1 octets littéraux ; n < 0 → octet suivant répété 1−n fois.
fn unpack_bits(src: &[u8], out: &mut [u8]) {
    let (mut i, mut o) = (0, 0);
    while i < src.len() && o < out.len() {
        let n = src[i] as i8;
        i += 1;
        if n >= 0 {
            let cnt = (n as usize + 1).min(out.len() - o).min(src.len().saturating_sub(i));
            out[o..o + cnt].copy_from_slice(&src[i..i + cnt]);
            i += n as usize + 1;
            o += cnt;
        } else if n != -128 {
            let cnt = ((1 - n as isize) as usize).min(out.len() - o);
            let v = src.get(i).copied().unwrap_or(0);
            out[o..o + cnt].fill(v);
            i += 1;
            o += cnt;
        }
    }
}

/// Écrit un PSD RVB 8 bits compressé en RLE (pour les tests et les benchmarks).
pub fn encode_rgb_rle(img: &image::RgbImage) -> Vec<u8> {
    let (w, h) = (img.width() as usize, img.height() as usize);
    let mut v = Vec::new();
    v.extend_from_slice(b"8BPS");
    v.extend_from_slice(&1u16.to_be_bytes());
    v.extend_from_slice(&[0; 6]);
    v.extend_from_slice(&3u16.to_be_bytes());
    v.extend_from_slice(&(h as u32).to_be_bytes());
    v.extend_from_slice(&(w as u32).to_be_bytes());
    v.extend_from_slice(&8u16.to_be_bytes());
    v.extend_from_slice(&3u16.to_be_bytes());
    v.extend_from_slice(&0u32.to_be_bytes()); // mode couleur
    v.extend_from_slice(&0u32.to_be_bytes()); // ressources
    v.extend_from_slice(&0u32.to_be_bytes()); // calques
    v.extend_from_slice(&1u16.to_be_bytes()); // RLE
    let mut rows: Vec<Vec<u8>> = Vec::new();
    for c in 0..3 {
        for y in 0..h {
            let line: Vec<u8> = (0..w).map(|x| img.get_pixel(x as u32, y as u32)[c]).collect();
            // encodage simple : répétitions ≥ 3, sinon littéraux (≤ 128)
            let mut enc = Vec::new();
            let mut i = 0;
            while i < w {
                let mut run = 1;
                while i + run < w && line[i + run] == line[i] && run < 128 {
                    run += 1;
                }
                if run >= 3 {
                    enc.push((1i16 - run as i16) as i8 as u8);
                    enc.push(line[i]);
                    i += run;
                } else {
                    let start = i;
                    while i < w && i - start < 128 && !(i + 2 < w && line[i] == line[i + 1] && line[i] == line[i + 2]) {
                        i += 1;
                    }
                    enc.push((i - start - 1) as u8);
                    enc.extend_from_slice(&line[start..i]);
                }
            }
            rows.push(enc);
        }
    }
    for r in &rows {
        v.extend_from_slice(&(r.len() as u16).to_be_bytes());
    }
    for r in rows {
        v.extend_from_slice(&r);
    }
    v
}
