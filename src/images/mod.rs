//! Décodage d'images étendu (QoL T6), commun aux miniatures de l'explorateur et à l'affichage
//! du lecteur.
//!
//! | Formats | Décodeur | Feature |
//! |---|---|---|
//! | PNG, JPEG, GIF, WebP, BMP, ICO, TIFF, TGA, QOI, HDR, DDS (BC1–BC3) | crate `image` (Rust) | `images` |
//! | PSD / PSB (image composite) | `psd.rs` (Rust, maison) | `images` |
//! | Aperçu RAW (JPEG intégré : CR2, CR3, NEF, ARW, DNG, RAF, ORF, RW2, PEF, SRW…) | `raw.rs` (Rust, maison) | `images` |
//! | OpenEXR | crate `exr` (Rust) | `img-exr` |
//! | SVG / SVGZ | `resvg` (Rust) | `img-svg` |
//! | JPEG XL | `jxl-oxide` (Rust) | `img-jxl` |
//! | HEIC/HEIF, AVIF, JXR, DDS BC4–BC7, RAW complet, JPEG réduit au décodage | Windows (WIC) | `img-wic` |
//!
//! Deux API :
//! - [`decode`] : image complète (affichage) ;
//! - [`thumbnail`] : **miniature rapide**, réduite le plus tôt possible (JPEG réduit par le
//!   décodeur WIC au 1/2, 1/4, 1/8 ; PSD : une ligne sur k ; RAW : aperçu JPEG intégré ; SVG :
//!   rendu direct à la bonne taille), puis ramenée à `max` px.
//!
//! [`DiskCache`] garde les miniatures en QOI sous `%LOCALAPPDATA%\<App>\miniatures` (clé =
//! chemin + date + taille + côté) et [`Pool`] décode sur 2–4 threads, la demande la plus récente
//! d'abord, avec annulation.

mod psd;
mod raw;
#[cfg(feature = "img-svg")]
mod svg;
#[cfg(all(feature = "img-wic", windows))]
pub mod wic;

pub use image::RgbaImage;

use crate::Cancel;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex};

/// Format reconnu (par la signature, puis par l'extension).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Décodé par la crate `image`.
    Std(image::ImageFormat),
    Psd,
    Raw,
    Svg,
    Jxl,
    Exr,
    /// HEIC, AVIF, JXR… : seulement par Windows.
    Wic,
}

/// Extensions RAW d'appareils photo.
pub const RAW_EXTS: &[&str] = &["dng", "cr2", "cr3", "crw", "nef", "nrw", "arw", "srf", "sr2", "raf", "orf", "rw2", "pef", "srw", "x3f", "3fr", "iiq", "erf", "kdc", "mrw", "rwl"];

/// Toutes les extensions que ce build sait (peut-être) lire. Les formats WIC dépendent des
/// extensions installées sur Windows (HEIF, AV1…).
pub fn extensions() -> Vec<&'static str> {
    let mut v = vec!["png", "jpg", "jpeg", "jpe", "jfif", "gif", "webp", "bmp", "dib", "ico", "cur", "tif", "tiff", "tga", "dds", "hdr", "qoi", "psd", "psb"];
    v.extend_from_slice(RAW_EXTS);
    if cfg!(feature = "img-exr") {
        v.push("exr");
    }
    if cfg!(feature = "img-svg") {
        v.extend(["svg", "svgz"]);
    }
    if cfg!(feature = "img-jxl") || cfg!(feature = "img-wic") {
        v.push("jxl");
    }
    if cfg!(feature = "img-wic") {
        v.extend(["heic", "heif", "hif", "avif", "jxr", "wdp", "hdp"]);
    }
    v
}

/// Vrai si l'extension fait partie des formats lisibles par ce build.
pub fn is_supported(path: &Path) -> bool {
    let e = ext(path);
    extensions().contains(&e.as_str())
}

fn ext(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default()
}

