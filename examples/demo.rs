//! Démo du socle : thème (☀/🌙), icônes, contrôle segmenté, champ de recherche, autocomplétion
//! (Tab), palette Ctrl+K, aide F1, toasts, modale et miniatures décodées en tâche de fond.
//!
//! `cargo run --release --example demo --features full [-- <dossier d'images>]`
//!
//! Même boucle que les apps : winit + egui + rendu CPU (hdrshot-egui-backend + softbuffer).
#![allow(deprecated)]

use egui::{Color32, CornerRadius, FontId, Rect, RichText, Sense, TextureHandle, Ui, pos2, vec2};
use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use socle::autocomplete::{Autocomplete, History};
use socle::icons::{self, Icon, Look};
use socle::images::{self, Pool};
use socle::theme::{self, Palette as Colors};
use socle::ui::{self, AutoField, Command, Palette, Toasts};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::window::{Window, WindowId};

struct Demo {
    pal: Colors,
    view: usize,
    filter: String,
    query: String,
    auto: AutoField,
    ac: Autocomplete,
    palette: Palette,
    help: bool,
    modal: bool,
    toasts: Toasts,
    pool: Pool,
    thumbs: Vec<(PathBuf, Option<TextureHandle>, Option<String>)>,
    started: Instant,
    /// Icônes sautées (captures de la 2ᵉ moitié de la grille).
    skip: usize,
}

fn commands() -> Vec<Command> {
    vec![
        Command::new("theme", "Basculer le thème clair / sombre").group("Vue").keys("Ctrl+Maj+T").icon(Icon::Moon),
        Command::new("grille", "Afficher les icônes").group("Vue").icon(Icon::Grid),
        Command::new("liste", "Afficher les miniatures").group("Vue").icon(Icon::Image),
        Command::new("toast", "Afficher un toast").group("Démo").icon(Icon::Info),
        Command::new("erreur", "Afficher une erreur").group("Démo").icon(Icon::Warning),
        Command::new("modale", "Ouvrir une fenêtre modale").group("Démo").keys("Ctrl+M").icon(Icon::Panel),
        Command::new("aide", "Raccourcis clavier").group("Aide").keys("F1").icon(Icon::Keyboard),
        Command::new("palette", "Palette de commandes").group("Aide").keys("Ctrl+K").icon(Icon::Command),
        Command::new("rien", "Commande désactivée").group("Démo").enabled(false),
    ]
}

