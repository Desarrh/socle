//! Icônes vectorielles dessinées au pinceau egui : nettes à toutes les tailles, toujours centrées
//! (les glyphes Unicode/emoji ne le sont pas), colorées par le thème.
//!
//! Fusion de `lecteur-universel/src/icons.rs` et `tube/src/icons.rs`, plus les icônes qui
//! remplacent les emoji de l'explorateur et de Vigie. Repère unique : **grille 24 × 24**,
//! trait de **1,8** (mis à l'échelle, au moins 1,2 px). Les icônes reprises de Tube gardent leur
//! dessin d'origine, exprimé dans un repère centré (`u`) converti vers la grille 24.
//!
//! Coût : rien n'est alloué hors des `Vec<Pos2>` des polylignes ; une icône = 1 à 12 formes.

use egui::{Color32, CornerRadius, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind, Ui, Vec2, vec2};
use std::f32::consts::PI;

/// Toutes les icônes. `VolumeLevel(n)` : 0, 1 ou 2 ondes.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Icon {
    // --- lecture (lecteur + Tube) ---
    Play,
    Pause,
    Stop,
    StepBack,
    StepFwd,
    Next,
    Back5,
    Fwd5,
    Back10,
    Fwd10,
    Volume,
    VolumeLevel(u8),
    Mute,
    Fullscreen,
    Unfullscreen,
    Mini,
    ExitMini,
    Half,
    Subs,
    Loop,
    MarkIn,
    MarkOut,
    Camera,
    // --- édition d'image (lecteur) ---
    Scissors,
    RotL,
    RotR,
    FlipH,
    FlipV,
    Crop,
    Resize,
    Convert,
    Clean,
    // --- actions générales ---
    Undo,
    Reset,
    Save,
    Open,
    Copy,
    Check,
    Plus,
    Close,
    More,
    Panel,
    Drag,
    Refresh,
    Search,
    SearchDeep,
    Help,
    Settings,
    Home,
    Clock,
    History,
    Playlist,
    Trash,
    Edit,
    Link,
    Download,
    Upload,
    Filter,
    Eye,
    Star,
    Pin,
    Command,
    Keyboard,
    Terminal,
    Code,
    Warning,
    Info,
    Dot,
    // --- navigation ---
    Back,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    ChevronLeft,
    ChevronRight,
    ChevronUp,
    ChevronDown,
    // --- vues ---
    Grid,
    ListView,
    Tree,
    Sun,
    Moon,
    // --- fichiers (explorateur) ---
    Folder,
    File,
    Image,
    TextFile,
    Audio,
    Video,
    Archive,
    Pdf,
    Drive,
    Git,
    // --- système (Vigie) ---
    Power,
    Sleep,
    Block,
    Gamepad,
    Container,
    Ghost,
    Chart,
    Shield,
    Broom,
    Bolt,
    Cpu,
    Memory,
    // --- marque ---
    Logo,
}

impl Icon {
    /// Liste complète (démo, tests).
    pub const ALL: &'static [Icon] = &[
        Icon::Play, Icon::Pause, Icon::Stop, Icon::StepBack, Icon::StepFwd, Icon::Next, Icon::Back5, Icon::Fwd5,
        Icon::Back10, Icon::Fwd10, Icon::Volume, Icon::VolumeLevel(0), Icon::VolumeLevel(1), Icon::VolumeLevel(2),
        Icon::Mute, Icon::Fullscreen, Icon::Unfullscreen, Icon::Mini, Icon::ExitMini, Icon::Half, Icon::Subs,
        Icon::Loop, Icon::MarkIn, Icon::MarkOut, Icon::Camera, Icon::Scissors, Icon::RotL, Icon::RotR, Icon::FlipH,
        Icon::FlipV, Icon::Crop, Icon::Resize, Icon::Convert, Icon::Clean, Icon::Undo, Icon::Reset, Icon::Save,
        Icon::Open, Icon::Copy, Icon::Check, Icon::Plus, Icon::Close, Icon::More, Icon::Panel, Icon::Drag,
        Icon::Refresh, Icon::Search, Icon::SearchDeep, Icon::Help, Icon::Settings, Icon::Home, Icon::Clock,
        Icon::History, Icon::Playlist, Icon::Trash, Icon::Edit, Icon::Link, Icon::Download, Icon::Upload,
        Icon::Filter, Icon::Eye, Icon::Star, Icon::Pin, Icon::Command, Icon::Keyboard, Icon::Terminal, Icon::Code,
        Icon::Warning, Icon::Info, Icon::Dot, Icon::Back, Icon::ArrowLeft, Icon::ArrowRight, Icon::ArrowUp,
        Icon::ArrowDown, Icon::ChevronLeft, Icon::ChevronRight, Icon::ChevronUp, Icon::ChevronDown, Icon::Grid,
        Icon::ListView, Icon::Tree, Icon::Sun, Icon::Moon, Icon::Folder, Icon::File, Icon::Image, Icon::TextFile,
        Icon::Audio, Icon::Video, Icon::Archive, Icon::Pdf, Icon::Drive, Icon::Git, Icon::Power, Icon::Sleep,
        Icon::Block, Icon::Gamepad, Icon::Container, Icon::Ghost, Icon::Chart, Icon::Shield, Icon::Broom, Icon::Bolt,
        Icon::Cpu, Icon::Memory, Icon::Logo,
    ];

    /// Icône d'un fichier d'après son extension (remplace `fsx::icon_for` de l'explorateur).
    pub fn for_extension(ext: &str) -> Icon {
        let e = ext.trim_start_matches('.').to_ascii_lowercase();
        match e.as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "tif" | "tiff" | "tga" | "dds" | "psd" | "exr"
            | "hdr" | "qoi" | "svg" | "avif" | "heic" | "heif" | "jxl" | "jxr" | "dng" | "cr2" | "cr3" | "nef" | "arw"
            | "raf" | "orf" | "rw2" => Icon::Image,
            "mp3" | "wav" | "flac" | "ogg" | "opus" | "m4a" | "aac" | "wma" => Icon::Audio,
            "mp4" | "mkv" | "webm" | "mov" | "avi" | "wmv" | "m4v" | "mts" => Icon::Video,
            "zip" | "7z" | "rar" | "tar" | "gz" | "xz" | "zst" | "bz2" | "cab" => Icon::Archive,
            "pdf" => Icon::Pdf,
            "exe" | "msi" | "bat" | "cmd" | "ps1" | "sh" => Icon::Terminal,
            "rs" | "js" | "ts" | "tsx" | "jsx" | "py" | "c" | "h" | "cpp" | "hpp" | "cs" | "go" | "java" | "kt"
            | "html" | "css" | "json" | "toml" | "yaml" | "yml" | "xml" | "lua" | "gd" | "sql" => Icon::Code,
            "txt" | "md" | "log" | "ini" | "cfg" | "csv" | "doc" | "docx" | "rtf" | "odt" => Icon::TextFile,
            "dll" | "sys" => Icon::Settings,
            _ => Icon::File,
        }
    }
}

