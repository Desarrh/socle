//! Décodage par Windows (WIC) : HEIC/HEIF, AVIF, JPEG XR, DDS (BC1–BC7), RAW complet, JPEG XL
//! (si l'extension du Store est installée)… Tout ce que Windows sait lire, sans rien embarquer.
//! Repris de `lecteur-universel/src/wic.rs`, plus le **décodage réduit** : pour un JPEG, le
//! décodeur de Windows sait sortir directement du 1/2, 1/4 ou 1/8 (`IWICBitmapSourceTransform`),
//! ce qui évite de décoder 24 Mpx pour une miniature de 256 px.

use super::RgbaImage;
use std::path::Path;
use windows::Win32::Foundation::GENERIC_READ;
use windows::Win32::Graphics::Imaging::*;
use windows::Win32::System::Com::*;
use windows::core::{HSTRING, Interface};

/// À appeler une fois par thread de décodage (le pool le fait).
pub fn init_thread() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn err(e: windows::core::Error) -> String {
    e.message().to_string()
}

fn factory() -> Result<IWICImagingFactory, String> {
    init_thread();
    unsafe { CoCreateInstance(&CLSID_WICImagingFactory, None, CLSCTX_INPROC_SERVER).map_err(err) }
}

fn frame(f: &IWICImagingFactory, path: &Path) -> Result<IWICBitmapFrameDecode, String> {
    unsafe {
        let dec = f
            .CreateDecoderFromFilename(&HSTRING::from(path.as_os_str()), None, GENERIC_READ, WICDecodeMetadataCacheOnDemand)
            .map_err(|_| missing_codec(path))?;
        dec.GetFrame(0).map_err(err)
    }
}

/// Copie une source WIC en RGBA 8 bits.
fn to_rgba(f: &IWICImagingFactory, src: &IWICBitmapSource) -> Result<RgbaImage, String> {
    unsafe {
        let conv = f.CreateFormatConverter().map_err(err)?;
        conv.Initialize(src, &GUID_WICPixelFormat32bppRGBA, WICBitmapDitherTypeNone, None, 0.0, WICBitmapPaletteTypeCustom).map_err(err)?;
        let (mut w, mut h) = (0u32, 0u32);
        conv.GetSize(&mut w, &mut h).map_err(err)?;
        let mut buf = vec![0u8; (w as usize) * (h as usize) * 4];
        conv.CopyPixels(std::ptr::null(), w * 4, &mut buf).map_err(err)?;
        RgbaImage::from_raw(w, h, buf).ok_or_else(|| "image vide".to_string())
    }
}

/// Image complète.
pub fn decode(path: &Path) -> Result<RgbaImage, String> {
    let f = factory()?;
    let fr = frame(&f, path)?;
    to_rgba(&f, &fr.cast::<IWICBitmapSource>().map_err(err)?)
}

/// Image réduite pour que son plus grand côté fasse au moins `max` px quand le décodeur sait
/// réduire nativement (JPEG : 1/2, 1/4, 1/8), sinon réduite par le redimensionneur de WIC.
pub fn decode_scaled(path: &Path, max: u32) -> Result<RgbaImage, String> {
    let f = factory()?;
    let fr = frame(&f, path)?;
    unsafe {
        let (mut w, mut h) = (0u32, 0u32);
        fr.GetSize(&mut w, &mut h).map_err(err)?;
        if w.max(h) <= max {
            return to_rgba(&f, &fr.cast::<IWICBitmapSource>().map_err(err)?);
        }
        let k = max as f64 / w.max(h) as f64;
        let (tw, th) = (((w as f64 * k).ceil() as u32).max(1), ((h as f64 * k).ceil() as u32).max(1));
        // 1. Réduction native du décodeur (aucun pixel inutile décodé).
        if let Ok(tr) = fr.cast::<IWICBitmapSourceTransform>() {
            let (mut cw, mut ch) = (tw, th);
            if tr.GetClosestSize(&mut cw, &mut ch).is_ok() && cw < w {
                // la taille proche peut être plus petite que demandée : on remonte d'un cran
                if cw.max(ch) < max {
                    let (mut cw2, mut ch2) = ((tw * 2).min(w), (th * 2).min(h));
                    if tr.GetClosestSize(&mut cw2, &mut ch2).is_ok() {
                        (cw, ch) = (cw2, ch2);
                    }
                }
                let mut fmt = GUID_WICPixelFormat24bppBGR;
                let _ = tr.GetClosestPixelFormat(&mut fmt);
                let bpp = if fmt == GUID_WICPixelFormat24bppBGR {
                    3
                } else if fmt == GUID_WICPixelFormat32bppBGRA || fmt == GUID_WICPixelFormat32bppBGR {
                    4
                } else if fmt == GUID_WICPixelFormat8bppGray {
                    1
                } else {
                    0
                };
                if bpp > 0 {
                    let stride = (cw * bpp).next_multiple_of(4);
                    let mut buf = vec![0u8; (stride * ch) as usize];
                    if tr.CopyPixels(std::ptr::null(), cw, ch, &fmt, WICBitmapTransformRotate0, stride, &mut buf).is_ok() {
                        let mut out = Vec::with_capacity((cw * ch * 4) as usize);
                        for row in buf.chunks_exact(stride as usize) {
                            for px in row[..(cw * bpp) as usize].chunks_exact(bpp as usize) {
                                match bpp {
                                    1 => out.extend_from_slice(&[px[0], px[0], px[0], 255]),
                                    3 => out.extend_from_slice(&[px[2], px[1], px[0], 255]),
                                    _ => out.extend_from_slice(&[px[2], px[1], px[0], if fmt == GUID_WICPixelFormat32bppBGR { 255 } else { px[3] }]),
                                }
                            }
                        }
                        if let Some(i) = RgbaImage::from_raw(cw, ch, out) {
                            return Ok(i);
                        }
                    }
                }
            }
        }
        // 2. Redimensionneur de WIC (Fant : bonne qualité en réduction).
        let sc = f.CreateBitmapScaler().map_err(err)?;
        sc.Initialize(&fr, tw, th, WICBitmapInterpolationModeFant).map_err(err)?;
        to_rgba(&f, &sc.cast::<IWICBitmapSource>().map_err(err)?)
    }
}

/// Message clair quand Windows n'a pas le codec (le lecteur l'affichait déjà).
fn missing_codec(path: &Path) -> String {
    let e = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    match e.as_str() {
        "heic" | "heif" | "hif" => "Format HEIC : installez « Extensions d'image HEIF » (et « Extensions vidéo HEVC ») depuis le Microsoft Store.".into(),
        "avif" => "Format AVIF : installez « Extension vidéo AV1 » depuis le Microsoft Store.".into(),
        "jxl" => "Format JPEG XL : installez « Extension d'image JPEG XL » depuis le Microsoft Store.".into(),
        e if super::RAW_EXTS.contains(&e) => "Photo RAW : installez « Extension d'image brute » depuis le Microsoft Store.".into(),
        _ => "Windows ne sait pas décoder ce format.".into(),
    }
}
