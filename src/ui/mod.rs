//! Composants communs (DIRECTION.md §3) : texte tronqué, toasts de barre d'état, contrôle
//! segmenté, modale sur fond assombri, champ de recherche, champ d'autocomplétion et palette
//! de commandes Ctrl+K.
//!
//! Règle des apps respectée : les composants **renvoient** ce qui s'est passé (clic, choix,
//! validation) ; c'est l'app qui applique l'action ensuite.

mod palette;
pub use palette::{Command, Palette, help_window};

use crate::autocomplete::{Autocomplete, Suggestion};
use crate::icons::{self, Icon};
use crate::theme::Palette as Colors;
use egui::{Align2, Color32, CornerRadius, FontId, Frame, Key, Margin, Modifiers, Pos2, Rect, Response, Sense, Stroke, StrokeKind, Ui, pos2, vec2};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------------------------
// Texte
// ---------------------------------------------------------------------------------------------

/// Texte sur une ligne, tronqué avec « … » au-delà de `max_w` (explorateur `ui::trunc`).
pub fn trunc(ui: &Ui, text: &str, font: FontId, color: Color32, max_w: f32) -> Arc<egui::Galley> {
    let mut job = egui::text::LayoutJob::simple_singleline(text.to_owned(), font, color);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(8.0));
    ui.painter().layout_job(job)
}

/// Peint un texte tronqué, aligné à gauche à `x`, centré verticalement dans `rect`.
pub fn paint_text_left(ui: &Ui, rect: Rect, x: f32, text: &str, font: FontId, color: Color32, max_w: f32) {
    let g = trunc(ui, text, font, color, max_w);
    let y = rect.center().y - g.size().y / 2.0;
    ui.painter().galley(pos2(x, y), g, color);
}

/// Peint un texte aligné à droite sur `right`, centré verticalement dans `rect`.
pub fn paint_text_right(ui: &Ui, rect: Rect, right: f32, text: &str, font: FontId, color: Color32) {
    ui.painter().text(pos2(right, rect.center().y), Align2::RIGHT_CENTER, text, font, color);
}

/// Durée écoulée lisible : « 3 s », « 2 min », « 5 h », « 4 j ».
pub fn ago(secs: f32) -> String {
    let s = secs.max(0.0) as u64;
    match s {
        0..60 => format!("{s} s"),
        60..3600 => format!("{} min", s / 60),
        3600..86_400 => format!("{} h", s / 3600),
        _ => format!("{} j", s / 86_400),
    }
}

/// Texte avec les caractères trouvés par la recherche floue en couleur d'accent (et en gras si
/// la famille `bold` est installée), tronqué à `max_w`.
pub fn highlighted(ui: &Ui, text: &str, query: &str, font: FontId, color: Color32, accent: Color32, max_w: f32) -> Arc<egui::Galley> {
    let marks = crate::fuzzy::highlight(text, query);
    let mut job = egui::text::LayoutJob::default();
    let mut on = false;
    let mut run = String::new();
    let flush = |job: &mut egui::text::LayoutJob, run: &mut String, on: bool| {
        if !run.is_empty() {
            let fmt = egui::TextFormat { font_id: font.clone(), color: if on { accent } else { color }, ..Default::default() };
            job.append(run, 0.0, fmt);
            run.clear();
        }
    };
    let mut k = 0;
    for (i, c) in text.chars().enumerate() {
        let m = k < marks.len() && marks[k] == i;
        if m {
            k += 1;
        }
        if m != on {
            flush(&mut job, &mut run, on);
            on = m;
        }
        run.push(c);
    }
    flush(&mut job, &mut run, on);
    job.wrap = egui::text::TextWrapping::truncate_at_width(max_w.max(8.0));
    ui.painter().layout_job(job)
}

/// En-tête de section : 11 px, MAJUSCULES, `muted`.
pub fn section_header(ui: &mut Ui, p: &Colors, text: &str) -> Response {
    ui.label(egui::RichText::new(text.to_uppercase()).size(crate::theme::taille::ENTETE).strong().color(p.muted))
}

// ---------------------------------------------------------------------------------------------
// Toasts
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToastKind {
    Info,
    Ok,
    Error,
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub msg: String,
    pub kind: ToastKind,
    born: Instant,
    ttl: Duration,
}

/// File de messages éphémères affichés dans la barre d'état (le plus récent d'abord).
/// Aucun sondage : un seul `request_repaint_after` jusqu'à la prochaine expiration.
#[derive(Default)]
pub struct Toasts {
    items: VecDeque<Toast>,
}

