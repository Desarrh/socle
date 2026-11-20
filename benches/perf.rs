//! Benchmarks du socle : `cargo bench --features full` (ou `--features "fuzzy images"`).
//! Échoue (code 1) si un objectif de DIRECTION.md §2 n'est pas tenu :
//! - recherche / autocomplétion < 5 ms sur 100 000 entrées.
//!
//! Les temps d'images sont mesurés et affichés (pas d'objectif chiffré : ils tournent sur des
//! threads), avec et sans WIC selon les features.

use socle::Cancel;
use socle::autocomplete::Autocomplete;
use socle::fuzzy::Index;
use std::time::{Duration, Instant};

const MOTS: &[&str] = &[
    "explorateur", "lecteur", "universel", "téléchargements", "vidéos", "musique", "capture", "écran", "projet", "rapport", "facture",
    "minecraft", "steam", "library", "shader", "texture", "sprite", "niveau", "sauvegarde", "config", "réglages", "cache", "index",
    "main", "app", "ui", "theme", "icons", "build", "release", "debug", "target", "node_modules", "src", "assets", "dossier",
    "photo", "vacances", "été", "noël", "anniversaire", "montage", "short", "psychonight", "tube", "vigie", "ultimate", "launcher",
];
const EXTS: &[&str] = &["rs", "png", "jpg", "mp4", "txt", "md", "json", "toml", "psd", "exe", "dll", "zip", "wav", "pdf"];

/// Générateur pseudo-aléatoire déterministe (xorshift).
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> usize {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 11) as usize
    }
}

/// 100 000 noms réalistes (« Capture_écran_2024-07-12 vidéos montage.mp4 »), ~32 caractères.
fn dataset(n: usize) -> Vec<String> {
    let mut r = Rng(0x9E37_79B9_7F4A_7C15);
    (0..n)
        .map(|i| {
            let k = 1 + r.next() % 4;
            let mut s = String::new();
            for j in 0..k {
                let w = MOTS[r.next() % MOTS.len()];
                if j > 0 {
                    s.push(['_', ' ', '-', '.'][r.next() % 4]);
                }
                if r.next() % 3 == 0 {
                    let mut c = w.chars();
                    if let Some(f) = c.next() {
                        s.extend(f.to_uppercase());
                        s.push_str(c.as_str());
                    }
                } else {
                    s.push_str(w);
                }
            }
            if r.next() % 2 == 0 {
                s.push_str(&format!("_{}", i % 9973));
            }
            s.push('.');
            s.push_str(EXTS[r.next() % EXTS.len()]);
            s
        })
        .collect()
}

/// Médiane et maximum de `runs` exécutions (après 2 tours de chauffe).
fn time<T>(runs: usize, mut f: impl FnMut() -> T) -> (Duration, Duration, T) {
    let _ = f();
    let mut last = f();
    let mut v = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        last = f();
        v.push(t.elapsed());
    }
    v.sort();
    (v[runs / 2], v[runs - 1], last)
}

fn ms(d: Duration) -> String {
    format!("{:.2} ms", d.as_secs_f64() * 1000.0)
}