/// Crayon : convertit les coordonnées vers l'écran et trace avec la couleur et l'épaisseur du thème.
#[derive(Clone, Copy)]
struct Pen<'a> {
    p: &'a Painter,
    /// Coin haut-gauche de la grille 24.
    o: Pos2,
    /// Taille d'une unité de grille en pixels.
    s: f32,
    col: Color32,
    w: f32,
}

/// Une unité du repère centré de Tube, en unités de grille. Tube dessinait à 24 × 0,31 = 7,44 ;
/// on agrandit à 8,8 pour que ses icônes aient la même taille optique que celles du lecteur
/// (qui occupent ~3,5–20,5 de la grille).
const TU: f32 = 8.8;

impl Pen<'_> {
    /// Point de la grille 24.
    fn q(&self, x: f32, y: f32) -> Pos2 {
        self.o + vec2(x * self.s, y * self.s)
    }
    /// Point du repère centré de Tube (-1..1 ≈ 62 % du côté).
    fn t(&self, x: f32, y: f32) -> Pos2 {
        self.q(12.0 + x * TU, 12.0 + y * TU)
    }
    fn stroke(&self) -> Stroke {
        Stroke::new(self.w, self.col)
    }
    fn line(&self, a: (f32, f32), b: (f32, f32)) {
        self.p.line_segment([self.q(a.0, a.1), self.q(b.0, b.1)], self.stroke());
    }
    fn poly(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.q(x, y)).collect();
        self.p.add(Shape::line(v, self.stroke()));
    }
    fn closed(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.q(x, y)).collect();
        self.p.add(Shape::closed_line(v, self.stroke()));
    }
    fn fill(&self, pts: &[(f32, f32)]) {
        let v: Vec<Pos2> = pts.iter().map(|&(x, y)| self.q(x, y)).collect();
        self.p.add(Shape::convex_polygon(v, self.col, Stroke::NONE));
    }
    fn rect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let rr = Rect::from_min_max(self.q(x0, y0), self.q(x1, y1));
        self.p.rect_stroke(rr, CornerRadius::same((r * self.s) as u8), self.stroke(), StrokeKind::Middle);
    }
    fn rect_fill(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32) {
        let rr = Rect::from_min_max(self.q(x0, y0), self.q(x1, y1));
        self.p.rect_filled(rr, CornerRadius::same((r * self.s) as u8), self.col);
    }
    fn circle(&self, x: f32, y: f32, r: f32) {
        self.p.circle_stroke(self.q(x, y), r * self.s, self.stroke());
    }
    fn dot(&self, x: f32, y: f32, r: f32) {
        self.p.circle_filled(self.q(x, y), r * self.s, self.col);
    }
    /// Points d'un arc (degrés, 0 = droite, sens horaire à l'écran).
    fn arc_pts(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
        let n = (((a1 - a0).abs() / 9.0).ceil() as usize).max(4);
        (0..=n)
            .map(|i| {
                let a = (a0 + (a1 - a0) * i as f32 / n as f32).to_radians();
                (cx + r * a.cos(), cy + r * a.sin())
            })
            .collect()
    }
    fn arc(&self, cx: f32, cy: f32, r: f32, a0: f32, a1: f32) {
        self.poly(&self.arc_pts(cx, cy, r, a0, a1));
    }
    /// Pointe de flèche pleine en (x, y) orientée selon (dx, dy), taille `l` (grille).
    fn head(&self, x: f32, y: f32, dx: f32, dy: f32, l: f32) {
        let n = (dx * dx + dy * dy).sqrt().max(1e-3);
        let (dx, dy) = (dx / n, dy / n);
        let (nx, ny) = (-dy, dx);
        self.fill(&[(x + dx * l, y + dy * l), (x + nx * l * 0.8, y + ny * l * 0.8), (x - nx * l * 0.8, y - ny * l * 0.8)]);
    }
    fn text(&self, x: f32, y: f32, t: &str, size: f32, family: egui::FontFamily) {
        self.p.text(self.q(x, y), egui::Align2::CENTER_CENTER, t, FontId::new(size * self.s, family), self.col);
    }
    // --- primitives du repère de Tube ---
    fn tline(&self, pts: &[(f32, f32)]) {
        let v: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (12.0 + x * TU, 12.0 + y * TU)).collect();
        self.poly(&v);
    }
    fn tfill(&self, pts: &[(f32, f32)]) {
        let v: Vec<(f32, f32)> = pts.iter().map(|&(x, y)| (12.0 + x * TU, 12.0 + y * TU)).collect();
        self.fill(&v);
    }
    fn trect(&self, x0: f32, y0: f32, x1: f32, y1: f32, r: f32, filled: bool) {
        let (a, b, c, d) = (12.0 + x0 * TU, 12.0 + y0 * TU, 12.0 + x1 * TU, 12.0 + y1 * TU);
        if filled { self.rect_fill(a, b, c, d, r * TU) } else { self.rect(a, b, c, d, r * TU) }
    }
    fn tcircle(&self, x: f32, y: f32, r: f32) {
        self.circle(12.0 + x * TU, 12.0 + y * TU, r * TU);
    }
    fn tarc(&self, x: f32, y: f32, r: f32, a0: f32, a1: f32) -> Vec<(f32, f32)> {
        self.arc_pts(x, y, r, a0, a1)
    }
    fn thead(&self, x: f32, y: f32, dx: f32, dy: f32, l: f32) {
        self.head(12.0 + x * TU, 12.0 + y * TU, dx, dy, l * TU);
    }
}

