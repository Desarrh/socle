//! Palette de commandes **Ctrl+K** (QoL T5) : composant générique. L'app fournit la liste de
//! ses commandes (id, libellé, groupe, raccourci, icône) ; la palette filtre en flou, met les
//! commandes récentes en tête et renvoie l'id choisi. L'app exécute ensuite l'action.
//!
//! Les apps peuvent lire `query()` pour proposer des commandes dynamiques (ex. Vigie : `:3000`
//! → « Tuer le processus qui écoute sur 3000 »).

use crate::fuzzy::Index;
use crate::icons::{self, Icon};
use crate::theme::Palette as Colors;
use egui::{Align2, CornerRadius, FontId, Frame, Key, Modifiers, Rect, RichText, Sense, Stroke, StrokeKind, pos2, vec2};

/// Une commande proposée par l'app.
#[derive(Clone, Debug)]
pub struct Command {
    /// Identifiant renvoyé quand la commande est choisie.
    pub id: String,
    pub label: String,
    /// Groupe affiché à droite (« Fichier », « Vue »…) ; compte aussi dans la recherche.
    pub group: String,
    /// Raccourci affiché (« Ctrl+Maj+N »), vide si aucun.
    pub keys: String,
    pub icon: Option<Icon>,
    /// Grisée et non exécutable.
    pub enabled: bool,
}

impl Command {
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Command { id: id.into(), label: label.into(), group: String::new(), keys: String::new(), icon: None, enabled: true }
    }
    pub fn group(mut self, g: impl Into<String>) -> Self {
        self.group = g.into();
        self
    }
    pub fn keys(mut self, k: impl Into<String>) -> Self {
        self.keys = k.into();
        self
    }
    pub fn icon(mut self, i: Icon) -> Self {
        self.icon = Some(i);
        self
    }
    pub fn enabled(mut self, on: bool) -> Self {
        self.enabled = on;
        self
    }
}

/// État de la palette (un par app).
#[derive(Default)]
pub struct Palette {
    open: bool,
    query: String,
    sel: usize,
    fresh: bool,
    /// Ids des dernières commandes exécutées (la plus récente d'abord).
    pub recent: Vec<String>,
}

const MAX_ROWS: usize = 12;