impl Demo {
    fn new(ctx: &egui::Context, dir: Option<PathBuf>) -> Demo {
        theme::install_fonts(ctx);
        let pal = theme::palette(theme::system_dark(ctx));
        theme::apply(ctx, &pal);
        let mut ac = Autocomplete::new(History::default());
        // Candidats : sous-dossiers du profil + quelques noms
        let mut cands: Vec<String> = ["Téléchargements", "Vidéos", "Musique", "Documents", "Développement", "Captures d'écran", "Minecraft", "SteamLibrary"].map(String::from).to_vec();
        if let Some(home) = std::env::var_os("USERPROFILE") {
            if let Ok(rd) = std::fs::read_dir(home) {
                cands.extend(rd.flatten().filter(|e| e.path().is_dir()).map(|e| e.file_name().to_string_lossy().into_owned()));
            }
        }
        ac.set_candidates(cands);
        ac.history.record("Projet Explorateur");
        let c2 = ctx.clone();
        let mut pool = Pool::new(3, Some(images::DiskCache::for_app("SocleDemo")), move || c2.request_repaint());
        let dir = dir.or_else(|| std::env::var_os("USERPROFILE").map(|h| PathBuf::from(h).join("Pictures")));
        let mut thumbs = Vec::new();
        if let Some(d) = dir {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten().map(|e| e.path()).filter(|p| images::is_supported(p)).take(40) {
                    pool.request(&e, 160);
                    thumbs.push((e, None, None));
                }
            }
        }
        let mut toasts = Toasts::default();
        toasts.info("Ctrl+K : palette · F1 : raccourcis · Tab : compléter");
        Demo { pal, view: 0, filter: String::new(), query: String::new(), auto: AutoField::default(), ac, palette: Palette::default(), help: false, modal: false, toasts, pool, thumbs, started: Instant::now(), skip: 0 }
    }

    fn set_dark(&mut self, ctx: &egui::Context, dark: bool) {
        self.pal = theme::palette(dark);
        theme::apply(ctx, &self.pal);
    }

    fn run(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        let p = self.pal;
        for d in self.pool.poll() {
            if let Some(t) = self.thumbs.iter_mut().find(|t| t.0 == d.path) {
                match d.result {
                    Ok(img) => t.1 = Some(ctx.load_texture(d.path.to_string_lossy(), images::to_color_image(&img), Default::default())),
                    Err(e) => t.2 = Some(e),
                }
            }
        }
        // ---- actions clavier
        self.palette.handle_shortcut(&ctx);
        if ctx.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::M)) {
            self.modal = true;
        }
        let cmds = commands();
        let mut run: Option<String> = self.palette.show(&ctx, &p, &cmds);
        ui::help_window(&ctx, &p, &mut self.help, &cmds);

        // ---- barre d'outils
        egui::Panel::top("outils").frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(12, 8))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Socle").size(theme::taille::TITRE).strong());
                ui.add_space(8.0);
                if ui::segmented(ui, &p, &mut self.view, &[(Some(Icon::Grid), "Icônes"), (Some(Icon::Image), "Miniatures")]) {
                    self.toasts.ok(format!("Vue {}", if self.view == 0 { "icônes" } else { "miniatures" }));
                }
                ui.add_space(8.0);
                ui::search_field(ui, &p, egui::Id::unique("filtre"), &mut self.filter, "Filtrer les icônes…", 220.0);
                ui.add_space(8.0);
                let r = ui::autocomplete_field(ui, &p, egui::Id::unique("auto"), &mut self.auto, &self.ac, &mut self.query, "Dossier… (Tab complète)", 260.0);
                if let Some(s) = r.submitted {
                    self.ac.history.record(&s);
                    self.toasts.ok(format!("Validé : {s}"));
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let look = Look::tool(&p);
                    if icons::button(ui, Icon::More, 32.0, look, "Palette (Ctrl+K)").clicked() {
                        self.palette.open();
                    }
                    let (ic, tip) = if p.dark { (Icon::Sun, "Thème clair") } else { (Icon::Moon, "Thème sombre") };
                    if icons::button(ui, ic, 32.0, look, tip).clicked() {
                        run = Some("theme".into());
                    }
                    if icons::button(ui, Icon::Help, 32.0, look, "Raccourcis (F1)").clicked() {
                        self.help = true;
                    }
                });
            });
        });
        // ---- barre d'état
        egui::Panel::bottom("etat").frame(egui::Frame::new().fill(p.chrome).inner_margin(egui::Margin::symmetric(12, 6))).show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} icônes · {} images · ouvert depuis {}", Icon::ALL.len(), self.thumbs.len(), ui::ago(self.started.elapsed().as_secs_f32()))).color(p.muted).size(theme::taille::ETAT));
                ui.add_space(12.0);
                self.toasts.ui(ui, &p);
            });
        });
        // ---- contenu
        egui::CentralPanel::default().frame(egui::Frame::new().fill(p.surface).inner_margin(16)).show(ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if self.view == 0 {
                    self.draw_icons(ui);
                } else {
                    self.draw_thumbs(ui);
                }
            });
        });
        if self.modal {
            let m = ui::modal(&ctx, "demo-modale", &p, 420.0, 140.0, |ui| {
                ui.label(RichText::new("Fenêtre modale").size(theme::taille::TITRE).strong());
                ui.label("Fond assombri, Échap ou clic à côté pour fermer.");
                ui.add_space(8.0);
                icons::wide(ui, Icon::Check, "D'accord", "Entrée", 200.0, Some((p.accent, p.on_accent)), &p).clicked()
            });
            if m.close || m.inner {
                self.modal = false;
            }
        }
        if let Some(id) = run {
            match id.as_str() {
                "theme" => self.set_dark(&ctx, !p.dark),
                "grille" => self.view = 0,
                "liste" => self.view = 1,
                "toast" => self.toasts.info("Bonjour depuis la palette"),
                "erreur" => self.toasts.error("Ceci est une erreur (toast rouge, 8 s)"),
                "modale" => self.modal = true,
                "aide" => self.help = true,
                "palette" => self.palette.open(),
                _ => {}
            }
        }
    }

    fn draw_icons(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::section_header(ui, &p, "Icônes vectorielles (grille 24, trait 1,8)");
        ui.add_space(6.0);
        let names: Vec<(Icon, String)> = Icon::ALL.iter().map(|i| (*i, format!("{i:?}"))).filter(|(_, n)| self.filter.is_empty() || socle::fuzzy::score(n, &self.filter).is_some()).skip(self.skip).collect();
        let cell = vec2(104.0, 84.0);
        let cols = ((ui.available_width() / (cell.x + 8.0)) as usize).max(1);
        for row in names.chunks(cols) {
            ui.horizontal(|ui| {
                for (ic, name) in row {
                    let (r, resp) = ui.allocate_exact_size(cell, Sense::click());
                    ui.painter().rect_filled(r, CornerRadius::same(theme::arrondi::CARTE), if resp.hovered() { p.hover_bg } else { p.side });
                    let ir = Rect::from_center_size(pos2(r.center().x, r.top() + 30.0), vec2(28.0, 28.0));
                    icons::paint(ui.painter(), ir, *ic, if matches!(ic, Icon::Folder) { p.accent } else { p.text });
                    let g = ui::highlighted(ui, name, &self.filter, FontId::proportional(11.5), p.muted, p.accent, r.width() - 10.0);
                    ui.painter().galley(pos2(r.center().x - g.size().x / 2.0, r.bottom() - 22.0), g, p.muted);
                    if resp.clicked() {
                        self.toasts.info(format!("Icon::{name}"));
                    }
                }
            });
        }
        ui.add_space(16.0);
        ui::section_header(ui, &p, "Tailles 12 / 16 / 24 / 48");
        ui.horizontal(|ui| {
            for s in [12.0, 16.0, 24.0, 48.0] {
                for ic in [Icon::Play, Icon::Folder, Icon::Settings, Icon::Git, Icon::Ghost] {
                    let (r, _) = ui.allocate_exact_size(vec2(s, s), Sense::hover());
                    icons::paint(ui.painter(), r, ic, p.text);
                }
                ui.add_space(12.0);
            }
        });
    }

    fn draw_thumbs(&mut self, ui: &mut Ui) {
        let p = self.pal;
        ui::section_header(ui, &p, "Miniatures (pool de 3 threads, cache disque QOI)");
        if self.thumbs.is_empty() {
            ui.label(RichText::new("Aucune image : passez un dossier en argument.").color(p.muted));
        }
        let cell = vec2(176.0, 196.0);
        let cols = ((ui.available_width() / (cell.x + 8.0)) as usize).max(1);
        for row in self.thumbs.chunks(cols) {
            ui.horizontal(|ui| {
                for (path, tex, err) in row {
                    let (r, _) = ui.allocate_exact_size(cell, Sense::hover());
                    ui.painter().rect_filled(r, CornerRadius::same(theme::arrondi::CARTE), p.side);
                    let area = Rect::from_min_size(r.min + vec2(8.0, 8.0), vec2(160.0, 160.0));
                    match (tex, err) {
                        (Some(t), _) => {
                            let s = t.size_vec2();
                            let k = (160.0 / s.x.max(s.y)).min(1.0);
                            let ir = Rect::from_center_size(area.center(), s * k);
                            ui.painter().image(t.id(), ir, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), Color32::WHITE);
                        }
                        (None, Some(e)) => {
                            icons::paint(ui.painter(), Rect::from_center_size(area.center(), vec2(32.0, 32.0)), Icon::Warning, p.del);
                            let _ = e;
                        }
                        (None, None) => icons::paint(ui.painter(), Rect::from_center_size(area.center(), vec2(28.0, 28.0)), Icon::Image, p.muted),
                    }
                    let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                    ui::paint_text_left(ui, Rect::from_min_size(pos2(r.left(), r.bottom() - 26.0), vec2(r.width(), 20.0)), r.left() + 8.0, &name, FontId::proportional(12.5), p.text, r.width() - 16.0);
                }
            });
        }
    }
}