fn bold() -> egui::FontFamily {
    egui::FontFamily::Proportional
}

/// Peint `icon` centrée dans `rect` (grille 24 × 24 mise à l'échelle sur le plus petit côté).
pub fn paint(p: &Painter, rect: Rect, icon: Icon, c: Color32) {
    let s = rect.width().min(rect.height()) / 24.0;
    let o = rect.center() - vec2(12.0 * s, 12.0 * s);
    let pen = Pen { p, o, s, col: c, w: (1.8 * s).max(1.2) };
    draw(&pen, icon);
}

fn draw(pen: &Pen, icon: Icon) {
    let s = pen.s;
    let c = pen.col;
    let p = pen.p;
    match icon {
        // ================= lecteur =================
        Icon::Play => pen.fill(&[(8.0, 5.5), (19.0, 12.0), (8.0, 18.5)]),
        Icon::Pause => {
            pen.rect_fill(7.0, 5.5, 10.5, 18.5, 1.0);
            pen.rect_fill(13.5, 5.5, 17.0, 18.5, 1.0);
        }
        Icon::StepBack => {
            pen.rect_fill(6.0, 6.5, 8.2, 17.5, 0.0);
            pen.fill(&[(18.0, 6.5), (9.5, 12.0), (18.0, 17.5)]);
        }
        Icon::StepFwd => {
            pen.rect_fill(15.8, 6.5, 18.0, 17.5, 0.0);
            pen.fill(&[(6.0, 6.5), (14.5, 12.0), (6.0, 17.5)]);
        }
        Icon::Back5 | Icon::Fwd5 => {
            let fwd = icon == Icon::Fwd5;
            if fwd { pen.arc(12.0, 13.0, 8.0, -70.0, 200.0) } else { pen.arc(12.0, 13.0, 8.0, -20.0, 250.0) }
            let tip = if fwd { (14.7, 5.4) } else { (9.3, 5.4) };
            let dir = if fwd { 1.0 } else { -1.0 };
            pen.poly(&[(tip.0 - 3.2 * dir, 2.6), (tip.0 + 0.6 * dir, tip.1), (tip.0 - 3.0 * dir, 8.4)]);
            pen.text(12.0, 13.4, "5", 9.5, egui::FontFamily::Name("bold".into()).clone_or(bold()));
        }
        Icon::Camera => {
            pen.poly(&[(3.5, 8.0), (7.5, 8.0), (9.0, 5.5), (15.0, 5.5), (16.5, 8.0), (20.5, 8.0), (20.5, 18.5), (3.5, 18.5), (3.5, 8.0)]);
            pen.circle(12.0, 13.0, 3.4);
        }
        Icon::Volume | Icon::Mute => {
            pen.fill(&[(4.0, 9.5), (7.5, 9.5), (12.0, 5.5), (12.0, 18.5), (7.5, 14.5), (4.0, 14.5)]);
            if icon == Icon::Volume {
                pen.arc(13.0, 12.0, 3.5, -50.0, 50.0);
                pen.arc(13.0, 12.0, 6.5, -50.0, 50.0);
            } else {
                pen.line((15.0, 9.0), (21.0, 15.0));
                pen.line((21.0, 9.0), (15.0, 15.0));
            }
        }
        Icon::Fullscreen => {
            for pts in [[(4.0, 9.0), (4.0, 4.0), (9.0, 4.0)], [(15.0, 4.0), (20.0, 4.0), (20.0, 9.0)], [(20.0, 15.0), (20.0, 20.0), (15.0, 20.0)], [(9.0, 20.0), (4.0, 20.0), (4.0, 15.0)]] {
                pen.poly(&pts);
            }
        }
        Icon::Unfullscreen => {
            for pts in [[(4.0, 9.0), (9.0, 9.0), (9.0, 4.0)], [(15.0, 4.0), (15.0, 9.0), (20.0, 9.0)], [(20.0, 15.0), (15.0, 15.0), (15.0, 20.0)], [(9.0, 20.0), (9.0, 15.0), (4.0, 15.0)]] {
                pen.poly(&pts);
            }
        }
        Icon::Scissors => {
            pen.circle(6.5, 6.5, 2.8);
            pen.circle(6.5, 17.5, 2.8);
            pen.line((8.6, 8.4), (20.0, 18.5));
            pen.line((8.6, 15.6), (20.0, 5.5));
        }
        Icon::RotL | Icon::RotR => {
            let r = icon == Icon::RotR;
            pen.arc(12.0, 13.0, 7.0, if r { -80.0 } else { -100.0 }, if r { 200.0 } else { -380.0 });
            let (x, d) = if r { (12.5, 1.0) } else { (11.5, -1.0) };
            pen.fill(&[(x - 0.5 * d, 2.8), (x + 3.5 * d, 6.0), (x - 0.5 * d, 9.2)]);
        }
        Icon::FlipH => {
            pen.line((12.0, 3.5), (12.0, 20.5));
            pen.poly(&[(9.5, 6.0), (3.5, 18.0), (9.5, 18.0), (9.5, 6.0)]);
            pen.fill(&[(14.5, 6.0), (20.5, 18.0), (14.5, 18.0)]);
        }
        Icon::FlipV => {
            pen.line((3.5, 12.0), (20.5, 12.0));
            pen.poly(&[(6.0, 9.5), (18.0, 3.5), (18.0, 9.5), (6.0, 9.5)]);
            pen.fill(&[(6.0, 14.5), (18.0, 20.5), (18.0, 14.5)]);
        }
        Icon::Crop => {
            pen.poly(&[(7.0, 2.5), (7.0, 17.0), (21.5, 17.0)]);
            pen.poly(&[(2.5, 7.0), (17.0, 7.0), (17.0, 21.5)]);
        }
        Icon::Resize => {
            pen.poly(&[(14.0, 4.0), (20.0, 4.0), (20.0, 10.0)]);
            pen.poly(&[(10.0, 20.0), (4.0, 20.0), (4.0, 14.0)]);
            pen.line((20.0, 4.0), (13.5, 10.5));
            pen.line((4.0, 20.0), (10.5, 13.5));
        }
        Icon::Undo | Icon::Reset => {
            pen.arc(13.0, 13.0, 6.5, 200.0, 450.0);
            pen.fill(&[(3.0, 9.5), (6.5, 14.5), (10.5, 10.0)]);
        }
        Icon::Save => {
            pen.poly(&[(4.5, 4.5), (16.5, 4.5), (19.5, 7.5), (19.5, 19.5), (4.5, 19.5), (4.5, 4.5)]);
            pen.poly(&[(8.0, 4.5), (8.0, 9.0), (15.0, 9.0), (15.0, 4.5)]);
            pen.rect(8.0, 13.0, 16.0, 19.5, 0.0);
        }
        Icon::Folder | Icon::Open => {
            pen.poly(&[(3.5, 6.5), (9.5, 6.5), (11.5, 8.5), (20.5, 8.5), (20.5, 18.5), (3.5, 18.5), (3.5, 6.5)]);
            if icon == Icon::Open {
                pen.line((12.0, 16.5), (12.0, 11.0));
                pen.poly(&[(9.5, 13.0), (12.0, 10.5), (14.5, 13.0)]);
            }
        }
        Icon::Plus => {
            pen.line((12.0, 5.0), (12.0, 19.0));
            pen.line((5.0, 12.0), (19.0, 12.0));
        }
        Icon::Close => {
            pen.line((6.5, 6.5), (17.5, 17.5));
            pen.line((17.5, 6.5), (6.5, 17.5));
        }
        Icon::Moon => moon(pen),
        Icon::Sun => {
            pen.circle(12.0, 12.0, 4.0);
            for i in 0..8 {
                let a = (i as f32 * 45.0f32).to_radians();
                pen.line((12.0 + 6.8 * a.cos(), 12.0 + 6.8 * a.sin()), (12.0 + 9.0 * a.cos(), 12.0 + 9.0 * a.sin()));
            }
        }
        Icon::More => {
            for x in [6.0, 12.0, 18.0] {
                pen.dot(x, 12.0, 1.7);
            }
        }
        Icon::Panel => {
            pen.rect(3.5, 5.0, 20.5, 19.0, 2.0);
            pen.line((14.5, 5.0), (14.5, 19.0));
        }
        Icon::Convert => {
            pen.line((4.0, 8.5), (18.0, 8.5));
            pen.poly(&[(14.5, 5.0), (18.0, 8.5), (14.5, 12.0)]);
            pen.line((20.0, 15.5), (6.0, 15.5));
            pen.poly(&[(9.5, 12.0), (6.0, 15.5), (9.5, 19.0)]);
        }
        Icon::MarkIn => {
            pen.poly(&[(10.0, 4.0), (6.0, 4.0), (6.0, 20.0), (10.0, 20.0)]);
            pen.fill(&[(11.0, 7.5), (18.0, 12.0), (11.0, 16.5)]);
        }
        Icon::MarkOut => {
            pen.poly(&[(14.0, 4.0), (18.0, 4.0), (18.0, 20.0), (14.0, 20.0)]);
            pen.fill(&[(13.0, 7.5), (6.0, 12.0), (13.0, 16.5)]);
        }
        Icon::Loop => {
            pen.poly(&[(5.0, 13.0), (5.0, 9.0), (17.5, 9.0)]);
            pen.poly(&[(15.0, 6.0), (18.5, 9.0), (15.0, 12.0)]);
            pen.poly(&[(19.0, 11.0), (19.0, 15.0), (6.5, 15.0)]);
            pen.poly(&[(9.0, 12.0), (5.5, 15.0), (9.0, 18.0)]);
        }
        Icon::Copy => {
            pen.rect(8.5, 8.5, 19.5, 19.5, 2.0);
            pen.poly(&[(5.0, 15.5), (4.5, 15.5), (4.5, 4.5), (15.5, 4.5), (15.5, 5.0)]);
        }
        Icon::Check => pen.poly(&[(5.0, 12.5), (10.0, 17.5), (19.5, 7.0)]),
        Icon::Drag => {
            for (x, y) in [(9.0, 6.0), (15.0, 6.0), (9.0, 12.0), (15.0, 12.0), (9.0, 18.0), (15.0, 18.0)] {
                pen.dot(x, y, 1.6);
            }
        }
        Icon::Logo => {
            pen.rect_fill(3.0, 3.0, 21.0, 21.0, 5.0);
            let v = vec![pen.q(9.5, 7.5), pen.q(17.0, 12.0), pen.q(9.5, 16.5)];
            p.add(Shape::convex_polygon(v, Color32::from_rgb(0xF5, 0xED, 0xDC), Stroke::NONE));
        }
        Icon::Clean => {
            pen.rect(3.5, 5.0, 20.5, 19.0, 2.0);
            pen.poly(&[(8.0, 10.0), (8.0, 8.0), (10.0, 8.0)]);
            pen.poly(&[(16.0, 14.0), (16.0, 16.0), (14.0, 16.0)]);
        }
        // ================= Tube (repère centré) =================
        Icon::Back10 | Icon::Fwd10 => {
            let m = if icon == Icon::Back10 { -1.0 } else { 1.0 };
            let pts: Vec<(f32, f32)> = pen.tarc(0.0, 0.05, 0.9, -90.0, 180.0).into_iter().map(|(x, y)| (x * -m, y)).collect();
            pen.tline(&pts);
            pen.thead(0.0, -0.85, m, 0.0, 0.32);
            p.text(pen.t(0.0, 0.1), egui::Align2::CENTER_CENTER, "10", FontId::proportional(TU * s * 0.78), c);
        }
        Icon::Next => {
            pen.tfill(&[(-0.75, -0.8), (0.45, 0.0), (-0.75, 0.8)]);
            pen.trect(0.5, -0.8, 0.78, 0.8, 0.08, true);
        }
        Icon::VolumeLevel(l) => {
            pen.trect(-0.95, -0.32, -0.5, 0.32, 0.06, true);
            pen.tfill(&[(-0.55, -0.32), (-0.05, -0.8), (-0.05, 0.8), (-0.55, 0.32)]);
            if l >= 1 {
                pen.tline(&pen.tarc(-0.05, 0.0, 0.42, -45.0, 45.0));
            }
            if l >= 2 {
                pen.tline(&pen.tarc(-0.05, 0.0, 0.85, -48.0, 48.0));
            }
        }
        Icon::Mini | Icon::ExitMini => {
            pen.trect(-0.95, -0.72, 0.95, 0.72, 0.12, false);
            if icon == Icon::Mini {
                pen.trect(0.05, 0.02, 0.72, 0.5, 0.06, true);
            } else {
                pen.tline(&[(0.5, 0.42), (-0.35, -0.3)]);
                pen.thead(-0.42, -0.36, -1.0, -0.85, 0.3);
            }
        }
        Icon::Half => {
            pen.trect(-0.95, -0.72, 0.95, 0.72, 0.12, false);
            pen.trect(-0.95, -0.72, 0.0, 0.72, 0.12, true);
        }
        Icon::Stop => pen.trect(-0.62, -0.62, 0.62, 0.62, 0.12, true),
        Icon::Back | Icon::ArrowLeft => {
            pen.tline(&[(0.85, 0.0), (-0.8, 0.0)]);
            pen.tline(&[(-0.15, -0.65), (-0.82, 0.0), (-0.15, 0.65)]);
        }
        Icon::Refresh => {
            pen.tline(&pen.tarc(0.0, 0.0, 0.82, -40.0, 270.0));
            pen.thead(0.0, -0.82, 1.0, 0.0, 0.32);
        }
        Icon::Search => {
            pen.tcircle(-0.18, -0.18, 0.58);
            pen.tline(&[(0.25, 0.25), (0.85, 0.85)]);
        }
        Icon::Help => {
            pen.tcircle(0.0, 0.0, 0.92);
            p.text(pen.t(0.0, 0.04), egui::Align2::CENTER_CENTER, "?", FontId::proportional(TU * s * 1.25), c);
        }
        Icon::Settings => {
            pen.tcircle(0.0, 0.0, 0.3);
            pen.tcircle(0.0, 0.0, 0.62);
            let thick = Pen { w: pen.w * 1.9, ..*pen };
            for i in 0..8 {
                let a = i as f32 * PI / 4.0;
                thick.tline(&[(0.66 * a.cos(), 0.66 * a.sin()), (0.92 * a.cos(), 0.92 * a.sin())]);
            }
        }
        Icon::Home => {
            pen.tline(&[(-0.95, -0.02), (0.0, -0.85), (0.95, -0.02)]);
            pen.tline(&[(-0.62, -0.28), (-0.62, 0.82), (0.62, 0.82), (0.62, -0.28)]);
            pen.tline(&[(-0.18, 0.82), (-0.18, 0.3), (0.18, 0.3), (0.18, 0.82)]);
        }
        Icon::Subs => {
            pen.trect(-0.92, -0.5, 0.92, 0.82, 0.14, false);
            pen.tline(&[(-0.6, -0.78), (0.6, -0.78)]);
            pen.tfill(&[(-0.22, -0.12), (0.34, 0.16), (-0.22, 0.44)]);
        }
        Icon::Clock => {
            pen.tcircle(0.0, 0.0, 0.88);
            pen.tline(&[(0.0, -0.5), (0.0, 0.0), (0.4, 0.25)]);
        }
        Icon::Playlist => {
            for (i, y) in [-0.6f32, 0.0, 0.6].into_iter().enumerate() {
                p.circle_filled(pen.t(-0.78, y), 0.11 * TU * s, c);
                let x1 = if i == 2 { 0.35 } else { 0.9 };
                pen.tline(&[(-0.42, y), (x1, y)]);
            }
            pen.tfill(&[(0.5, 0.38), (0.95, 0.62), (0.5, 0.86)]);
        }
        Icon::History => {
            pen.tline(&pen.tarc(0.05, 0.0, 0.85, -170.0, 150.0));
            pen.thead(-0.8, -0.12, -0.35, 1.0, 0.3);
            pen.tline(&[(0.05, -0.45), (0.05, 0.0), (0.38, 0.22)]);
        }
        // ================= nouvelles (explorateur, Vigie) =================
        Icon::ArrowRight => {
            pen.line((5.5, 12.0), (18.0, 12.0));
            pen.poly(&[(13.0, 7.0), (18.2, 12.0), (13.0, 17.0)]);
        }
        Icon::ArrowUp => {
            pen.line((12.0, 18.5), (12.0, 6.0));
            pen.poly(&[(7.0, 11.0), (12.0, 5.8), (17.0, 11.0)]);
        }
        Icon::ArrowDown => {
            pen.line((12.0, 5.5), (12.0, 18.0));
            pen.poly(&[(7.0, 13.0), (12.0, 18.2), (17.0, 13.0)]);
        }
        Icon::ChevronLeft => pen.poly(&[(14.5, 6.0), (8.5, 12.0), (14.5, 18.0)]),
        Icon::ChevronRight => pen.poly(&[(9.5, 6.0), (15.5, 12.0), (9.5, 18.0)]),
        Icon::ChevronUp => pen.poly(&[(6.0, 14.5), (12.0, 8.5), (18.0, 14.5)]),
        Icon::ChevronDown => pen.poly(&[(6.0, 9.5), (12.0, 15.5), (18.0, 9.5)]),
        Icon::SearchDeep => {
            pen.circle(10.0, 10.0, 5.5);
            pen.line((14.0, 14.0), (19.0, 19.0));
            pen.poly(&[(8.0, 8.5), (8.0, 12.0), (12.0, 12.0)]);
            pen.head(12.6, 12.0, 1.0, 0.0, 1.6);
        }
        Icon::Grid => {
            for (x, y) in [(4.0, 4.0), (13.5, 4.0), (4.0, 13.5), (13.5, 13.5)] {
                pen.rect(x, y, x + 6.5, y + 6.5, 1.5);
            }
        }
        Icon::ListView => {
            for y in [6.5, 12.0, 17.5] {
                pen.dot(5.0, y, 1.3);
                pen.line((8.5, y), (20.0, y));
            }
        }
        Icon::Tree => {
            pen.rect(3.5, 3.5, 9.5, 8.5, 1.2);
            pen.rect(13.5, 10.0, 20.5, 14.5, 1.2);
            pen.rect(13.5, 16.5, 20.5, 21.0, 1.2);
            pen.poly(&[(6.5, 8.5), (6.5, 18.75), (13.5, 18.75)]);
            pen.line((6.5, 12.25), (13.5, 12.25));
        }
        Icon::File => file_outline(pen),
        Icon::TextFile => {
            file_outline(pen);
            for (y, x1) in [(12.0, 15.5), (15.0, 15.5), (18.0, 12.5)] {
                pen.line((8.5, y), (x1, y));
            }
        }
        Icon::Code => {
            pen.poly(&[(8.0, 7.0), (3.5, 12.0), (8.0, 17.0)]);
            pen.poly(&[(16.0, 7.0), (20.5, 12.0), (16.0, 17.0)]);
            pen.line((13.5, 5.0), (10.5, 19.0));
        }
        Icon::Image => {
            pen.rect(3.5, 4.5, 20.5, 19.5, 2.0);
            pen.circle(9.0, 9.5, 1.8);
            pen.poly(&[(4.0, 17.5), (9.5, 12.5), (13.0, 15.5), (15.5, 13.0), (20.0, 17.0)]);
        }
        Icon::Audio => {
            pen.poly(&[(9.0, 17.5), (9.0, 5.5), (19.0, 3.8), (19.0, 15.5)]);
            pen.line((9.0, 9.0), (19.0, 7.3));
            p.circle_filled(pen.q(6.6, 17.5), 2.6 * s, c);
            p.circle_filled(pen.q(16.6, 15.5), 2.6 * s, c);
        }
        Icon::Video => {
            pen.rect(3.5, 5.0, 20.5, 19.0, 2.0);
            for y in [8.0, 12.0, 16.0] {
                pen.dot(6.0, y, 0.9);
                pen.dot(18.0, y, 0.9);
            }
            pen.fill(&[(10.0, 9.0), (14.8, 12.0), (10.0, 15.0)]);
        }
        Icon::Archive => {
            pen.rect(4.0, 7.5, 20.0, 20.0, 1.5);
            pen.poly(&[(4.0, 7.5), (6.5, 4.0), (17.5, 4.0), (20.0, 7.5)]);
            pen.line((12.0, 4.0), (12.0, 7.5));
            pen.rect(10.0, 10.5, 14.0, 13.5, 0.5);
        }
        Icon::Pdf => {
            file_outline(pen);
            pen.text(12.0, 15.2, "PDF", 5.6, egui::FontFamily::Name("bold".into()).clone_or(bold()));
        }
        Icon::Drive => {
            pen.rect(3.0, 7.0, 21.0, 17.0, 2.5);
            pen.line((3.0, 13.0), (21.0, 13.0));
            pen.dot(17.5, 15.0, 1.0);
            pen.line((6.0, 15.0), (11.0, 15.0));
        }
        Icon::Git => {
            pen.circle(7.0, 5.5, 2.2);
            pen.circle(7.0, 18.5, 2.2);
            pen.circle(17.0, 8.5, 2.2);
            pen.line((7.0, 7.7), (7.0, 16.3));
            let mut pts = vec![(17.0, 10.7)];
            pts.extend(pen.arc_pts(11.5, 10.7, 5.5, 0.0, 90.0));
            pts.push((7.0, 16.2));
            pen.poly(&pts);
        }
        Icon::Pin => {
            pen.closed(&[(9.0, 3.5), (15.0, 3.5), (14.0, 9.0), (17.5, 13.5), (6.5, 13.5), (10.0, 9.0)]);
            pen.line((12.0, 13.5), (12.0, 20.5));
        }
        Icon::Trash => {
            pen.line((4.0, 6.5), (20.0, 6.5));
            pen.poly(&[(9.0, 6.5), (9.5, 3.8), (14.5, 3.8), (15.0, 6.5)]);
            pen.poly(&[(6.0, 6.5), (7.2, 20.0), (16.8, 20.0), (18.0, 6.5)]);
            pen.line((10.0, 10.0), (10.3, 16.5));
            pen.line((14.0, 10.0), (13.7, 16.5));
        }
        Icon::Edit => {
            pen.closed(&[(15.5, 4.5), (19.5, 8.5), (9.0, 19.0), (4.5, 19.5), (5.0, 15.0)]);
            pen.line((13.5, 6.5), (17.5, 10.5));
        }
        Icon::Link => {
            pen.rect(2.8, 8.8, 13.5, 15.2, 3.2);
            pen.rect(10.5, 8.8, 21.2, 15.2, 3.2);
        }
        Icon::Download | Icon::Upload => {
            if icon == Icon::Download {
                pen.line((12.0, 3.5), (12.0, 14.5));
                pen.poly(&[(7.5, 10.0), (12.0, 14.5), (16.5, 10.0)]);
            } else {
                pen.line((12.0, 14.5), (12.0, 3.5));
                pen.poly(&[(7.5, 8.0), (12.0, 3.5), (16.5, 8.0)]);
            }
            pen.poly(&[(4.0, 15.0), (4.0, 19.5), (20.0, 19.5), (20.0, 15.0)]);
        }
        Icon::Filter => pen.closed(&[(3.5, 5.0), (20.5, 5.0), (14.0, 12.5), (14.0, 19.0), (10.0, 17.0), (10.0, 12.5)]),
        Icon::Eye => {
            // amande : arc du haut (4,12)→(20,12), puis arc du bas en retour
            let mut pts = pen.arc_pts(12.0, 18.0, 10.0, 216.87, 323.13);
            pts.extend(pen.arc_pts(12.0, 6.0, 10.0, 36.87, 143.13));
            pen.closed(&pts);
            pen.circle(12.0, 12.0, 2.8);
        }
        Icon::Star => {
            let pts: Vec<(f32, f32)> = (0..10)
                .map(|i| {
                    let a = (-90.0 + i as f32 * 36.0f32).to_radians();
                    let r = if i % 2 == 0 { 9.0 } else { 3.9 };
                    (12.0 + r * a.cos(), 12.8 + r * a.sin())
                })
                .collect();
            pen.closed(&pts);
        }
        Icon::Command => {
            // Palette Ctrl+K : champ de saisie avec loupe, liste dessous.
            pen.rect(3.0, 3.5, 21.0, 10.5, 2.0);
            pen.circle(7.3, 7.0, 1.7);
            pen.line((8.6, 8.3), (9.8, 9.5));
            pen.line((12.0, 7.0), (18.0, 7.0));
            pen.line((4.0, 15.0), (20.0, 15.0));
            pen.line((4.0, 19.5), (15.0, 19.5));
        }
        Icon::Keyboard => {
            pen.rect(2.5, 6.0, 21.5, 18.0, 2.0);
            for x in [6.0, 9.5, 13.0, 16.5] {
                pen.dot(x + 0.5, 9.8, 0.9);
            }
            for x in [6.0, 18.0] {
                pen.dot(x, 13.0, 0.9);
            }
            pen.line((8.5, 14.5), (15.5, 14.5));
        }
        Icon::Terminal => {
            pen.rect(3.0, 4.5, 21.0, 19.5, 2.0);
            pen.poly(&[(7.0, 9.0), (10.5, 12.0), (7.0, 15.0)]);
            pen.line((12.5, 15.5), (17.0, 15.5));
        }
        Icon::Warning => {
            pen.closed(&[(12.0, 3.5), (21.5, 19.5), (2.5, 19.5)]);
            pen.line((12.0, 9.5), (12.0, 14.0));
            pen.dot(12.0, 16.8, 1.1);
        }
        Icon::Info => {
            pen.circle(12.0, 12.0, 9.0);
            pen.line((12.0, 11.0), (12.0, 17.0));
            pen.dot(12.0, 7.8, 1.1);
        }
        Icon::Dot => pen.dot(12.0, 12.0, 4.0),
        Icon::Power => {
            pen.arc(12.0, 13.0, 7.5, -50.0, 230.0);
            pen.line((12.0, 3.5), (12.0, 11.5));
        }
        Icon::Sleep => {
            moon(pen);
            let small = Pen { w: pen.w * 0.8, ..*pen };
            small.poly(&[(15.0, 3.5), (19.0, 3.5), (15.0, 7.5), (19.0, 7.5)]);
        }
        Icon::Block => {
            pen.circle(12.0, 12.0, 8.5);
            pen.line((6.0, 18.0), (18.0, 6.0));
        }
        Icon::Gamepad => {
            let mut pts = vec![(7.0, 7.0), (17.0, 7.0)];
            pts.extend(pen.arc_pts(17.5, 12.5, 5.0, -95.0, 60.0));
            pts.extend([(17.0, 17.0), (15.0, 15.0), (9.0, 15.0), (7.0, 17.0)]);
            pts.extend(pen.arc_pts(6.5, 12.5, 5.0, 120.0, 275.0));
            pen.closed(&pts);
            pen.line((5.5, 11.0), (9.5, 11.0));
            pen.line((7.5, 9.0), (7.5, 13.0));
            pen.dot(15.5, 10.0, 1.0);
            pen.dot(17.5, 12.5, 1.0);
        }
        Icon::Container => {
            pen.rect(3.0, 11.0, 21.0, 18.0, 1.5);
            for x in [7.5, 12.0, 16.5] {
                pen.line((x, 12.5), (x, 16.5));
            }
            pen.rect(5.0, 6.5, 9.5, 11.0, 0.5);
            pen.rect(9.5, 6.5, 14.0, 11.0, 0.5);
            pen.rect(9.5, 2.5, 14.0, 6.5, 0.5);
        }
        Icon::Ghost => {
            let mut pts = pen.arc_pts(12.0, 10.0, 6.5, 180.0, 360.0);
            pts.extend([(18.5, 20.0), (16.3, 18.3), (14.2, 20.0), (12.0, 18.3), (9.8, 20.0), (7.7, 18.3), (5.5, 20.0)]);
            pen.closed(&pts);
            pen.dot(9.5, 10.0, 1.2);
            pen.dot(14.5, 10.0, 1.2);
        }
        Icon::Chart => {
            pen.poly(&[(3.5, 3.5), (3.5, 20.5), (20.5, 20.5)]);
            pen.poly(&[(6.5, 16.0), (10.5, 11.0), (13.5, 13.5), (19.0, 6.5)]);
        }
        Icon::Shield => {
            pen.closed(&[(12.0, 3.0), (19.5, 6.0), (19.0, 12.5), (16.0, 17.5), (12.0, 21.0), (8.0, 17.5), (5.0, 12.5), (4.5, 6.0)]);
            pen.poly(&[(8.5, 12.0), (11.0, 14.5), (15.5, 9.5)]);
        }
        Icon::Broom => {
            pen.line((19.5, 3.5), (12.5, 11.0));
            pen.closed(&[(11.0, 9.5), (14.5, 13.0), (12.0, 20.5), (3.5, 12.0)]);
            pen.line((7.5, 16.5), (10.0, 14.0));
        }
        Icon::Bolt => pen.closed(&[(13.5, 2.5), (5.5, 13.5), (11.5, 13.5), (10.5, 21.5), (18.5, 10.5), (12.5, 10.5)]),
        Icon::Cpu => {
            pen.rect(6.0, 6.0, 18.0, 18.0, 2.0);
            pen.rect(9.5, 9.5, 14.5, 14.5, 0.8);
            for k in [9.0, 12.0, 15.0] {
                pen.line((k, 3.0), (k, 6.0));
                pen.line((k, 18.0), (k, 21.0));
                pen.line((3.0, k), (6.0, k));
                pen.line((18.0, k), (21.0, k));
            }
        }
        Icon::Memory => {
            pen.rect(2.5, 7.0, 21.5, 15.5, 1.2);
            for x in [5.5, 9.5, 14.5, 18.5] {
                pen.rect_fill(x - 1.0, 9.5, x + 1.0, 13.0, 0.2);
            }
            for x in [5.0, 8.0, 11.0, 13.0, 16.0, 19.0] {
                pen.line((x, 15.5), (x, 18.0));
            }
        }
    }
}