/// Détecte le format d'après les premiers octets (et l'extension quand il n'y a pas de signature).
pub fn detect(head: &[u8], path: &Path) -> Option<Format> {
    use image::ImageFormat as F;
    let e = ext(path);
    if RAW_EXTS.contains(&e.as_str()) {
        return Some(Format::Raw);
    }
    let h = |sig: &[u8]| head.starts_with(sig);
    let f = if h(b"\x89PNG") {
        Format::Std(F::Png)
    } else if h(&[0xFF, 0xD8, 0xFF]) {
        Format::Std(F::Jpeg)
    } else if h(b"GIF8") {
        Format::Std(F::Gif)
    } else if h(b"RIFF") && head.get(8..12) == Some(b"WEBP") {
        Format::Std(F::WebP)
    } else if h(b"BM") {
        Format::Std(F::Bmp)
    } else if h(&[0, 0, 1, 0]) || h(&[0, 0, 2, 0]) {
        Format::Std(F::Ico)
    } else if h(b"II*\0") || h(b"MM\0*") {
        Format::Std(F::Tiff)
    } else if h(b"DDS ") {
        Format::Std(F::Dds)
    } else if h(b"8BPS") {
        Format::Psd
    } else if h(b"qoif") {
        Format::Std(F::Qoi)
    } else if h(&[0x76, 0x2F, 0x31, 0x01]) {
        Format::Exr
    } else if h(b"#?RADIANCE") || h(b"#?RGBE") {
        Format::Std(F::Hdr)
    } else if h(&[0xFF, 0x0A]) || h(&[0, 0, 0, 0x0C, b'J', b'X', b'L', b' ']) {
        Format::Jxl
    } else if head.get(4..8) == Some(b"ftyp") {
        Format::Wic // HEIC, HEIF, AVIF (WIC sait lire les deux familles)
    } else if h(&[0x49, 0x49, 0xBC]) {
        Format::Wic // JPEG XR
    } else if h(&[0x1F, 0x8B]) && e == "svgz" {
        Format::Svg
    } else {
        match e.as_str() {
            "svg" | "svgz" => Format::Svg,
            "tga" => Format::Std(F::Tga),
            "heic" | "heif" | "hif" | "avif" | "jxr" | "wdp" | "hdp" => Format::Wic,
            "exr" => Format::Exr,
            _ => {
                // SVG sans extension : on cherche la balise au début du texte
                let t = String::from_utf8_lossy(&head[..head.len().min(512)]);
                if t.contains("<svg") { Format::Svg } else { return None }
            }
        }
    };
    Some(f)
}

/// Erreur lisible pour l'utilisateur (toast rouge).
#[allow(dead_code)]
fn unsupported(what: &str, feature: &str) -> String {
    format!("{what} : non pris en charge dans ce build (feature « {feature} » du socle)")
}

/// Décode l'image complète (à appeler sur un thread). `cancel` est vérifié entre les étapes.
pub fn decode(path: &Path, cancel: &Cancel) -> Result<RgbaImage, String> {
    load(path, 0, cancel)
}

/// Miniature rapide : côté le plus long ≤ `max` px (jamais agrandie). À appeler sur un thread.
pub fn thumbnail(path: &Path, max: u32, cancel: &Cancel) -> Result<RgbaImage, String> {
    let img = load(path, max.max(1), cancel)?;
    if cancel.is_cancelled() {
        return Err("annulé".into());
    }
    Ok(fit(img, max.max(1)))
}

/// Réduit `img` pour que son plus grand côté fasse `max` px (filtre rapide). Ne l'agrandit pas.
pub fn fit(img: RgbaImage, max: u32) -> RgbaImage {
    let (w, h) = img.dimensions();
    if w <= max && h <= max {
        return img;
    }
    let k = max as f32 / w.max(h) as f32;
    let (nw, nh) = (((w as f32 * k).round() as u32).max(1), ((h as f32 * k).round() as u32).max(1));
    image::imageops::thumbnail(&img, nw, nh)
}

/// `max` = 0 : image complète ; sinon taille visée (le décodeur peut réduire tôt).
fn load(path: &Path, max: u32, cancel: &Cancel) -> Result<RgbaImage, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("lecture de {} : {e}", path.display()))?;
    if cancel.is_cancelled() {
        return Err("annulé".into());
    }
    let fmt = detect(&bytes[..bytes.len().min(512)], path).ok_or_else(|| "format d'image inconnu".to_string())?;
    let r = decode_format(fmt, &bytes, path, max, cancel);
    // Repli sur Windows (codecs du Store : HEIF, AV1, RAW…) quand le décodeur Rust échoue.
    #[cfg(all(feature = "img-wic", windows))]
    if r.is_err() && !cancel.is_cancelled() && fmt != Format::Wic {
        if let Ok(i) = if max > 0 { wic::decode_scaled(path, max) } else { wic::decode(path) } {
            return Ok(i);
        }
    }
    r
}