// ------------------------------------------------------------------ boucle (copie de l'explorateur)

struct Gfx {
    window: Rc<Window>,
    egui: egui_winit::State,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    renderer: EguiSoftwareRender,
}

enum UserEvent {
    RepaintAt(Instant),
}

struct Runner {
    ctx: egui::Context,
    gfx: Option<Gfx>,
    app: Option<Demo>,
    dir: Option<PathBuf>,
    next_repaint: Option<Instant>,
    first_frame: Option<Instant>,
    _proxy: EventLoopProxy<UserEvent>,
}

impl Runner {
    fn paint(&mut self) {
        let (Some(gfx), Some(app)) = (self.gfx.as_mut(), self.app.as_mut()) else { return };
        let size = gfx.window.inner_size();
        let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
        let raw = gfx.egui.take_egui_input(&gfx.window);
        let out = self.ctx.run_ui(raw, |ui| app.run(ui));
        gfx.egui.handle_platform_output(&gfx.window, out.platform_output);
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        if gfx.surface.resize(w, h).is_err() {
            return;
        }
        let Ok(mut buf) = gfx.surface.buffer_mut() else { return };
        buf.fill(app.pal.clear_u32());
        {
            let px: &mut [[u8; 4]] = bytemuck::cast_slice_mut(&mut buf[..]);
            let mut target = BufferMutRef::new(px, w.get() as usize, h.get() as usize);
            gfx.renderer.render(&mut target, &prims, &out.textures_delta, out.pixels_per_point);
        }
        let _ = buf.present();
        if let Some(t) = self.first_frame.take() {
            eprintln!("première image affichée en {:.0} ms", t.elapsed().as_secs_f64() * 1000.0);
        }
    }
}

