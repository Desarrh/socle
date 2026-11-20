# Migration vers le socle

Guide court pour la session de chaque app. **Le socle ne modifie aucune app** : chacune migre à son rythme, et peut le faire par morceaux (thème d'abord, le reste ensuite).

## 0. Comment dépendre du socle (proposition, à valider)

`explorateur` et `lecteur-universel` sont publics, avec une CI GitHub : une dépendance `path = "../socle"` ferait échouer leur build CI, car le dossier n'existe pas sur le runner.

**Option recommandée : dépôt public `Desarrh/socle` + dépendance git épinglée par tag.**

```toml
# Cargo.toml de l'app
socle = { git = "https://github.com/Desarrh/socle", tag = "v0.1.1", default-features = false, features = ["ui", "settings", "crash", "instance"] }
```

Pour développer le socle et une app en même temps, sans toucher au `Cargo.toml` ni à la CI, chaque app ajoute un fichier **non versionné** `.cargo/config.toml` (à mettre dans `.gitignore`) :

```toml
[patch."https://github.com/Desarrh/socle"]
socle = { path = "../socle" }
```

En local, Cargo prend `C:\Dev\socle`. En CI, il prend le tag. Une montée de version reste un changement explicite (`tag = "v0.2.0"`) : une app ne casse jamais à cause d'un changement du socle qu'elle n'a pas choisi.

Alternatives écartées :
- **`path` seul** : casse la CI des dépôts publics.
- **Copier le code (subtree, vendoring)** : on retrouve les copies qui divergent, ce que le socle doit justement supprimer.
- **crates.io** : publication publique et versions immuables, c'est lourd pour une crate interne.
- **Dépôt privé** : la CI des dépôts publics n'y aurait pas accès sans jeton.

Fait le 20/11 : dépôt public https://github.com/Desarrh/socle (MIT), tags `v0.1.0` puis `v0.1.1` (mode test). Chaque nouvelle version = un nouveau tag ; une app ne change de version qu'en modifiant son `tag = "…"`.

## 1. Explorateur

Features : `["ui", "images", "img-svg", "img-wic", "settings", "crash", "instance", "update"]` (+ `img-exr` / `img-jxl` si voulu).

| Avant | Après |
|---|---|
| `src/theme.rs` | supprimer ; `use socle::theme` (identique, plus la famille `bold` et la police emoji) |
| emoji de `fsx::icon_for` (📁 📄 🖼 📝 🎵 🎞 📦 ⚙ 📕) | `Icon::for_extension(&e.ext)` (et `Icon::Folder` pour un dossier) + `icons::paint(painter, rect, icon, couleur)` |
| barre d'outils ← → ↑ ⟳ 🔍 ⤵ ▦ ☰ ☀ 🌙 ⋯ ⎇ 📌 🖴 ▲▼ | `Icon::ArrowLeft/ArrowRight/ArrowUp/Refresh/Search/SearchDeep/Grid/ListView/Sun/Moon/More/Git/Pin/Drive/ChevronUp/ChevronDown` avec `icons::button(ui, icon, 32.0, Look::tool(&pal), "infobulle")` |
| Ctrl+1/2 impossibles en AZERTY | `socle::keys::consume(ctx, Modifiers::COMMAND, Key::Num1)` : touche physique, ça marche |
| `fsx::fuzzy_score` | `socle::fuzzy::score` ou, mieux, un `fuzzy::Index` construit une fois par dossier (accents, initiales, fautes en plus) |
| `trunc`, `paint_text_left/right`, `blend`, `ago` dans `ui.rs` | `socle::ui::{trunc, paint_text_left, paint_text_right, ago}`, `socle::theme::blend` |
| sélecteur Bureau / Dev | `socle::ui::segmented(ui, &pal, &mut idx, &[(Some(Icon::Grid), "Bureau"), (Some(Icon::Code), "Dev")])` |
| fond assombri de Ctrl+P / plein écran | `socle::ui::modal(...)` (Échap et clic à côté renvoient `close`) |
| filtre, barre d'adresse (QoL E4 / T4) | `socle::ui::autocomplete_field` + un `Autocomplete` par champ (candidats = sous-dossiers déjà chargés, historique dans `Settings`) |
| pas de Ctrl+K | `socle::ui::Palette` : `handle_shortcut` + `show` → id de commande → `Action`. **Ctrl+P reste « Aller à »** |
| `thumbs.rs` : `decode` Rust + fournisseur Windows | garder le fournisseur Windows (vidéos, PDF, Office) en dernier recours ; avant lui `socle::images::thumbnail_cached(&DiskCache::for_app("Explorateur"), path, taille, &cancel)`. Ou remplacer le pool maison par `socle::images::Pool` (LIFO + annulation au changement de dossier) |
| `winx::single` (inbox de fichiers sondée toutes les 250 ms) | `socle::instance::start("Explorateur", &args)` : plus de sondage, transmission < 1 ms. Supprimer `inbox\` et le thread de 250 ms |
| `updater.rs` | `socle::update::Updater { repo: "Desarrh/explorateur", current: env!("CARGO_PKG_VERSION"), asset_suffix: "windows-x64.zip", exe_name: None }`. `cleanup_old()` supprime aussi l'ancien `Explorateur.old`, `is_after_update()` reconnaît `--after-update` **et** `--apres-maj` |
| `install_crash_log` | `socle::crash::install("Explorateur", env!("CARGO_PKG_VERSION"))` puis, au lancement, `socle::crash::pending("Explorateur")` → toast |
| `Settings::save` (atomique) | `socle::settings::save("Explorateur", &s)` ; garder `#[serde(default)]` |

Attention : `windows` 0.58 (explorateur) et 0.62 (`img-wic`) coexisteraient. Passer l'explorateur en 0.62 (comme le lecteur et Tube) évite d'embarquer les deux.

## 2. Lecteur universel

Features : `["ui", "images", "img-exr", "img-svg", "img-jxl", "img-wic", "settings", "crash", "instance", "update"]`.

| Avant | Après |
|---|---|
| `src/theme.rs` | `socle::theme` (identique) |
| `src/icons.rs` | `socle::icons` : mêmes noms (`Icon::Play`… `Icon::Clean`), même dessin, mêmes `button` / `wide` (ce dernier prend `&socle::theme::Palette`) |
| `src/wic.rs` + `open_any` | `socle::images::decode(path, &cancel)` : `image` d'abord, puis PSD, RAW, SVG, EXR, JXL en Rust, et WIC en repli. Les messages « installez l'extension HEIF / AV1 / RAW » sont conservés. `encode_heic` reste dans le lecteur (sert aux tests) |
| `kind::ALL_EXTS` pour les images | compléter avec `socle::images::extensions()` (psd, psb, exr, svg, jxl, qoi, tga, hdr…) |
| `winx::forward_to_running` / `serve` (tube nommé) | `socle::instance::start("Lecteur", &args)` / `.listen(...)` : même principe, plus un mutex contre la course au démarrage. Le nom du tube change : une ancienne version ouverte ne recevra pas les fichiers d'une nouvelle (une seule fois, après la mise à jour) |
| `update.rs` | `socle::update::Updater { repo: "Desarrh/lecteur-universel", asset_suffix: "windows-x64.zip", exe_name: Some("lecteur-universel.exe"), .. }` ; `install` renvoie l'exe, puis `relaunch` |
| pas de Ctrl+K (T5) | `socle::ui::Palette` + `help_window` (F1) |
| pas de J/L | sans rapport avec le socle (convention média) ; lire les touches avec `socle::keys` pour les chiffres |

## 3. Vigie

Features : `["ui", "settings", "crash", "instance"]`.

| Avant | Après |
|---|---|
| `src/theme.rs` (CRLF, `#![allow(dead_code)]`) | `socle::theme` |
| emoji ⏸ ⏻ ⏾ ⊘ 🎮 🐳 👻 📈 🛡 🧹 ⚙ ☀ 🌙 🔍 ⋯ ▸ ▾ ▲▼ 📁 📷 🔊 🌳 ⟳ | `Icon::Pause/Power/Sleep/Block/Gamepad/Container/Ghost/Chart/Shield/Broom/Settings/Sun/Moon/Search/More/ChevronRight/ChevronDown/ChevronUp/Folder/Camera/Volume/Tree/Refresh` ; aussi `Cpu`, `Memory`, `Bolt` pour les graphiques |
| palette maison (`Palette2`, Ctrl+K / Ctrl+P) | `socle::ui::Palette` ; pour V1 « tuer par port », lire `palette.query()` (`:3000`) et ajouter une commande dynamique avant `show` |
| `already_running` (mutex + FindWindow) | `socle::instance::start("Vigie", &args)` pour la fenêtre. Ne pas l'appeler en mode sans fenêtre (`--avant-jeu` / `--apres-jeu`) ni après une élévation (même contournement qu'aujourd'hui) |
| `install_crash_log` | `socle::crash::install("Vigie", ...)`. Marche avec `panic = "abort"` |
| Ctrl+1–4 cassés en AZERTY | `socle::keys::consume_ctrl_digit(ctx)` (ou `keys::consume(ctx, Modifiers::COMMAND, Key::Num1)`) : touche physique pour les chiffres |

## 4. Tube

Features : `["ui", "settings", "crash", "instance"]` (+ `images` si les miniatures passent par le cache disque du socle).

| Avant | Après |
|---|---|
| `src/theme.rs` | `socle::theme` |
| `src/icons.rs` (`Ic`) | `socle::icons::Icon`. Correspondance : `Full` → `Fullscreen`, `ExitFull` → `Unfullscreen`, `Vol(n)` → `VolumeLevel(n)`, `Gear` → `Settings`, `List` → `Playlist`. Les autres gardent leur nom. Les icônes reprises de Tube sont ~18 % plus grandes qu'avant pour s'aligner sur la grille 24 : passer un rectangle plus petit si besoin |
| `Play/Pause/Close/Sun/Moon/Plus/Mute` | ce sont maintenant les dessins du lecteur (légères différences) |
| `already_running` (mutex + EnumWindows) | `socle::instance::start("Tube", &args)` : permet enfin `Tube.exe "recherche"` depuis Ultimate (intégration « Guide ») |
| réglages dans `%LOCALAPPDATA%\Tube` | la convention est `%APPDATA%\Tube\settings.json` : `socle::settings::load("Tube")`, en reprenant l'ancien fichier une fois s'il existe. Cookies, outils et cache restent dans `%LOCALAPPDATA%\Tube` (`socle::paths::cache_dir("Tube")`) |
| correctif AZERTY de `handle_keys` (touche physique des chiffres) | `socle::keys::pressed(ctx)` fait la même chose (même règle, reprise de Tube) |
| `smtc.rs` (panneau média Windows) | reste dans Tube pour l'instant ; candidat pour une feature `smtc` du socle quand le lecteur fera T7 |
| Y4 recherche locale dans l'historique | `socle::fuzzy::Index` sur les titres, ou `ui::autocomplete_field` |

## 5. Mode test (règle n° 0, toutes les apps)

Pendant les tests, l'utilisateur doit pouvoir se servir de son PC comme si de rien n'était. Feature `testmode` (`socle::testmode`) :

| Besoin | Outil |
|---|---|
| savoir si on est en test | `testmode::enabled()` : `--test-hors-ecran` ou `SOCLE_TEST=1` |
| fenêtre qui ne dérange pas | créer la fenêtre avec `with_active(false)` et `with_position(PhysicalPosition::new(-32000, -32000))`, puis `testmode::hide(hwnd)` juste après la création : hors écran, `WS_EX_NOACTIVATE`, hors de la barre des tâches. Le focus de l'utilisateur ne bouge pas (vérifié en test) |
| capture d'écran de l'app | `testmode::capture(hwnd)` → `(largeur, hauteur, RGBA)` par `PrintWindow`, sans toucher à l'écran. La fenêtre doit être affichée (hors écran suffit) |
| capture de l'interface seule | préférer le rendu **sans fenêtre** : egui + rendu CPU dans un tampon, puis PNG. Voir `capture()` dans `examples/demo.rs` (`--capture <dossier>`) |
| ne pas ralentir le PC | `testmode::low_priority()` au début d'un test ; lancer les compilations en priorité basse (`start /low cargo …` ou `Start-Process -Priority BelowNormal`) |
| instance unique pendant un test | utiliser un nom d'app de test (`socle::instance::start("MonAppTest123", …)`) pour ne jamais écrire dans l'instance ouverte de l'utilisateur |

Le socle ne fournit volontairement **aucune entrée simulée** (souris, clavier). Les événements se passent directement à egui (`RawInput.events`), comme dans la démo.

En mode test, pas de son ni de notification, et aucun réglage de l'utilisateur modifié : pointer les réglages vers un dossier temporaire (`socle::settings::save_to`).

## Ce qui ne change pas

- Le rendu (winit + softbuffer + hdrshot) reste dans chaque app ; la démo du socle en contient une copie.
- Les couleurs propres à une app (doré du lecteur, rouge et vert de Tube) restent dans l'app.