fn decode_format(fmt: Format, bytes: &[u8], path: &Path, max: u32, cancel: &Cancel) -> Result<RgbaImage, String> {
    let _ = (path, cancel);
    match fmt {
        Format::Std(image::ImageFormat::Jpeg) if max > 0 => {
            // JPEG : le décodeur de Windows sait réduire au 1/8 pendant le décodage (10× plus rapide
            // pour une photo de 24 Mpx). Sans WIC : décodage complet puis réduction.
            #[cfg(all(feature = "img-wic", windows))]
            if let Ok(i) = wic::decode_scaled(path, max) {
                return Ok(i);
            }
            std_decode(bytes, image::ImageFormat::Jpeg)
        }
        Format::Std(f) => std_decode(bytes, f),
        Format::Psd => psd::decode(bytes, max),
        Format::Raw => raw::preview(bytes, max).ok_or_else(|| "aperçu RAW introuvable".to_string()).and_then(|j| {
            if cancel.is_cancelled() {
                return Err("annulé".into());
            }
            std_decode(j, image::ImageFormat::Jpeg)
        }),
        Format::Exr => {
            #[cfg(feature = "img-exr")]
            {
                std_decode(bytes, image::ImageFormat::OpenExr)
            }
            #[cfg(not(feature = "img-exr"))]
            Err(unsupported("OpenEXR", "img-exr"))
        }
        Format::Svg => {
            #[cfg(feature = "img-svg")]
            {
                svg::render(bytes, max)
            }
            #[cfg(not(feature = "img-svg"))]
            Err(unsupported("SVG", "img-svg"))
        }
        Format::Jxl => {
            #[cfg(feature = "img-jxl")]
            {
                let d = jxl_oxide::integration::JxlDecoder::new(std::io::Cursor::new(bytes)).map_err(|e| format!("JPEG XL : {e}"))?;
                let img = image::DynamicImage::from_decoder(d).map_err(|e| format!("JPEG XL : {e}"))?;
                Ok(to_rgba8(img))
            }
            #[cfg(not(feature = "img-jxl"))]
            Err(unsupported("JPEG XL", "img-jxl ou img-wic"))
        }
        Format::Wic => {
            #[cfg(all(feature = "img-wic", windows))]
            {
                if max > 0 { wic::decode_scaled(path, max) } else { wic::decode(path) }
            }
            #[cfg(not(all(feature = "img-wic", windows)))]
            Err(unsupported("HEIC / AVIF / JXR", "img-wic"))
        }
    }
}

fn std_decode(bytes: &[u8], f: image::ImageFormat) -> Result<RgbaImage, String> {
    let img = image::load_from_memory_with_format(bytes, f).map_err(|e| e.to_string())?;
    Ok(to_rgba8(img))
}

/// Conversion en RGBA 8 bits. Les images flottantes (HDR, EXR) sont tonifiées (Reinhard + gamma
/// 2,2) au lieu d'être écrêtées, sinon elles paraissent brûlées.
fn to_rgba8(img: image::DynamicImage) -> RgbaImage {
    use image::DynamicImage as D;
    match img {
        D::ImageRgb32F(_) | D::ImageRgba32F(_) => {
            let f = img.into_rgba32f();
            let (w, h) = f.dimensions();
            let tm = |c: f32| {
                let c = c.max(0.0);
                ((c / (1.0 + c)).powf(1.0 / 2.2) * 255.0 + 0.5) as u8
            };
            let px: Vec<u8> = f.pixels().flat_map(|p| [tm(p[0]), tm(p[1]), tm(p[2]), (p[3].clamp(0.0, 1.0) * 255.0 + 0.5) as u8]).collect();
            RgbaImage::from_raw(w, h, px).unwrap_or_default()
        }
        other => other.into_rgba8(),
    }
}

// ---------------------------------------------------------------------------------------------
// Cache disque des miniatures
// ---------------------------------------------------------------------------------------------

