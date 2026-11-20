//! Exe témoin pour mesurer ce que chaque feature ajoute à la taille d'une app.
//! La base imite une app : egui + décodage PNG/JPEG/GIF/WebP/BMP/ICO par `image` (ce que
//! l'explorateur embarque déjà). Chaque feature activée ajoute un vrai appel à son API, pour
//! que l'éditeur de liens garde le code. Voir `scripts/taille.ps1`.
#![allow(unused)]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let ctx = egui::Context::default();
    // Base réaliste : ce que toute app egui utilise déjà (polices, visuels, widgets, zones, dessin).
    ctx.set_fonts(egui::FontDefinitions::default());
    ctx.set_visuals_of(egui::Theme::Dark, egui::Visuals::dark());
    let mut txt = String::new();
    let _ = ctx.run_ui(Default::default(), |ui| {
        ui.label(args.len().to_string());
        egui::ScrollArea::vertical().show(ui, |ui| {
            if ui.button("b").clicked() {
                txt.push('x');
            }
            ui.text_edit_singleline(&mut txt);
            egui::Frame::new().fill(egui::Color32::RED).corner_radius(4).show(ui, |ui| ui.label("f"));
            ui.painter().circle_stroke(egui::pos2(1.0, 1.0), 2.0, egui::Stroke::new(1.0, egui::Color32::WHITE));
            ui.painter().add(egui::Shape::convex_polygon(vec![egui::pos2(0.0, 0.0), egui::pos2(1.0, 0.0), egui::pos2(0.0, 1.0)], egui::Color32::WHITE, egui::Stroke::NONE));
        });
        egui::Area::new(egui::Id::unique("z")).show(ui.ctx(), |ui| ui.label("a"));
        egui::Grid::new("g").show(ui, |ui| {
            ui.label("x");
            ui.end_row();
        });
        if ui.input_mut(|i| i.consume_key(egui::Modifiers::COMMAND, egui::Key::K)) {
            txt.clear();
        }
    });
    println!("{txt}");
    if let Some(p) = args.get(1) {
        if let Ok(b) = std::fs::read(p) {
            println!("{:?}", image::load_from_memory(&b).map(|i| i.width()).ok());
        }
    }
    #[cfg(feature = "theme")]
    {
        socle::theme::install_fonts(&ctx);
        socle::theme::apply(&ctx, &socle::theme::palette(args.len() > 3));
    }
    #[cfg(feature = "keys")]
    println!("{:?}", socle::keys::pressed(&ctx));
    #[cfg(feature = "icons")]
    let _ = ctx.run_ui(Default::default(), |ui| {
        let r = egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(24.0, 24.0));
        for ic in socle::icons::Icon::ALL {
            socle::icons::paint(ui.painter(), r, *ic, egui::Color32::WHITE);
        }
    });
    #[cfg(feature = "ui")]
    {
        let p = socle::theme::palette(false);
        let mut pal = socle::ui::Palette::default();
        pal.open();
        let mut toasts = socle::ui::Toasts::default();
        toasts.info("x");
        let mut ac = socle::autocomplete::Autocomplete::default();
        ac.set_candidates(args.clone());
        let mut f = socle::ui::AutoField::default();
        let mut t = String::new();
        let mut v = 0;
        let cmds = vec![socle::ui::Command::new("a", "A")];
        let _ = ctx.run_ui(Default::default(), |ui| {
            println!("{:?}", pal.show(ui.ctx(), &p, &cmds));
            toasts.ui(ui, &p);
            socle::ui::autocomplete_field(ui, &p, egui::Id::unique("a"), &mut f, &ac, &mut t, "", 100.0);
            socle::ui::segmented(ui, &p, &mut v, &[(None, "a"), (None, "b")]);
            let mut h = false;
            socle::ui::help_window(ui.ctx(), &p, &mut h, &cmds);
            socle::ui::modal(ui.ctx(), "m", &p, 100.0, 10.0, |ui| ui.label("x"));
        });
    }
    #[cfg(feature = "fuzzy")]
    {
        let ix = socle::fuzzy::Index::from_iter(&args);
        println!("{:?}", ix.search("abc", 5));
        let mut ac = socle::autocomplete::Autocomplete::default();
        ac.set_candidates(args.clone());
        println!("{:?}", ac.suggest("a", 3));
    }
    #[cfg(feature = "images")]
    if let Some(p) = args.get(2) {
        let c = socle::Cancel::new();
        let p = std::path::Path::new(p);
        println!("{:?}", socle::images::thumbnail(p, 256, &c).map(|i| i.width()));
        println!("{:?}", socle::images::decode(p, &c).map(|i| i.width()));
        let cache = socle::images::DiskCache::for_app("Taille");
        println!("{:?}", socle::images::thumbnail_cached(&cache, p, 64, &c).map(|i| i.width()));
        let mut pool = socle::images::Pool::new(2, None, || {});
        pool.request(p, 64);
        println!("{}", pool.poll().len());
    }
    #[cfg(feature = "settings")]
    {
        let s: std::collections::HashMap<String, String> = socle::settings::load("Taille");
        println!("{:?}", socle::settings::save("Taille", &s));
    }
    #[cfg(feature = "crash")]
    {
        socle::crash::install("Taille", "0");
        println!("{:?}", socle::crash::pending("Taille"));
    }
    #[cfg(all(feature = "instance", windows))]
    if let socle::instance::Start::Primary(i) = socle::instance::start("Taille", &args) {
        i.listen(|a| println!("{a:?}"));
    }
    #[cfg(all(feature = "testmode", windows))]
    if args.len() > 6 {
        socle::testmode::low_priority();
        socle::testmode::hide(args.len() as isize);
        println!("{:?} {}", socle::testmode::capture(args.len() as isize).map(|c| c.0), socle::testmode::foreground());
        let mut raw = egui::RawInput::default();
        socle::testmode::prepare_input(&mut raw);
    }
    #[cfg(feature = "update")]
    {
        let u = socle::update::Updater { repo: "a/b", current: "0.1.0", asset_suffix: ".zip", exe_name: None };
        if args.len() > 5 {
            println!("{:?}", u.check());
            if let Ok(Some(r)) = u.parse(args[5].as_bytes()) {
                println!("{:?}", u.install(&r, &|_| {}));
            }
        }
        socle::update::cleanup_old();
    }
}