fn main() {
    // `cargo bench` passe « --bench » : on l'ignore.
    let mut ok = true;
    println!("== Recherche floue (objectif < 5 ms, 100 000 entrées) ==");
    let data = dataset(100_000);
    let avg = data.iter().map(|s| s.chars().count()).sum::<usize>() / data.len();
    let t = Instant::now();
    let ix = Index::from_iter(&data);
    println!("construction de l'index : {} (longueur moyenne {avg} car.)", ms(t.elapsed()));
    let queries = [
        ("e", "1 lettre (presque tout correspond)"),
        ("explo", "préfixe"),
        ("telechargements", "sans accents"),
        ("telechargemnts", "lettre oubliée"),
        ("tlchrgmnts", "lettres sautées"),
        ("exploarteur", "faute de frappe"),
        ("cev", "initiales (Capture Écran Vidéos)"),
        ("capture ecran", "deux mots"),
        ("zzqwx", "rien ne correspond"),
        ("psychonight short montage", "trois mots"),
    ];
    for (q, label) in queries {
        let (med, max, hits) = time(30, || ix.search(q, 50));
        let verdict = if max.as_secs_f64() * 1000.0 < 5.0 { "OK" } else { "TROP LENT" };
        if verdict != "OK" {
            ok = false;
        }
        let first = hits.first().map(|h| data[h.index].as_str()).unwrap_or("—");
        println!("{q:>26} ({label:<34}) médiane {:>9}  max {:>9}  {verdict:<9} → {first}", ms(med), ms(max));
    }

    // Référence sans parallélisme : 39 999 entrées (sous le seuil de 40 000).
    let mono = Index::from_iter(&data[..39_999]);
    let (med, max, _) = time(30, || mono.search("exploarteur", 50));
    println!("{:>26} (1 seul cœur, 39 999 entrées      ) médiane {:>9}  max {:>9}", "exploarteur", ms(med), ms(max));

    println!("\n== Autocomplétion (100 000 candidats + 200 entrées d'historique) ==");
    let mut ac = Autocomplete::default();
    ac.set_candidates(data.iter().cloned());
    for w in MOTS.iter().take(40) {
        for k in 0..5 {
            ac.history.record(&format!("{w} {k}"));
        }
    }
    for q in ["vid", "telech", "minecrft"] {
        let (med, max, s) = time(30, || ac.suggest(q, 8));
        let verdict = if max.as_secs_f64() * 1000.0 < 5.0 { "OK" } else { "TROP LENT" };
        if verdict != "OK" {
            ok = false;
        }
        println!("{q:>12} médiane {:>9}  max {:>9}  {verdict} → {}", ms(med), ms(max), s.first().map(|x| x.text.as_str()).unwrap_or("—"));
    }

    #[cfg(feature = "images")]
    images();

    if !ok {
        eprintln!("\nobjectif de 5 ms dépassé");
        std::process::exit(1);
    }
    println!("\nTous les objectifs sont tenus.");
}

#[cfg(feature = "images")]
fn images() {
    use socle::images;
    println!("\n== Images (1 thread ; le pool en utilise 2 à 4) ==");
    let dir = std::env::temp_dir().join("socle-bench-images");
    std::fs::create_dir_all(&dir).ok();
    let photo = image::RgbImage::from_fn(6000, 4000, |x, y| image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x ^ y) % 256) as u8]));
    let jpg = dir.join("photo-24mpx.jpg");
    if !jpg.exists() {
        image::DynamicImage::ImageRgb8(photo.clone()).save(&jpg).unwrap();
    }
    let png = dir.join("capture-4k.png");
    if !png.exists() {
        image::imageops::thumbnail(&photo, 3840, 2160).save(&png).unwrap();
    }
    let psd = dir.join("dessin-4000.psd");
    if !psd.exists() {
        let small = image::imageops::thumbnail(&photo, 4000, 3000);
        std::fs::write(&psd, images::testdata::psd_rgb_rle(&small)).unwrap();
    }
    let c = Cancel::new();
    for (name, p) in [("JPEG 24 Mpx", &jpg), ("PNG 3840×2160", &png), ("PSD 4000×3000 (RLE)", &psd)] {
        let (full, _, img) = time(3, || images::decode(p, &c).unwrap());
        let (th, _, t) = time(5, || images::thumbnail(p, 256, &c).unwrap());
        println!("{name:<22} complète {:>10} ({}×{})   miniature 256 {:>10} ({}×{})", ms(full), img.width(), img.height(), ms(th), t.width(), t.height());
    }
    let cache = images::DiskCache { dir: dir.join("cache") };
    let _ = images::thumbnail_cached(&cache, &jpg, 256, &c);
    let (hit, _, _) = time(20, || images::thumbnail_cached(&cache, &jpg, 256, &c).unwrap());
    println!("{:<22} {:>10}", "miniature depuis le cache disque (QOI)", ms(hit));
    #[cfg(feature = "img-svg")]
    {
        let svg = dir.join("logo.svg");
        std::fs::write(&svg, r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><circle cx="12" cy="12" r="10" fill="#2E5E9E"/><path d="M8 6l9 6-9 6z" fill="#F5EDDC"/></svg>"##).unwrap();
        let (t, _, _) = time(10, || images::thumbnail(&svg, 256, &c).unwrap());
        println!("{:<22} {:>10}", "SVG → 256 px", ms(t));
    }
}