impl ApplicationHandler<UserEvent> for Runner {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("Socle — démo").with_inner_size(winit::dpi::LogicalSize::new(1180.0, 760.0));
        let window = Rc::new(el.create_window(attrs).expect("fenêtre"));
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("surface");
        let egui = egui_winit::State::new(self.ctx.clone(), egui::ViewportId::ROOT, &*window, Some(window.scale_factor() as f32), window.theme(), Some(8192));
        let renderer = EguiSoftwareRender::new(ColorFieldOrder::Bgra);
        window.request_redraw();
        self.gfx = Some(Gfx { window, egui, _context: context, surface, renderer });
        if self.app.is_none() {
            // egui connaît le thème système après la 1ʳᵉ entrée : on l'initialise ici.
            let _ = self.ctx.run_ui(Default::default(), |_| {});
            self.app = Some(Demo::new(&self.ctx, self.dir.take()));
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(gfx) = self.gfx.as_mut() else { return };
        let resp = gfx.egui.on_window_event(&gfx.window, &event);
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::RedrawRequested => self.paint(),
            _ => {
                if resp.repaint {
                    gfx.window.request_redraw();
                }
            }
        }
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, ev: UserEvent) {
        let UserEvent::RepaintAt(t) = ev;
        if t <= Instant::now() {
            if let Some(g) = &self.gfx {
                g.window.request_redraw();
            }
        } else {
            self.next_repaint = Some(self.next_repaint.map_or(t, |n| n.min(t)));
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        match self.next_repaint {
            Some(t) if t <= Instant::now() => {
                self.next_repaint = None;
                if let Some(g) = &self.gfx {
                    g.window.request_redraw();
                }
                el.set_control_flow(ControlFlow::Wait);
            }
            Some(t) => el.set_control_flow(ControlFlow::WaitUntil(t)),
            None => el.set_control_flow(ControlFlow::Wait),
        }
    }
}

