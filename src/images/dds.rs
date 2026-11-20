//! DDS **non compressés**, que la crate `image` refuse (« DDS non pris en charge ») : en-tête
//! classique avec masques de bits (RVB, RVBA, luminance, alpha seul ; 8, 16, 24 ou 32 bits) et
//! en-tête DX10 pour les formats 8 bits par canal courants. Seul le 1er niveau de mipmap est lu.

use super::RgbaImage;

fn u32le(b: &[u8], o: usize) -> Option<u32> {
    b.get(o..o + 4).map(|s| u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Extrait un canal selon son masque et le ramène sur 8 bits.
fn channel(px: u32, mask: u32) -> Option<u8> {
    if mask == 0 {
        return None;
    }
    let shift = mask.trailing_zeros();
    let bits = (mask >> shift).count_ones();
    let v = (px & mask) >> shift;
    let max = (1u64 << bits) - 1;
    Some(((v as u64 * 255 + max / 2) / max) as u8)
}

pub fn decode_uncompressed(b: &[u8]) -> Option<RgbaImage> {
    if b.get(0..4)? != b"DDS " || u32le(b, 4)? != 124 {
        return None;
    }
    let h = u32le(b, 12)? as usize;
    let w = u32le(b, 16)? as usize;
    if w == 0 || h == 0 || w > 32768 || h > 32768 {
        return None;
    }
    let pf_flags = u32le(b, 80)?;
    let fourcc = b.get(84..88)?;
    let (bits, masks, data) = if pf_flags & 0x4 != 0 && fourcc == b"DX10" {
        // DXGI_FORMAT : 28/29 = R8G8B8A8, 87/91 = B8G8R8A8, 88/93 = B8G8R8X8
        let masks = match u32le(b, 128)? {
            28 | 29 => [0x0000_00FF, 0x0000_FF00, 0x00FF_0000, 0xFF00_0000],
            87 | 91 => [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0xFF00_0000],
            88 | 93 => [0x00FF_0000, 0x0000_FF00, 0x0000_00FF, 0],
            _ => return None,
        };
        (32, masks, 148)
    } else if pf_flags & 0x4 != 0 {
        return None; // compressé (FourCC) : `image` ou WIC
    } else {
        let bits = u32le(b, 88)? as usize;
        let (r, g, bl, a) = (u32le(b, 92)?, u32le(b, 96)?, u32le(b, 100)?, u32le(b, 104)?);
        // 0x1 = alpha présent, 0x2 = alpha seul
        let a = if pf_flags & 0x3 != 0 { a } else { 0 };
        if !matches!(bits, 8 | 16 | 24 | 32) {
            return None;
        }
        // luminance (0x20000) : le masque « rouge » porte la valeur, recopiée en gris
        let lum = pf_flags & 0x2_0000 != 0;
        let masks = if lum { [r, r, r, a] } else { [r, g, bl, a] };
        (bits, masks, 128)
    };
    let bpp = bits / 8;
    let pitch = w * bpp;
    let src = b.get(data..data + pitch * h)?;
    let mut out = Vec::with_capacity(w * h * 4);
    for px in src.chunks_exact(bpp) {
        let mut v = 0u32;
        for (i, &x) in px.iter().enumerate() {
            v |= (x as u32) << (8 * i);
        }
        let c = |m: u32| channel(v, m);
        // alpha seul : blanc avec transparence
        let (r, g, bl) = if masks[0] == 0 && masks[1] == 0 && masks[2] == 0 {
            (255, 255, 255)
        } else {
            (c(masks[0]).unwrap_or(0), c(masks[1]).unwrap_or(0), c(masks[2]).unwrap_or(0))
        };
        out.extend_from_slice(&[r, g, bl, c(masks[3]).unwrap_or(255)]);
    }
    RgbaImage::from_raw(w as u32, h as u32, out)
}

/// DDS 32 bits BGRA non compressé (tests).
pub fn encode_bgra32(img: &RgbaImage) -> Vec<u8> {
    let (w, h) = img.dimensions();
    let mut v = vec![0u8; 128];
    v[0..4].copy_from_slice(b"DDS ");
    let put = |v: &mut Vec<u8>, o: usize, x: u32| v[o..o + 4].copy_from_slice(&x.to_le_bytes());
    put(&mut v, 4, 124);
    put(&mut v, 8, 0x1 | 0x2 | 0x4 | 0x8 | 0x1000); // caps, height, width, pitch, pixelformat
    put(&mut v, 12, h);
    put(&mut v, 16, w);
    put(&mut v, 20, w * 4);
    put(&mut v, 76, 32);
    put(&mut v, 80, 0x40 | 0x1); // RGB + alpha
    put(&mut v, 88, 32);
    put(&mut v, 92, 0x00FF_0000);
    put(&mut v, 96, 0x0000_FF00);
    put(&mut v, 100, 0x0000_00FF);
    put(&mut v, 104, 0xFF00_0000);
    put(&mut v, 108, 0x1000);
    for p in img.pixels() {
        v.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masques() {
        // R5G6B5 : rouge pur = 0xF800
        assert_eq!(channel(0xF800, 0xF800), Some(255));
        assert_eq!(channel(0x07E0, 0x07E0), Some(255));
        assert_eq!(channel(0x0000, 0x001F), Some(0));
        assert_eq!(channel(1, 0), None);
    }
}