/// Contour de fichier à coin replié.
fn file_outline(pen: &Pen) {
    pen.closed(&[(5.5, 2.5), (14.0, 2.5), (18.5, 7.0), (18.5, 21.5), (5.5, 21.5)]);
    pen.poly(&[(14.0, 2.5), (14.0, 7.0), (18.5, 7.0)]);
}

/// Croissant (lecteur) : arc extérieur hors du disque qui « mord », puis arc intérieur.
fn moon(pen: &Pen) {
    let (c1, r1, c2, r2) = ((12.0f32, 12.5f32), 8.0f32, (16.5f32, 8.5f32), 7.0f32);
    let d = |a: (f32, f32), b: (f32, f32)| ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt();
    let s = pen.s;
    let arc_pts = |c: (f32, f32), r: f32, keep: &dyn Fn((f32, f32)) -> bool| -> Vec<Pos2> {
        let mut v: Vec<Pos2> = Vec::new();
        for i in 0..72 {
            let a = (i as f32 * 5.0).to_radians();
            let pt = (c.0 + r * a.cos(), c.1 + r * a.sin());
            if keep(pt) {
                v.push(pen.q(pt.0, pt.1));
            }
        }
        if let Some(k) = (1..v.len()).find(|&k| v[k].distance(v[k - 1]) > 3.0 * s) {
            v.rotate_left(k);
        }
        v
    };
    let mut pts = arc_pts(c1, r1, &|pt| d(pt, c2) >= r2);
    let mut inner = arc_pts(c2, r2, &|pt| d(pt, c1) <= r1);
    if let (Some(last), Some(f), Some(l)) = (pts.last().copied(), inner.first().copied(), inner.last().copied()) {
        if last.distance(f) > last.distance(l) {
            inner.reverse();
        }
    }
    pts.append(&mut inner);
    if let Some(f) = pts.first().copied() {
        pts.push(f);
    }
    pen.p.add(Shape::line(pts, pen.stroke()));
}