/// Images d'exemple dans plusieurs formats (dans %TEMP%).
fn samples() -> PathBuf {
    let d = std::env::temp_dir().join("socle-demo-images");
    std::fs::create_dir_all(&d).ok();
    let grad = |a: [u8; 3], b: [u8; 3]| {
        image::RgbImage::from_fn(480, 320, move |x, y| {
            let t = (x + y) as f32 / 800.0;
            let l = |i: usize| (a[i] as f32 + (b[i] as f32 - a[i] as f32) * t) as u8;
            if (x / 40 + y / 40) % 5 == 0 { image::Rgb([245, 237, 220]) } else { image::Rgb([l(0), l(1), l(2)]) }
        })
    };
    let _ = grad([46, 94, 158], [242, 166, 90]).save(d.join("degrade.png"));
    let _ = grad([15, 27, 45], [111, 168, 255]).save(d.join("nuit.jpg"));
    let _ = grad([169, 86, 26], [239, 230, 211]).save(d.join("caramel.tga"));
    let _ = grad([47, 115, 70], [224, 235, 214]).save(d.join("foret.qoi"));
    let _ = std::fs::write(d.join("dessin.psd"), images::testdata::psd_rgb_rle(&grad([168, 52, 52], [242, 220, 212])));
    let _ = std::fs::write(d.join("logo.svg"), r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><rect x="3" y="3" width="18" height="18" rx="5" fill="#2E5E9E"/><path d="M9.5 7.5L17 12l-7.5 4.5z" fill="#F5EDDC"/></svg>"##);
    let mut raw = b"II*    ".to_vec();
    let mut j = Vec::new();
    let _ = image::DynamicImage::ImageRgb8(grad([98, 88, 70], [247, 240, 225])).write_to(&mut std::io::Cursor::new(&mut j), image::ImageFormat::Jpeg);
    raw.extend(vec![0u8; 2048]);
    raw.extend(j);
    let _ = std::fs::write(d.join("photo.nef"), raw);
    let _ = std::fs::write(d.join("abime.png"), b"pas une image");
    d
}

/// `--capture <dossier>` : rend la démo hors écran (sans fenêtre) en PNG, pour vérifier le rendu.
fn capture(out: &std::path::Path, dir: Option<PathBuf>) {
    let ctx = egui::Context::default();
    let (w, h) = (1180usize, 760usize);
    let input = |events: Vec<egui::Event>| egui::RawInput {
        screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(w as f32, h as f32))),
        events,
        ..Default::default()
    };
    let _ = ctx.run_ui(input(vec![]), |_| {});
    let dir = dir.unwrap_or_else(samples);
    let mut app = Demo::new(&ctx, Some(dir));
    let mut renderer = EguiSoftwareRender::new(ColorFieldOrder::Rgba);
    let mut shot = |app: &mut Demo, name: &str, events: Vec<egui::Event>| {
        // quelques images : mise en page, polices, textures
        for k in 0..6 {
            let ev = if k == 2 { events.clone() } else { vec![] };
            std::thread::sleep(std::time::Duration::from_millis(if k == 3 { 300 } else { 5 }));
            let o = ctx.run_ui(input(ev), |ui| app.run(ui));
            let prims = ctx.tessellate(o.shapes, o.pixels_per_point);
            let mut px = vec![[0u8; 4]; w * h];
            let bg = app.pal.bg;
            px.fill([bg.r(), bg.g(), bg.b(), 255]);
            let mut target = BufferMutRef::new(&mut px, w, h);
            renderer.render(&mut target, &prims, &o.textures_delta, o.pixels_per_point);
            if k == 5 {
                let raw: Vec<u8> = px.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                let img = image::RgbaImage::from_raw(w as u32, h as u32, raw).unwrap();
                img.save(out.join(format!("{name}.png"))).unwrap();
            }
        }
    };
    std::fs::create_dir_all(out).ok();
    app.set_dark(&ctx, false);
    shot(&mut app, "1-icones-clair", vec![]);
    app.set_dark(&ctx, true);
    shot(&mut app, "2-icones-sombre", vec![]);
    app.palette.open();
    shot(&mut app, "3-palette", vec![egui::Event::Text("aff".into())]);
    app.palette.close();
    app.view = 1;
    app.set_dark(&ctx, false);
    shot(&mut app, "4-miniatures", vec![]);
    app.view = 0;
    app.help = true;
    shot(&mut app, "5-aide-f1", vec![]);
    app.help = false;
    app.skip = 60;
    shot(&mut app, "7-icones-suite", vec![]);
    app.skip = 0;
    // autocomplétion : focus sur le champ puis frappe
    ctx.memory_mut(|m| m.request_focus(egui::Id::unique("auto")));
    shot(&mut app, "6-autocompletion", vec![egui::Event::Text("tele".into())]);
    println!("captures écrites dans {}", out.display());
}

fn main() {
    let t0 = Instant::now();
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--capture") {
        let out = PathBuf::from(args.get(i + 1).cloned().unwrap_or_else(|| "captures".into()));
        capture(&out, args.get(i + 2).map(PathBuf::from));
        return;
    }
    socle::crash::install("SocleDemo", env!("CARGO_PKG_VERSION"));
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().expect("boucle d'événements");
    let proxy = event_loop.create_proxy();
    let ctx = egui::Context::default();
    let p2 = proxy.clone();
    ctx.set_request_repaint_callback(move |info| {
        let _ = p2.send_event(UserEvent::RepaintAt(Instant::now() + info.delay));
    });
    let dir = args.get(1).map(PathBuf::from);
    let mut runner = Runner { ctx, gfx: None, app: None, dir, next_repaint: None, first_frame: Some(t0), _proxy: proxy };
    let _ = event_loop.run_app(&mut runner);
}