impl Toasts {
    pub fn push(&mut self, msg: impl Into<String>, kind: ToastKind) {
        let ttl = Duration::from_secs(if kind == ToastKind::Error { 8 } else { 4 });
        self.items.push_front(Toast { msg: msg.into(), kind, born: Instant::now(), ttl });
        self.items.truncate(4);
    }
    pub fn info(&mut self, msg: impl Into<String>) {
        self.push(msg, ToastKind::Info);
    }
    pub fn ok(&mut self, msg: impl Into<String>) {
        self.push(msg, ToastKind::Ok);
    }
    pub fn error(&mut self, msg: impl Into<String>) {
        self.push(msg, ToastKind::Error);
    }
    /// Message affiché en ce moment.
    pub fn current(&self) -> Option<&Toast> {
        self.items.iter().find(|t| t.born.elapsed() < t.ttl)
    }

    /// Dessine le toast courant (à placer dans la barre d'état). Il s'estompe sur ses 400
    /// dernières ms. Un clic le ferme.
    pub fn ui(&mut self, ui: &mut Ui, p: &Colors) {
        self.items.retain(|t| t.born.elapsed() < t.ttl);
        let Some(t) = self.items.front() else { return };
        let left = t.ttl.saturating_sub(t.born.elapsed());
        let alpha = (left.as_secs_f32() / 0.4).min(1.0);
        let (color, icon) = match t.kind {
            ToastKind::Info => (p.text, Icon::Info),
            ToastKind::Ok => (p.add, Icon::Check),
            ToastKind::Error => (p.del, Icon::Warning),
        };
        let color = color.gamma_multiply(alpha);
        let r = ui
            .horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(14.0, 14.0), Sense::hover());
                icons::paint(ui.painter(), ir, icon, color);
                ui.label(egui::RichText::new(&t.msg).color(color).size(crate::theme::taille::ETAT))
            })
            .inner;
        if r.interact(Sense::click()).clicked() {
            self.items.pop_front();
        }
        // réveil pour le fondu puis l'expiration, sans sondage
        let wake = if left > Duration::from_millis(400) { left - Duration::from_millis(400) } else { Duration::from_millis(16) };
        ui.ctx().request_repaint_after(wake);
    }
}

// ---------------------------------------------------------------------------------------------
// Contrôle segmenté
// ---------------------------------------------------------------------------------------------

/// Contrôle segmenté (Bureau / Dev de l'explorateur). Renvoie vrai si la sélection a changé.
pub fn segmented(ui: &mut Ui, p: &Colors, current: &mut usize, items: &[(Option<Icon>, &str)]) -> bool {
    let mut changed = false;
    Frame::new().fill(p.soft).corner_radius(CornerRadius::same(9)).inner_margin(Margin::same(3)).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            for (i, (icon, label)) in items.iter().enumerate() {
                let on = i == *current;
                let font = FontId::proportional(crate::theme::taille::TEXTE);
                let color = if on { p.text } else { p.muted };
                let g = ui.painter().layout_no_wrap(label.to_string(), font, color);
                let iw = if icon.is_some() { 22.0 } else { 0.0 };
                let w = g.size().x + iw + 20.0;
                let (rect, r) = ui.allocate_exact_size(vec2(w, 28.0), Sense::click());
                let fill = if on { p.seg_on() } else if r.hovered() { p.hover_bg } else { Color32::TRANSPARENT };
                ui.painter().rect_filled(rect, CornerRadius::same(7), fill);
                let mut x = rect.left() + 10.0;
                if let Some(ic) = icon {
                    icons::paint(ui.painter(), Rect::from_center_size(pos2(x + 8.0, rect.center().y), vec2(16.0, 16.0)), *ic, color);
                    x += iw;
                }
                ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, color);
                if r.clicked() && !on {
                    *current = i;
                    changed = true;
                }
            }
        });
    });
    changed
}

// ---------------------------------------------------------------------------------------------
// Modale
// ---------------------------------------------------------------------------------------------

/// Résultat d'une modale.
pub struct Modal<R> {
    pub inner: R,
    /// Clic sur le fond ou Échap : l'app doit fermer la modale.
    pub close: bool,
}

