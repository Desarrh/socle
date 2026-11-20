//! Vérifie `socle::testmode` avec une vraie fenêtre winit + egui (rendu CPU des apps) qui doit
//! continuer à se redessiner, avec `hide` (invisible à l'écran). La v0.1.1 plaçait la fenêtre
//! hors écran : Windows ne la redessinait plus et la capture sortait blanche à 85 %.
//!
//! `cargo run --release --example testmode_check --features "theme testmode"`
//!
//! Respecte la règle n° 0 : fenêtre créée invisible et sans focus, jamais visible à l'écran,
//! priorité basse, fermeture automatique après ~1,5 s.

use egui_software_backend::{BufferMutRef, ColorFieldOrder, EguiSoftwareRender};
use std::num::NonZeroU32;
use std::rc::Rc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::{Window, WindowId};

struct Gfx {
    window: Rc<Window>,
    egui: egui_winit::State,
    _context: softbuffer::Context<Rc<Window>>,
    surface: softbuffer::Surface<Rc<Window>, Rc<Window>>,
    renderer: EguiSoftwareRender,
    hwnd: isize,
}

struct Check {
    ctx: egui::Context,
    gfx: Option<Gfx>,
    frames: u32,
    start: Instant,
    focus: isize,
    result: Option<String>,
}

const BG: [u8; 3] = [0x0F, 0x1B, 0x2D];

impl ApplicationHandler for Check {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        let attrs = Window::default_attributes().with_title("socle testmode").with_inner_size(winit::dpi::PhysicalSize::new(400, 300)).with_active(false).with_visible(false);
        let window = Rc::new(el.create_window(attrs).expect("fenêtre"));
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get(),
            _ => 0,
        };
        socle::testmode::hide(hwnd);
        let context = softbuffer::Context::new(window.clone()).expect("softbuffer");
        let surface = softbuffer::Surface::new(&context, window.clone()).expect("surface");
        let egui = egui_winit::State::new(self.ctx.clone(), egui::ViewportId::ROOT, &*window, Some(1.0), None, None);
        window.request_redraw();
        self.gfx = Some(Gfx { window, egui, _context: context, surface, renderer: EguiSoftwareRender::new(ColorFieldOrder::Bgra), hwnd });
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some(g) = self.gfx.as_mut() else { return };
        let _ = g.egui.on_window_event(&g.window, &event);
        if let WindowEvent::RedrawRequested = event {
            let size = g.window.inner_size();
            let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height)) else { return };
            let raw = g.egui.take_egui_input(&g.window);
            let n = self.frames;
            let out = self.ctx.run_ui(raw, |ui| {
                // un carré rouge qui se déplace : prouve que l'app se redessine
                let x = (n % 200) as f32;
                ui.painter().rect_filled(egui::Rect::from_min_size(egui::pos2(x, 100.0), egui::vec2(60.0, 60.0)), 0.0, egui::Color32::RED);
            });
            let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
            if g.surface.resize(w, h).is_ok() {
                if let Ok(mut buf) = g.surface.buffer_mut() {
                    buf.fill(((BG[0] as u32) << 16) | ((BG[1] as u32) << 8) | BG[2] as u32);
                    let px: &mut [[u8; 4]] = bytemuck::cast_slice_mut(&mut buf[..]);
                    let mut t = BufferMutRef::new(px, w.get() as usize, h.get() as usize);
                    g.renderer.render(&mut t, &prims, &out.textures_delta, out.pixels_per_point);
                    let _ = buf.present();
                }
            }
            self.frames += 1;
            if self.start.elapsed() < Duration::from_millis(1500) {
                g.window.request_redraw();
            } else if self.result.is_none() {
                let shot = socle::testmode::capture(g.hwnd);
                let (white, bg, red) = match &shot {
                    Some((_, _, px)) => {
                        let n = (px.len() / 4) as f32;
                        let c = |f: &dyn Fn(&[u8]) -> bool| px.chunks_exact(4).filter(|p| f(p)).count() as f32 / n * 100.0;
                        (c(&|p| p[0] > 240 && p[1] > 240 && p[2] > 240), c(&|p| p[0] == BG[0] && p[1] == BG[1] && p[2] == BG[2]), c(&|p| p[0] > 200 && p[1] < 40 && p[2] < 40))
                    }
                    None => (0.0, 0.0, 0.0),
                };
                self.result = Some(format!(
                    "{} : {} images en 1,5 s · capture {} · blanc {white:.0} % · fond {bg:.0} % · rouge {red:.1} % · focus inchangé : {}",
                    "invisible à l'écran (hide)",
                    self.frames,
                    if shot.is_some() { "ok" } else { "échec" },
                    socle::testmode::foreground() == self.focus
                ));
                el.exit();
            }
        }
    }

    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        // garde-fou : si Windows ne demande plus de dessin, on s'arrête quand même
        if self.start.elapsed() > Duration::from_millis(2500) && self.result.is_none() {
            let shot = self.gfx.as_ref().and_then(|g| socle::testmode::capture(g.hwnd));
            let white = shot.as_ref().map(|(_, _, px)| px.chunks_exact(4).filter(|p| p[0] > 240 && p[1] > 240 && p[2] > 240).count() * 100 / (px.len() / 4)).unwrap_or(0);
            self.result = Some(format!("{} : figée après {} images · capture blanche à {white} %", "invisible à l'écran (hide)", self.frames));
            el.exit();
            return;
        }
        el.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(100)));
    }
}

fn main() {
    socle::testmode::low_priority();
    let el = EventLoop::new().expect("boucle");
    let mut c = Check { ctx: egui::Context::default(), gfx: None, frames: 0, start: Instant::now(), focus: socle::testmode::foreground(), result: None };
    let _ = el.run_app(&mut c);
    println!("{}", c.result.unwrap_or_else(|| "pas de résultat".into()));
}