/// Miniatures enregistrées en QOI (décodage ~0,2 ms pour 256 px). Un fichier modifié change de
/// clé (date + taille), donc pas d'invalidation à gérer.
#[derive(Clone, Debug)]
pub struct DiskCache {
    pub dir: PathBuf,
}

impl DiskCache {
    /// `%LOCALAPPDATA%\<app>\miniatures`.
    pub fn for_app(app: &str) -> Self {
        DiskCache { dir: crate::paths::cache_dir(app).join("miniatures") }
    }

    fn key(path: &Path, max: u32) -> Option<String> {
        let m = std::fs::metadata(path).ok()?;
        let t = m.modified().ok()?.duration_since(std::time::UNIX_EPOCH).ok()?.as_nanos();
        // FNV-1a 64 bits
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |b: &[u8]| {
            for &x in b {
                h ^= x as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(path.to_string_lossy().to_lowercase().as_bytes());
        eat(&t.to_le_bytes());
        eat(&m.len().to_le_bytes());
        eat(&max.to_le_bytes());
        Some(format!("{h:016x}.qoi"))
    }

    pub fn get(&self, path: &Path, max: u32) -> Option<RgbaImage> {
        let f = self.dir.join(Self::key(path, max)?);
        let data = std::fs::read(f).ok()?;
        let (hd, px) = qoi::decode_to_vec(&data).ok()?;
        let px = if hd.channels == qoi::Channels::Rgba { px } else { px.chunks_exact(3).flat_map(|c| [c[0], c[1], c[2], 255]).collect() };
        RgbaImage::from_raw(hd.width, hd.height, px)
    }

    pub fn put(&self, path: &Path, max: u32, img: &RgbaImage) {
        let Some(k) = Self::key(path, max) else { return };
        if let Ok(data) = qoi::encode_to_vec(img.as_raw(), img.width(), img.height()) {
            let _ = crate::paths::write_atomic(&self.dir.join(k), &data);
        }
    }

    /// Garde le cache sous `max_bytes` en supprimant les plus anciennes (à lancer sur un thread,
    /// par exemple au démarrage). Renvoie le nombre de fichiers supprimés.
    pub fn prune(&self, max_bytes: u64) -> usize {
        let Ok(rd) = std::fs::read_dir(&self.dir) else { return 0 };
        let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> =
            rd.flatten().filter_map(|e| e.metadata().ok().map(|m| (m.modified().unwrap_or(std::time::UNIX_EPOCH), m.len(), e.path()))).collect();
        let mut total: u64 = files.iter().map(|f| f.1).sum();
        files.sort_by_key(|f| f.0);
        let mut n = 0;
        for (_, len, p) in files {
            if total <= max_bytes {
                break;
            }
            if std::fs::remove_file(p).is_ok() {
                total -= len;
                n += 1;
            }
        }
        n
    }
}

/// Miniature via le cache disque (lecture, sinon décodage puis écriture).
pub fn thumbnail_cached(cache: &DiskCache, path: &Path, max: u32, cancel: &Cancel) -> Result<RgbaImage, String> {
    if let Some(i) = cache.get(path, max) {
        return Ok(i);
    }
    let img = thumbnail(path, max, cancel)?;
    cache.put(path, max, &img);
    Ok(img)
}

// ---------------------------------------------------------------------------------------------
// Pool de décodage
// ---------------------------------------------------------------------------------------------

/// Résultat d'une demande.
pub struct Done {
    pub id: u64,
    pub path: PathBuf,
    /// 0 = image complète.
    pub max: u32,
    pub result: Result<RgbaImage, String>,
}

struct Job {
    id: u64,
    path: PathBuf,
    max: u32,
    generation: u64,
}

struct Shared {
    queue: Mutex<(Vec<Job>, u64, bool)>, // (file, génération courante, arrêt)
    cv: Condvar,
    cancel: Mutex<Cancel>,
}

/// Décodeurs en tâche de fond. La demande la plus récente passe en premier (ce qui est à l'écran
/// maintenant), `cancel_pending` jette tout ce qui attend (changement de dossier, défilement).
pub struct Pool {
    shared: Arc<Shared>,
    rx: std::sync::mpsc::Receiver<Done>,
    next: u64,
}

impl Pool {
    /// `workers` threads (2–4 conseillé), cache disque optionnel, `wake` appelé à chaque
    /// résultat (typiquement `move || ctx.request_repaint()`).
    pub fn new(workers: usize, cache: Option<DiskCache>, wake: impl Fn() + Send + Sync + 'static) -> Pool {
        let shared = Arc::new(Shared { queue: Mutex::new((Vec::new(), 0, false)), cv: Condvar::new(), cancel: Mutex::new(Cancel::new()) });
        let (tx, rx) = std::sync::mpsc::channel();
        let wake = Arc::new(wake);
        for i in 0..workers.max(1) {
            let sh = shared.clone();
            let tx = tx.clone();
            let cache = cache.clone();
            let wake = wake.clone();
            let _ = std::thread::Builder::new().name(format!("socle-images-{i}")).spawn(move || {
                #[cfg(all(feature = "img-wic", windows))]
                wic::init_thread();
                loop {
                    let (job, cancel) = {
                        let mut q = sh.queue.lock().unwrap_or_else(|e| e.into_inner());
                        loop {
                            if q.2 {
                                return;
                            }
                            let cur = q.1;
                            q.0.retain(|j| j.generation == cur);
                            if let Some(j) = q.0.pop() {
                                break (j, sh.cancel.lock().unwrap_or_else(|e| e.into_inner()).clone());
                            }
                            q = sh.cv.wait(q).unwrap_or_else(|e| e.into_inner());
                        }
                    };
                    let result = if job.max == 0 {
                        decode(&job.path, &cancel)
                    } else if let Some(c) = &cache {
                        thumbnail_cached(c, &job.path, job.max, &cancel)
                    } else {
                        thumbnail(&job.path, job.max, &cancel)
                    };
                    if cancel.is_cancelled() {
                        continue;
                    }
                    if tx.send(Done { id: job.id, path: job.path, max: job.max, result }).is_err() {
                        return;
                    }
                    wake();
                }
            });
        }
        Pool { shared, rx, next: 1 }
    }

    /// Demande une miniature (`max` > 0) ou l'image complète (`max` = 0). Renvoie l'id.
    pub fn request(&mut self, path: impl Into<PathBuf>, max: u32) -> u64 {
        let id = self.next;
        self.next += 1;
        let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        let generation = q.1;
        q.0.push(Job { id, path: path.into(), max, generation });
        drop(q);
        self.shared.cv.notify_one();
        id
    }

    /// Abandonne les demandes en attente et signale l'annulation aux décodages en cours
    /// (leurs résultats ne seront pas livrés).
    pub fn cancel_pending(&mut self) {
        let mut q = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        q.1 += 1;
        q.0.clear();
        let mut c = self.shared.cancel.lock().unwrap_or_else(|e| e.into_inner());
        c.cancel();
        *c = Cancel::new();
    }

    /// Nombre de demandes en attente.
    pub fn pending(&self) -> usize {
        self.shared.queue.lock().map(|q| q.0.len()).unwrap_or(0)
    }

    /// Résultats arrivés depuis le dernier appel (à appeler à chaque image, ne bloque jamais).
    pub fn poll(&self) -> Vec<Done> {
        self.rx.try_iter().collect()
    }

    /// Attend un résultat (tests, outils en ligne de commande).
    pub fn recv_timeout(&self, t: std::time::Duration) -> Option<Done> {
        self.rx.recv_timeout(t).ok()
    }
}

impl Drop for Pool {
    fn drop(&mut self) {
        if let Ok(mut q) = self.shared.queue.lock() {
            q.2 = true;
        }
        self.shared.cv.notify_all();
    }
}

/// Convertit en `egui::ColorImage` (pour `ctx.load_texture`).
#[cfg(feature = "theme")]
pub fn to_color_image(img: &RgbaImage) -> egui::ColorImage {
    egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw())
}

/// Fabrique de fichiers de test (tests, benchmarks, démo).
#[doc(hidden)]
pub mod testdata {
    /// PSD RVB 8 bits compressé en RLE.
    pub fn psd_rgb_rle(img: &image::RgbImage) -> Vec<u8> {
        super::psd::encode_rgb_rle(img)
    }
}

#[cfg(test)]
mod tests;