/// Fenêtre modale sur fond assombri, centrée horizontalement, à `top` px du haut.
/// Échap et clic sur le fond demandent la fermeture.
pub fn modal<R>(ctx: &egui::Context, id: &str, p: &Colors, width: f32, top: f32, add: impl FnOnce(&mut Ui) -> R) -> Modal<R> {
    let screen = ctx.content_rect();
    let w = width.min(screen.width() - 32.0);
    let esc = ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape));
    let backdrop = egui::Area::new(egui::Id::unique((id, "fond"))).order(egui::Order::Foreground).fade_in(false).fixed_pos(screen.min).show(ctx, |ui| {
        let (r, resp) = ui.allocate_exact_size(screen.size(), Sense::click());
        ui.painter().rect_filled(r, CornerRadius::ZERO, Color32::from_black_alpha(90));
        resp.clicked()
    });
    let inner = egui::Area::new(egui::Id::unique(id))
        .order(egui::Order::Tooltip)
        .fade_in(false) // apparition immédiate : réponse en moins d'une image
        .fixed_pos(pos2(screen.center().x - w / 2.0, screen.top() + top))
        .show(ctx, |ui| {
            Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(crate::theme::arrondi::CARTE))
                .inner_margin(Margin::same(10))
                .shadow(egui::Shadow { offset: [0, 12], blur: 40, spread: 0, color: Color32::from_black_alpha(70) })
                .show(ui, |ui| {
                    ui.set_width(w);
                    add(ui)
                })
                .inner
        })
        .inner;
    Modal { inner, close: esc || backdrop.inner }
}

// ---------------------------------------------------------------------------------------------
// Champ de recherche
// ---------------------------------------------------------------------------------------------

/// Champ de recherche : loupe à gauche, × pour vider à droite, fond `code`, bord `border`
/// (accent quand il a le focus). Renvoie la réponse du `TextEdit`.
pub fn search_field(ui: &mut Ui, p: &Colors, id: egui::Id, text: &mut String, hint: &str, width: f32) -> Response {
    let h = 32.0;
    let (rect, _) = ui.allocate_exact_size(vec2(width, h), Sense::hover());
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(rect, CornerRadius::same(8), p.code, Stroke::new(1.0, if focused { p.accent } else { p.border }), StrokeKind::Inside);
    icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.left() + 16.0, rect.center().y), vec2(16.0, 16.0)), Icon::Search, p.muted);
    let clear_w = if text.is_empty() { 0.0 } else { 26.0 };
    let er = Rect::from_min_max(pos2(rect.left() + 30.0, rect.top()), pos2(rect.right() - 6.0 - clear_w, rect.bottom()));
    let resp = ui.put(
        er,
        egui::TextEdit::singleline(text).id(id).hint_text(hint).frame(Frame::NONE).font(FontId::proportional(crate::theme::taille::TEXTE)).align(egui::Align2::LEFT_CENTER),
    );
    if !text.is_empty() {
        let cr = Rect::from_center_size(pos2(rect.right() - 17.0, rect.center().y), vec2(22.0, 22.0));
        let c = ui.interact(cr, id.with("vider"), Sense::click());
        if c.hovered() {
            ui.painter().rect_filled(cr, CornerRadius::same(6), p.hover_bg);
        }
        icons::paint(ui.painter(), cr.shrink(5.0), Icon::Close, p.muted);
        if c.on_hover_text("Effacer (Échap)").clicked() {
            text.clear();
            resp.request_focus();
        }
    }
    if focused && !text.is_empty() && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Escape)) {
        text.clear();
    }
    resp
}

// ---------------------------------------------------------------------------------------------
// Champ d'autocomplétion
// ---------------------------------------------------------------------------------------------

/// État d'un champ d'autocomplétion (à garder dans l'app, un par champ).
#[derive(Default)]
pub struct AutoField {
    pub suggestions: Vec<Suggestion>,
    /// Suggestion surlignée (↑↓), `None` = la saisie.
    pub sel: Option<usize>,
    last: Option<String>,
    open: bool,
}

/// Ce qui s'est passé dans le champ.
pub struct AutoResponse {
    pub response: Response,
    /// Entrée : texte validé (déjà enregistré dans l'historique par l'app si elle le veut).
    pub submitted: Option<String>,
    /// Tab a complété le texte.
    pub completed: bool,
}

