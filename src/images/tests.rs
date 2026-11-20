//! Tests du décodage : fichiers générés à la volée dans %TEMP% (aucun fichier binaire versionné).
use super::*;
use std::time::Duration;

fn dir() -> PathBuf {
    let d = std::env::temp_dir().join(format!("socle-images-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// Dégradé 640 × 400 avec un carré rouge en haut à gauche.
fn sample() -> image::RgbImage {
    image::RgbImage::from_fn(640, 400, |x, y| if x < 64 && y < 64 { image::Rgb([255, 0, 0]) } else { image::Rgb([(x / 3) as u8, (y / 2) as u8, 128]) })
}

fn write(name: &str, f: image::ImageFormat) -> PathBuf {
    let p = dir().join(name);
    let img = image::DynamicImage::ImageRgb8(sample());
    let img = if matches!(f, image::ImageFormat::Hdr | image::ImageFormat::OpenExr) { image::DynamicImage::ImageRgb32F(img.into_rgb32f()) } else { img };
    img.save_with_format(&p, f).unwrap();
    p
}

fn check(p: &Path) {
    let c = Cancel::new();
    let full = decode(p, &c).unwrap_or_else(|e| panic!("{} : {e}", p.display()));
    assert_eq!(full.dimensions(), (640, 400), "{}", p.display());
    let px = full.get_pixel(10, 10);
    assert!(px[0] > 200 && px[1] < 60, "{} : pixel {px:?}", p.display());
    let t = thumbnail(p, 128, &c).unwrap();
    assert_eq!(t.width().max(t.height()), 128, "{}", p.display());
}

#[test]
fn formats_rust() {
    use image::ImageFormat as F;
    for (n, f) in [("a.png", F::Png), ("a.jpg", F::Jpeg), ("a.bmp", F::Bmp), ("a.gif", F::Gif), ("a.tga", F::Tga), ("a.qoi", F::Qoi), ("a.tif", F::Tiff), ("a.webp", F::WebP)] {
        check(&write(n, f));
    }
    // HDR : flottant tonifié, le rouge reste rouge
    let p = write("a.hdr", F::Hdr);
    let i = decode(&p, &Cancel::new()).unwrap();
    assert_eq!(i.dimensions(), (640, 400));
}

#[cfg(feature = "img-exr")]
#[test]
fn exr() {
    let p = write("a.exr", image::ImageFormat::OpenExr);
    assert_eq!(decode(&p, &Cancel::new()).unwrap().dimensions(), (640, 400));
}

#[test]
fn psd_composite_et_miniature() {
    let p = dir().join("a.psd");
    std::fs::write(&p, testdata::psd_rgb_rle(&sample())).unwrap();
    check(&p);
    // sous-échantillonnage : une ligne sur 4
    let t = psd::decode(&std::fs::read(&p).unwrap(), 128).unwrap();
    assert_eq!(t.dimensions(), (128, 80));
}

#[test]
fn raw_apercu() {
    let p = dir().join("a.nef");
    let mut raw = b"MM\0*\0\0\0\x08".to_vec();
    raw.extend(vec![0u8; 4096]);
    let mut j = Vec::new();
    image::DynamicImage::ImageRgb8(sample()).write_to(&mut std::io::Cursor::new(&mut j), image::ImageFormat::Jpeg).unwrap();
    raw.extend(j);
    raw.extend(vec![0x55u8; 4096]);
    std::fs::write(&p, raw).unwrap();
    check(&p);
}

#[cfg(feature = "img-svg")]
#[test]
fn svg() {
    let p = dir().join("a.svg");
    std::fs::write(&p, r##"<svg xmlns="http://www.w3.org/2000/svg" width="640" height="400"><rect width="640" height="400" fill="#202080"/><rect width="64" height="64" fill="#ff0000"/></svg>"##).unwrap();
    check(&p);
    let t = thumbnail(&p, 1280, &Cancel::new()).unwrap();
    assert_eq!(t.width(), 1280, "vectoriel : rendu net à la taille demandée");
}

#[test]
fn detection() {
    assert_eq!(detect(b"\x89PNG....", Path::new("x.bin")), Some(Format::Std(image::ImageFormat::Png)));
    assert_eq!(detect(b"8BPS", Path::new("x")), Some(Format::Psd));
    assert_eq!(detect(b"\0\0\0\x1cftypheic", Path::new("x.heic")), Some(Format::Wic));
    assert_eq!(detect(b"II*\0", Path::new("x.cr2")), Some(Format::Raw));
    assert_eq!(detect(b"<?xml version", Path::new("x.svg")), Some(Format::Svg));
    assert_eq!(detect(b"rien", Path::new("x.dat")), None);
    assert!(is_supported(Path::new("C:/a/B.PSD")));
}

#[test]
fn cache_disque() {
    let p = write("c.png", image::ImageFormat::Png);
    let cache = DiskCache { dir: dir().join("cache") };
    let _ = std::fs::remove_dir_all(&cache.dir);
    assert!(cache.get(&p, 96).is_none());
    let a = thumbnail_cached(&cache, &p, 96, &Cancel::new()).unwrap();
    let b = cache.get(&p, 96).expect("en cache");
    assert_eq!(a, b);
    // fichier modifié → autre clé
    std::thread::sleep(Duration::from_millis(20));
    write("c.png", image::ImageFormat::Png);
    assert!(cache.get(&p, 96).is_none());
    assert_eq!(cache.prune(0), 1);
}

#[test]
fn pool_et_annulation() {
    let p = write("d.png", image::ImageFormat::Png);
    let mut pool = Pool::new(2, None, || {});
    let id = pool.request(&p, 64);
    let d = pool.recv_timeout(Duration::from_secs(5)).expect("résultat");
    assert_eq!(d.id, id);
    assert_eq!(d.result.unwrap().width(), 64);
    // demandes annulées : rien n'est livré
    let mut pool = Pool::new(1, None, || {});
    for _ in 0..50 {
        pool.request(&p, 0);
    }
    pool.cancel_pending();
    std::thread::sleep(Duration::from_millis(100));
    assert!(pool.poll().len() <= 1, "au plus le décodage déjà commencé");
    assert_eq!(pool.pending(), 0);
}

#[test]
fn erreurs_lisibles() {
    let e = decode(Path::new("C:/n/existe/pas.png"), &Cancel::new()).unwrap_err();
    assert!(e.contains("lecture"), "{e}");
    let p = dir().join("faux.png");
    std::fs::write(&p, b"pas une image").unwrap();
    assert!(decode(&p, &Cancel::new()).is_err());
}