impl Palette {
    pub fn is_open(&self) -> bool {
        self.open
    }
    pub fn open(&mut self) {
        self.open = true;
        self.fresh = true;
        self.query.clear();
        self.sel = 0;
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    pub fn toggle(&mut self) {
        if self.open { self.close() } else { self.open() }
    }
    /// Saisie courante (pour les commandes dynamiques).
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Ctrl+K ouvre / ferme. À appeler une fois par image, avant `show`.
    pub fn handle_shortcut(&mut self, ctx: &egui::Context) -> bool {
        let hit = ctx.input_mut(|i| i.consume_key(Modifiers::COMMAND, Key::K));
        if hit {
            self.toggle();
        }
        hit
    }

    /// Classe les commandes pour la saisie courante (indices dans `commands`).
    pub fn rank(&self, commands: &[Command]) -> Vec<usize> {
        let boost = |c: &Command| self.recent.iter().position(|r| *r == c.id).map_or(0, |i| 300 - 15 * i.min(19) as i32);
        if self.query.trim().is_empty() {
            let mut v: Vec<usize> = (0..commands.len()).collect();
            v.sort_by_key(|&i| (-boost(&commands[i]), i));
            return v;
        }
        // Libellé d'abord ; « groupe libellé » en secours (« vue grille »).
        let mut ix = Index::new();
        for c in commands {
            ix.push(&c.label, boost(c));
        }
        let mut hits = ix.search(&self.query, commands.len());
        let mut gx = Index::new();
        for c in commands {
            gx.push(&format!("{} {}", c.group, c.label), boost(c) - 400);
        }
        for h in gx.search(&self.query, commands.len()) {
            if !hits.iter().any(|x| x.index == h.index) {
                hits.push(h);
            }
        }
        hits.sort_by_key(|h| (-h.score, h.index));
        hits.into_iter().map(|h| h.index).collect()
    }

    /// Dessine la palette si elle est ouverte. Renvoie l'id de la commande choisie (Entrée ou clic).
    pub fn show(&mut self, ctx: &egui::Context, p: &Colors, commands: &[Command]) -> Option<String> {
        if !self.open {
            return None;
        }
        let order = self.rank(commands);
        let shown = order.len().min(MAX_ROWS * 4);
        if self.sel >= shown {
            self.sel = shown.saturating_sub(1);
        }
        let (up, down, enter) = ctx.input_mut(|i| {
            (i.consume_key(Modifiers::NONE, Key::ArrowUp), i.consume_key(Modifiers::NONE, Key::ArrowDown), i.key_pressed(Key::Enter))
        });
        if up {
            self.sel = self.sel.saturating_sub(1);
        }
        if down && self.sel + 1 < shown {
            self.sel += 1;
        }
        let mut chosen: Option<usize> = None;
        if enter {
            chosen = order.get(self.sel).copied();
        }
        let query = self.query.clone();
        let sel = self.sel;
        let fresh = std::mem::take(&mut self.fresh);
        let mut new_sel = None;
        let w = 640.0;
        let m = super::modal(ctx, "socle-palette", p, w, 90.0, |ui| {
            let w = ui.available_width();
            ui.horizontal(|ui| {
                let (ir, _) = ui.allocate_exact_size(vec2(22.0, 22.0), Sense::hover());
                icons::paint(ui.painter(), ir, Icon::Search, p.accent);
                let r = ui.add(
                    egui::TextEdit::singleline(&mut self.query)
                        .id(egui::Id::unique("socle-palette-saisie"))
                        .hint_text("Rechercher une commande…")
                        .frame(Frame::NONE)
                        .font(FontId::proportional(17.0))
                        .desired_width(f32::INFINITY),
                );
                if fresh || !r.has_focus() {
                    r.request_focus();
                }
                if r.changed() {
                    new_sel = Some(0);
                }
            });
            ui.add_space(6.0);
            if order.is_empty() {
                ui.label(RichText::new("Aucune commande ne correspond.").color(p.muted));
            }
            let rows = order.len().min(shown);
            egui::ScrollArea::vertical().max_height(40.0 * MAX_ROWS as f32).auto_shrink([false, true]).show(ui, |ui| {
                for (k, &ci) in order.iter().take(rows).enumerate() {
                    let c = &commands[ci];
                    let (rect, resp) = ui.allocate_exact_size(vec2(w, 40.0), Sense::click());
                    let on = k == sel;
                    if on {
                        resp.scroll_to_me(None);
                    }
                    if on || resp.hovered() {
                        ui.painter().rect_filled(rect, CornerRadius::same(8), if on { p.sel_bg } else { p.hover_bg });
                    }
                    let fg = if c.enabled { p.text } else { p.muted.gamma_multiply(0.7) };
                    if let Some(ic) = c.icon {
                        icons::paint(ui.painter(), Rect::from_center_size(pos2(rect.left() + 20.0, rect.center().y), vec2(18.0, 18.0)), ic, if c.enabled { p.accent } else { fg });
                    }
                    // raccourci en « touches »
                    let mut right = rect.right() - 10.0;
                    for key in c.keys.split('+').rev().filter(|k| !k.is_empty()) {
                        let g = ui.painter().layout_no_wrap(key.to_string(), FontId::monospace(11.0), p.muted);
                        let kw = g.size().x + 10.0;
                        let kr = Rect::from_min_size(pos2(right - kw, rect.center().y - 10.0), vec2(kw, 20.0));
                        ui.painter().rect(kr, CornerRadius::same(5), p.soft, Stroke::new(1.0, p.border), StrokeKind::Inside);
                        ui.painter().galley(pos2(kr.left() + 5.0, kr.center().y - g.size().y / 2.0), g, p.muted);
                        right = kr.left() - 4.0;
                    }
                    if !c.group.is_empty() {
                        let g = ui.painter().layout_no_wrap(c.group.clone(), FontId::proportional(11.5), p.muted);
                        right -= 8.0;
                        ui.painter().galley(pos2(right - g.size().x, rect.center().y - g.size().y / 2.0), g.clone(), p.muted);
                        right -= g.size().x;
                    }
                    let x = rect.left() + 40.0;
                    let g = super::highlighted(ui, &c.label, &query, FontId::proportional(14.5), fg, p.accent, right - x - 12.0);
                    ui.painter().galley(pos2(x, rect.center().y - g.size().y / 2.0), g, fg);
                    if resp.clicked() {
                        chosen = Some(ci);
                        new_sel = Some(k);
                    }
                }
            });
            ui.add_space(4.0);
            ui.painter().text(
                pos2(ui.min_rect().left() + 4.0, ui.cursor().top() + 8.0),
                Align2::LEFT_CENTER,
                "↑↓ choisir · Entrée exécuter · Échap fermer · Ctrl+K ouvrir/fermer",
                FontId::proportional(11.5),
                p.muted,
            );
            ui.add_space(16.0);
        });
        if let Some(s) = new_sel {
            self.sel = s;
        }
        if m.close {
            self.close();
            return None;
        }
        let c = chosen.and_then(|i| commands.get(i)).filter(|c| c.enabled)?;
        self.recent.retain(|r| *r != c.id);
        self.recent.insert(0, c.id.clone());
        self.recent.truncate(20);
        self.close();
        Some(c.id.clone())
    }
}

/// Fenêtre d'aide **F1** : toutes les commandes qui ont un raccourci, par groupe.
/// `open` repasse à faux quand l'utilisateur ferme (Échap, clic sur le fond, F1).
pub fn help_window(ctx: &egui::Context, p: &Colors, open: &mut bool, commands: &[Command]) {
    if ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::F1)) {
        *open = !*open;
    }
    if !*open {
        return;
    }
    let mut groups: Vec<&str> = Vec::new();
    for c in commands.iter().filter(|c| !c.keys.is_empty()) {
        if !groups.contains(&c.group.as_str()) {
            groups.push(&c.group);
        }
    }
    let m = super::modal(ctx, "socle-aide", p, 560.0, 70.0, |ui| {
        ui.label(RichText::new("Raccourcis clavier").size(crate::theme::taille::TITRE).strong());
        ui.add_space(6.0);
        egui::ScrollArea::vertical().max_height(ctx.content_rect().height() - 200.0).show(ui, |ui| {
            for g in &groups {
                if !g.is_empty() {
                    super::section_header(ui, p, g);
                }
                egui::Grid::new(("socle-aide", *g)).num_columns(2).spacing(vec2(16.0, 6.0)).show(ui, |ui| {
                    for c in commands.iter().filter(|c| !c.keys.is_empty() && c.group == *g) {
                        ui.label(RichText::new(&c.keys).monospace().color(p.accent));
                        ui.label(&c.label);
                        ui.end_row();
                    }
                });
                ui.add_space(6.0);
            }
        });
    });
    if m.close {
        *open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classement_et_recents() {
        let cmds = vec![
            Command::new("grille", "Afficher en grille").group("Vue").keys("Ctrl+Maj+G"),
            Command::new("liste", "Afficher en liste").group("Vue"),
            Command::new("nouveau", "Nouveau dossier").group("Fichier"),
            Command::new("theme", "Basculer le thème sombre").group("Vue"),
        ];
        let mut pal = Palette::default();
        pal.query = "nouv dos".into();
        assert_eq!(pal.rank(&cmds)[0], 2);
        pal.query = "grile".into(); // faute
        assert_eq!(pal.rank(&cmds)[0], 0);
        pal.query = "vue liste".into(); // par le groupe
        assert_eq!(pal.rank(&cmds)[0], 1);
        pal.query.clear();
        pal.recent = vec!["theme".into()];
        assert_eq!(pal.rank(&cmds)[0], 3, "les récentes d'abord");
    }
}