/// Champ de recherche avec suggestions floues (historique + candidats) dans une liste
/// déroulante. ↑↓ choisit, **Tab complète**, Entrée valide, Échap ferme la liste.
#[allow(clippy::too_many_arguments)]
pub fn autocomplete_field(ui: &mut Ui, p: &Colors, id: egui::Id, state: &mut AutoField, ac: &Autocomplete, text: &mut String, hint: &str, width: f32) -> AutoResponse {
    let focused = ui.memory(|m| m.has_focus(id));
    let mut completed = false;
    let mut submitted = None;
    // Touches lues AVANT le TextEdit (sinon Tab ferait perdre le focus, ↑↓ bougeraient le curseur).
    if focused && state.open && !state.suggestions.is_empty() {
        let n = state.suggestions.len();
        let (up, down, tab, esc) = ui.input_mut(|i| {
            (
                i.consume_key(Modifiers::NONE, Key::ArrowUp),
                i.consume_key(Modifiers::NONE, Key::ArrowDown),
                i.consume_key(Modifiers::NONE, Key::Tab),
                i.consume_key(Modifiers::NONE, Key::Escape),
            )
        });
        if down {
            state.sel = Some(state.sel.map_or(0, |s| (s + 1).min(n - 1)));
        }
        if up {
            state.sel = state.sel.and_then(|s| s.checked_sub(1));
        }
        if esc {
            state.open = false;
        }
        if tab {
            if let Some(t) = ac.complete(text, &state.suggestions, state.sel) {
                *text = t;
                completed = true;
                state.sel = None;
                set_cursor_end(ui.ctx(), id, text);
            }
        }
    } else if focused && ui.input_mut(|i| i.consume_key(Modifiers::NONE, Key::ArrowDown)) {
        state.open = true;
    }
    let resp = search_field(ui, p, id, text, hint, width);
    if resp.has_focus() && ui.input(|i| i.key_pressed(Key::Enter)) || resp.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
        let t = state.sel.and_then(|s| state.suggestions.get(s)).map(|s| s.text.clone()).unwrap_or_else(|| text.clone());
        *text = t.clone();
        submitted = Some(t);
        state.open = false;
        state.sel = None;
    }
    if resp.changed() {
        state.open = true;
        state.sel = None;
    }
    // Suggestions recalculées seulement quand la saisie change (< 1 ms, souvent ~µs).
    if state.last.as_deref() != Some(text.as_str()) {
        state.suggestions = ac.suggest(text, 8);
        state.last = Some(text.clone());
    }
    let show = (resp.has_focus() || completed) && state.open && !state.suggestions.is_empty();
    if show {
        let top_left = resp.rect.left_bottom() + vec2(-30.0, 6.0);
        let w = width;
        let mut clicked = None;
        egui::Area::new(id.with("liste")).order(egui::Order::Foreground).fade_in(false).fixed_pos(top_left).show(ui.ctx(), |ui| {
            Frame::new()
                .fill(p.surface)
                .stroke(Stroke::new(1.0, p.border))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::same(4))
                .shadow(egui::Shadow { offset: [0, 6], blur: 18, spread: 0, color: Color32::from_black_alpha(50) })
                .show(ui, |ui| {
                    ui.set_width(w - 8.0);
                    for (i, s) in state.suggestions.iter().enumerate() {
                        let (rect, r) = ui.allocate_exact_size(vec2(w - 8.0, 28.0), Sense::click());
                        let on = state.sel == Some(i);
                        if on || r.hovered() {
                            ui.painter().rect_filled(rect, CornerRadius::same(6), if on { p.sel_bg } else { p.hover_bg });
                        }
                        let ic = if s.past { Icon::History } else { Icon::Search };
                        icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.left() + 14.0, rect.center().y), vec2(14.0, 14.0)), ic, p.muted);
                        let g = highlighted(ui, &s.text, text, FontId::proportional(crate::theme::taille::TEXTE), p.text, p.accent, rect.width() - 40.0);
                        ui.painter().galley(pos2(rect.left() + 30.0, rect.center().y - g.size().y / 2.0), g, p.text);
                        if r.clicked() {
                            clicked = Some(i);
                        }
                    }
                    ui.label(egui::RichText::new("Tab compléter · ↑↓ choisir · Entrée valider").size(11.0).color(p.muted));
                });
        });
        if let Some(i) = clicked {
            let t = state.suggestions[i].text.clone();
            *text = t.clone();
            submitted = Some(t);
            state.open = false;
            resp.request_focus();
            set_cursor_end(ui.ctx(), id, text);
        }
    }
    AutoResponse { response: resp, submitted, completed }
}

/// Place le curseur en fin de texte (après une complétion).
pub(crate) fn set_cursor_end(ctx: &egui::Context, id: egui::Id, text: &str) {
    // état absent (1ʳᵉ ouverture) : on le crée, sinon le curseur partirait au début
    let mut st = egui::TextEdit::load_state(ctx, id).unwrap_or_default();
    let end = egui::text::CCursor::new(text.chars().count());
    st.cursor.set_char_range(Some(egui::text::CCursorRange::one(end)));
    st.store(ctx, id);
}

/// Point d'ancrage utile : coin d'un rectangle décalé (pour placer un menu ⋯).
pub fn below(r: Rect, dx: f32) -> Pos2 {
    r.left_bottom() + vec2(dx, 4.0)
}