/// La famille `bold` n'existe que si `theme::install_fonts` a tourné ; sinon egui paniquerait
/// au rendu du texte : on retombe alors sur la police proportionnelle.
trait OrBold {
    fn clone_or(self, fallback: egui::FontFamily) -> egui::FontFamily;
}
impl OrBold for egui::FontFamily {
    fn clone_or(self, fallback: egui::FontFamily) -> egui::FontFamily {
        if BOLD_OK.load(std::sync::atomic::Ordering::Relaxed) { self } else { fallback }
    }
}
static BOLD_OK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Appelé par `theme::install_fonts` : la famille `bold` existe désormais.
#[doc(hidden)]
pub fn set_bold_available(ok: bool) {
    BOLD_OK.store(ok, std::sync::atomic::Ordering::Relaxed);
}

/// Couleurs d'un bouton-icône.
#[derive(Clone, Copy)]
pub struct Look {
    pub fg: Color32,
    pub bg: Color32,
    pub hover: Color32,
    pub round: bool,
}

impl Look {
    /// Bouton d'outil discret : transparent au repos, `hover_bg` au survol.
    #[cfg(feature = "theme")]
    pub fn tool(p: &crate::theme::Palette) -> Look {
        Look { fg: p.text, bg: Color32::TRANSPARENT, hover: p.hover_bg, round: false }
    }
    /// Bouton actif (mode sélectionné).
    #[cfg(feature = "theme")]
    pub fn active(p: &crate::theme::Palette) -> Look {
        Look { fg: p.text, bg: p.sel_bg, hover: p.sel_bg, round: false }
    }
}

/// Bouton-icône carré (taille `size`), icône centrée à ~62 % (50 % au-delà de 44 px).
pub fn button(ui: &mut Ui, icon: Icon, size: f32, look: Look, tip: &str) -> Response {
    let (rect, r) = ui.allocate_exact_size(Vec2::splat(size), Sense::click());
    let bg = if r.is_pointer_button_down_on() { look.hover.gamma_multiply(1.4) } else if r.hovered() { look.hover } else { look.bg };
    if bg != Color32::TRANSPARENT {
        let rr = if look.round { CornerRadius::same((size / 2.0) as u8) } else { CornerRadius::same(7) };
        ui.painter().rect_filled(rect, rr, bg);
    }
    let k = if size >= 44.0 { 0.5 } else { 0.62 };
    let fg = if ui.is_enabled() { look.fg } else { look.fg.gamma_multiply(0.45) };
    paint(ui.painter(), Rect::from_center_size(rect.center(), Vec2::splat(size * k)), icon, fg);
    if tip.is_empty() { r } else { r.on_hover_text(tip) }
}

/// Bouton large du lecteur : icône + libellé (+ raccourci à droite), hauteur 36 (40 si principal).
#[cfg(feature = "theme")]
pub fn wide(ui: &mut Ui, icon: Icon, label: &str, key: &str, w: f32, primary: Option<(Color32, Color32)>, p: &crate::theme::Palette) -> Response {
    let h = if primary.is_some() { 40.0 } else { 36.0 };
    let enabled = ui.is_enabled();
    let (rect, r) = ui.allocate_exact_size(vec2(w, h), Sense::click());
    let (bg, fg) = match primary {
        Some((bg, fg)) => (if r.hovered() { bg.gamma_multiply(0.9) } else { bg }, fg),
        None => (if r.hovered() { p.hover_bg } else { p.code }, p.text),
    };
    let (bg, fg) = if enabled { (bg, fg) } else if primary.is_some() { (p.soft, p.muted) } else { (bg, fg.gamma_multiply(0.45)) };
    ui.painter().rect(rect, CornerRadius::same(8), bg, if primary.is_some() { Stroke::NONE } else { Stroke::new(1.0, p.border) }, StrokeKind::Inside);
    let ir = Rect::from_center_size(egui::pos2(rect.left() + 20.0, rect.center().y), Vec2::splat(18.0));
    paint(ui.painter(), ir, icon, fg);
    let font = if primary.is_some() { FontId::new(14.0, egui::FontFamily::Name("bold".into()).clone_or(bold())) } else { FontId::proportional(13.5) };
    let key_w = if key.is_empty() { 0.0 } else { ui.painter().layout_no_wrap(key.to_string(), FontId::monospace(11.0), fg).size().x + 14.0 };
    let mut job = egui::text::LayoutJob::simple_singleline(label.to_string(), font, fg);
    job.wrap = egui::text::TextWrapping { max_width: (w - 48.0 - key_w).max(20.0), max_rows: 1, break_anywhere: true, overflow_character: Some('…') };
    let g = ui.painter().layout_job(job);
    ui.painter().galley(egui::pos2(rect.left() + 38.0, rect.center().y - g.size().y / 2.0), g, fg);
    if !key.is_empty() {
        ui.painter().text(egui::pos2(rect.right() - 10.0, rect.center().y), egui::Align2::RIGHT_CENTER, key, FontId::monospace(11.0), fg.gamma_multiply(0.6));
    }
    r
}
