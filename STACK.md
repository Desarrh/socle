# Socle — stack technique (document de référence)

Bibliothèque Rust commune aux applis egui de Desarrh : `explorateur`, `lecteur-universel`, `vigie` et `tube`. Version 0.1.6. Dépôt public : https://github.com/Desarrh/socle. Guide d'adoption par app : [MIGRATION.md](MIGRATION.md).

**Vocabulaire du document**
- **Tube** (majuscule) désigne l'app de lecture YouTube. Le canal Windows qui relie deux instances d'une même app est appelé **tube nommé** (*named pipe*).
- **Ultimate** est le lanceur de jeux de l'auteur, en Tauri, hors socle.
- **Palette** a deux sens : `theme::Palette`, les couleurs, et `ui::Palette`, la palette de commandes Ctrl+K.
- **Règle n° 0** : pendant les tests, l'utilisateur garde l'usage normal de son PC. Pas de fenêtre visible, pas de focus pris, pas d'entrée simulée, rien d'écrit dans ses apps ni dans ses réglages.
- **QoL** : pistes d'amélioration du quotidien. « T4 », « T5 » et « T6 » sont numérotées dans le plan de travail du projet (hors dépôt) : T4 = autocomplétion floue partout, T5 = palette Ctrl+K partout, T6 = presque tous les formats d'image. « Piste QoL n » renvoie à la liste du §9.

## 1. But et direction

### Problème

Le fichier de thème existait en 4 copies, et les icônes en 2 copies, qui commençaient à diverger. Chaque app réécrivait aussi sa recherche, sa palette, ses réglages, son journal de plantage, son instance unique et sa mise à jour.

### Objectifs

- Une seule implémentation de ce qui est commun : thème, icônes, raccourcis, recherche floue et autocomplétion (T4), palette Ctrl+K (T5), toasts et composants, décodage d'images (T6), réglages, `crash.log`, instance unique, mise à jour, mode test.
- **Compilé dans chaque exe** : aucune dépendance à l'exécution. Chaque app reste un exe unique qui marche seul, même si les autres sont absentes.
- **Rapide** : recherche en moins de 5 ms sur 100 000 entrées avec au moins 2 cœurs (mesuré, §6). Aucun sondage. Le travail lourd tourne sur des threads annulables.
- **Petit** : chaque app n'embarque que ce qu'elle active (features Cargo). Seul `theme` est actif par défaut, et `paths` est toujours compilé.
- Reprendre le code existant des apps plutôt que réinventer.

### Non-objectifs

- Ultimate (Tauri, interface web) : il garde sa propre direction artistique.
- Développer les photos RAW : on lit seulement l'aperçu JPEG intégré.
- Un service ou une DLL partagés entre apps.
- La fenêtre et le rendu (winit, softbuffer) : chaque app garde sa boucle, la démo en contient une copie.
- Simuler des entrées (souris, clavier) dans les tests.

## 2. Stack

| Domaine | Choix |
|---|---|
| Langage | Rust, édition 2024, testé avec rustc 1.99.1 (MSVC) |
| UI | `egui` 0.37 (comme les apps), **sans** `default_fonts` (polices de Windows), sans winit dans la bibliothèque |
| Recherche floue | maison + `memchr` 2.8 (memmem SIMD) ; distance d'édition bit-parallèle de Myers |
| Images | `image` 0.25.10 (png, jpeg, gif, webp, bmp, ico, tiff, tga, dds, hdr, qoi), `qoi` 0.4 (cache) ; PSD, DDS non compressé et aperçu RAW maison ; optionnels : `exr` via `image/exr`, `resvg` 0.50 (svgz, sans texte), `jxl-oxide` 0.12 ; WIC via `windows` 0.62 |
| Réglages, mise à jour | `serde` 1, `serde_json` 1, `zip` 9 (deflate), `sha2` 0.10 ; réseau par `%SystemRoot%\System32\curl.exe` (fourni avec Windows 10/11) |
| API Windows | `windows-sys` 0.61 (mutex, tube nommé, fenêtres de test), `windows` 0.62 (WIC) |
| Démo et mesures | `egui-winit` 0.37, `winit` 0.30, `softbuffer` 0.4, `hdrshot-egui-backend` 0.2 (dev-dependencies seulement) |

Les versions sont des contraintes semver de `Cargo.toml`. Le `Cargo.lock` de chaque app fixe la version exacte.

### Features

La colonne « Dépendances » ne liste que ce que la feature ajoute elle-même.

| Feature | Contenu | Dépendances |
|---|---|---|
| `theme` (défaut) | palettes, `apply`, polices, tailles, arrondis | egui |
| `icons` | 107 icônes vectorielles, `button`, `wide` | egui |
| `keys` | raccourcis compatibles AZERTY | egui |
| `fuzzy` | `fuzzy`, `autocomplete` (sans egui) | memchr |
| `ui` | composants, champ d'autocomplétion, palette Ctrl+K, aide F1 (active `theme`, `icons`, `keys`, `fuzzy`) | — |
| `images` | décodage, miniatures, cache disque, pool | image, qoi, memchr |
| `img-exr` / `img-svg` / `img-jxl` / `img-wic` | formats lourds, optionnels | exr / resvg / jxl-oxide / windows |
| `settings` | réglages JSON | serde, serde_json |
| `crash` | `crash.log` | — |
| `instance` | instance unique (Windows) | windows-sys |
| `update` | mise à jour par GitHub Releases | serde_json, zip, sha2 |
| `testmode` | mode test (Windows) : fenêtre invisible sans focus, capture, focus egui forcé, priorité basse | windows-sys, egui |
| `full` | tout (démo, tests) | — |

### Taille ajoutée par feature

Mesurée par `sh scripts/taille.sh` (le 20/11, v0.1.4 à v0.1.5 ; les features d'interface n'ont pas changé depuis). L'exe témoin imite une app : egui + `image` png/jpeg/gif/webp/bmp/ico = 4 602 Ko, avec le même profil release que les apps.

| Features | Ajout | Remarque |
|---|---:|---|
| `theme` | +30 Ko | |
| `icons` | +47 Ko | |
| `keys` | +6 Ko | |
| `fuzzy` | +90 Ko | |
| `ui` | +349 Ko | inclut theme, icons, keys, fuzzy |
| `images` | +1 061 Ko | tga, dds, hdr, qoi, tiff, psd, raw, cache, pool |
| `images` + `img-wic` | +1 075 Ko | WIC : +14 Ko |
| `images` + `img-exr` | +1 761 Ko | EXR : +700 Ko |
| `images` + `img-svg` | +2 176 Ko | SVG : +1 115 Ko |
| `images` + `img-jxl` | +3 317 Ko | JPEG XL : +2 256 Ko |
| `settings` | +76 Ko | |
| `crash` | +33 Ko | |
| `instance` | +33 Ko | |
| `update` | +408 Ko | zip et serde_json sont déjà dans l'explorateur et le lecteur |
| `testmode` | +5 Ko | |
| `full` | +5 919 Ko | |

Les polices intégrées d'egui (`default_fonts`) coûteraient ~630 Ko : le socle ne les active pas.

### RAM des polices

`theme::FontOptions`, mesurée par `examples/polices.rs` : mémoire privée après installation et rendu d'une page de texte.

| Polices | RAM | Par défaut |
|---|---:|---|
| Segoe UI + Consolas (toujours) | +1,7 Mo | oui |
| `symbols` : Segoe UI Symbol (flèches, ⋯, ●) | +2,4 Mo | oui |
| `bold` : Segoe UI Bold | +1,0 Mo | non : la famille `bold` est alors un alias du normal |
| `emoji` : Segoe UI Emoji | +12,1 Mo | non |

`install_fonts(ctx)` charge le défaut (+4,1 Mo). Tout charger coûte +17,1 Mo (mesuré, arrondi). La v0.1.2 chargeait tout. L'explorateur est alors passé de 29 à 43 Mo, d'où le choix par défaut de la v0.1.3.

### Commandes

- `cargo test --release --features full`
- `cargo bench --features full`
- `cargo run --release --example demo --features full [-- dossier]`
- `cargo run --release --example demo --features full -- --capture <dossier>` (captures PNG sans fenêtre)
- `cargo run --release --example testmode_check --features "theme testmode"`
- `sh scripts/taille.sh`

### Alternatives écartées

| Choix | Alternative | Pourquoi elle a été écartée |
|---|---|---|
| Crate compilée dans chaque exe | DLL ou service partagé | Contraire à « chaque app marche seule » : une DLL absente ou d'une autre version casserait une app |
| Dépendance git par tag | `path = "../socle"` ; crates.io ; copier le code | `path` casse la CI publique de l'explorateur et du lecteur. Pour crates.io, la publication est publique et définitive, lourde pour une crate interne. Une copie refait diverger le code |
| Une crate à features | Plusieurs petites crates (workspace) | Une seule version à suivre par app ; les features donnent déjà le « n'embarquer que ce qu'on utilise » |
| Recherche floue maison | `nucleo`, `fuzzy-matcher`, `skim` | Il fallait les fautes de frappe (distance d'édition), qu'aucune ne fait, plus les accents et les initiales, en restant sous 5 ms. Le code maison tient 0,6 à 2,6 ms pour 100 000 entrées (§6). Les crates n'ont pas été mesurées : justification par le besoin, pas par la vitesse |
| `curl.exe` de Windows | `ureq` + rustls, WinHTTP | Déjà utilisé par l'explorateur et le lecteur ; aucune pile TLS dans l'exe. `ureq` + rustls ajouterait plusieurs centaines de Ko (non mesuré) |
| Mise à jour maison | crate `self_update` | Elle embarque un client HTTP complet ; on reprend le code existant des apps |
| Empreinte SHA-256 de l'API GitHub | signature ed25519 | La signature protège même si le compte GitHub est compromis, mais elle demande une clé privée et une étape de signature à chaque release. Décision reportée par l'utilisateur (§8) |
| JPEG XL en Rust (`img-jxl`, optionnel) | WIC seulement | L'extension JPEG XL du Store n'est pas installée par défaut. La feature reste optionnelle à cause de son coût (+2,2 Mo) |
| AVIF / HEIC par WIC | décodeur Rust | Pas de décodeur AV1 / HEVC en Rust pur utilisable ; dav1d est en C |

## 3. Architecture

### Bibliothèque (`src/`)

| Fichier | Rôle |
|---|---|
| `src/lib.rs` | modules derrière leurs features, `Cancel` (jeton d'annulation partagé), `VERSION` |
| `src/theme.rs` | `Palette` (19 rôles de couleur), `palette(dark)`, `apply`, `install_fonts`, `install_fonts_with`, `FontOptions`, `font_definitions`, `blend`, `system_dark`, constantes `taille`, `arrondi`, `dim` |
| `src/icons.rs` | `enum Icon`, `paint`, `button`, `wide`, `Look`, `Icon::for_extension` |
| `src/keys.rs` | `effective`, `pressed`, `consume`, `consume_ctrl_digit` : touche physique pour la rangée des chiffres |
| `src/fuzzy.rs` | `fold` (accents, casse), `Index` (entrées repliées une fois), `search`, `score`, `kind`, `highlight` |
| `src/autocomplete.rs` | `History` (fréquence + récence, sérialisable), `Autocomplete` (`suggest`, `complete` pour Tab, `generation`) |
| `src/ui/mod.rs` | `trunc`, `paint_text_left`, `paint_text_right`, `ago`, `highlighted`, `section_header`, `Toasts`, `segmented`, `modal`, `search_field`, `autocomplete_field` |
| `src/ui/palette.rs` | `Command`, `Palette` (Ctrl+K, `open_with`, `query`), `help_window` (F1) |
| `src/images/mod.rs` | `detect`, `decode`, `thumbnail`, `fit`, `extensions`, `is_supported`, `MAX_FILE`, `MAX_PIXELS`, `DiskCache`, `thumbnail_cached`, `Pool`, `to_color_image` |
| `src/images/dds.rs` | DDS non compressés (masques de bits, DX10 8 bits par canal) ; BC1–BC3 passent par `image` |
| `src/images/psd.rs` | image composite PSD/PSB (RLE ou brut, 8/16/32 bits, RVB/gris/CMJN/indexé) |
| `src/images/raw.rs` | JPEG d'aperçu des RAW |
| `src/images/svg.rs` | resvg |
| `src/images/wic.rs` | WIC : décodage complet et réduit au décodage |
| `src/images/tests.rs` | tests des formats, du cache et du pool (fichiers générés dans `%TEMP%`) |
| `src/paths.rs` | `config_dir`, `cache_dir`, `write_atomic` (toujours compilé) |
| `src/settings.rs` | `load`, `save`, `load_from`, `save_to`, `Saver` (`new`, `debounced`, `touch`, `tick`, `flush`, `take_error`) |
| `src/crash.rs` | `install`, `pending`, `log_path` |
| `src/instance.rs` | `start` → `Start::{Primary, Forwarded, Alone}`, `Instance::listen` |
| `src/update.rs` | `Updater` (`check`, `parse`, `install`), `Release`, `due`, `is_after_update`, `cleanup_old`, `relaunch`, essai et retour arrière : `startup` → `Startup::{Normal, Trial, RolledBack}`, `confirm_started`, `rollback_on_panic` |
| `src/testmode.rs` | `enabled`, `hide`, `capture`, `foreground`, `prepare_input`, `force_focus`, `low_priority` |

### Outillage

| Fichier | Rôle |
|---|---|
| `examples/demo.rs` | démo complète (boucle winit et rendu CPU des apps, mode `--capture`) |
| `examples/testmode_check.rs` | vraie fenêtre winit + egui en mode test : compte les images dessinées, capture, vérifie le focus |
| `examples/polices.rs` | RAM de chaque police facultative |
| `examples/taille.rs`, `scripts/taille.sh` | taille ajoutée par feature |
| `benches/perf.rs` | benchmarks (échoue si l'objectif de 5 ms est dépassé) |

### Flux

Comme dans les apps, les composants **renvoient** ce qui s'est passé : l'id de la commande choisie pour la palette, `AutoResponse` (texte validé, complétion par Tab), `Modal { close }`, un `bool` pour le contrôle segmenté. L'app applique l'action ensuite. Pour les commandes dynamiques, l'app lit `Palette::query()` (le texte tapé) avant `show` et ajoute ses commandes à la liste.

### Threads

- **Recherche** : synchrone, sur le thread qui l'appelle. Au-delà de 40 000 entrées, elle se répartit sur les cœurs (au plus 8, `std::thread::scope`).
- **`images::Pool`** : N threads et une file LIFO (ce qui est à l'écran passe d'abord). Les demandes identiques sont dédoublonnées, et un seul décodage d'image complète tourne à la fois. `cancel_pending` vide la file et annule les décodages en cours. Le rappel `wake` est fourni par l'app (typiquement `ctx.request_repaint()`) et appelé depuis le thread de décodage à chaque résultat.
- **`instance`** : un thread attend le prochain client sur le tube nommé, sans consommer de CPU (attente asynchrone).
- **`settings::Saver`** : écrit sur un thread ponctuel ; `flush` attend la fin de l'écriture. En mode `debounced`, l'écriture a lieu après un silence (par exemple 300 ms), et au plus tard après un délai maximal (par exemple 2 s).

## 4. Direction artistique

Le socle porte la direction artistique commune aux 4 apps egui (document de direction du projet, hors dépôt). Les valeurs exactes sont dans le code (`theme.rs`, `ui/`) et visibles sur les captures de `docs/captures/`.

- **Couleurs** : 19 rôles, en clair « crème » et en sombre « bleu nuit », dont `bg`, `surface`, `side`, `soft`, `text`, `muted`, `accent`, `live`, `add` / `del` (ajouts / suppressions), `sel_bg` et `hover_bg`. La liste complète est dans `theme::Palette`.
- **Thème** : repris de l'explorateur, plus ce que le lecteur et Tube avaient ajouté (famille `bold`, police emoji). Ces polices sont facultatives depuis la v0.1.3. Les 4 copies étaient identiques, hors fins de ligne.
- **Ajouts** : `Palette::seg_on()` (bouton actif du contrôle segmenté) et `clear_u32()` (vidage du tampon de rendu). Constantes `taille`, `arrondi` et `dim`.
- **Icônes** : grille 24 × 24, trait 1,8 (au moins 1,2 px), couleur passée par l'app. Les icônes du lecteur gardent leur dessin. Celles de Tube aussi, agrandies pour avoir la même taille optique. Les nouvelles (fichiers, navigation, Vigie) suivent le même style : contour, coins arrondis, rares aplats.
- **Composants** : contrôle segmenté, modale sur fond assombri, palette Ctrl+K, liste d'autocomplétion (icône horloge pour l'historique), toasts dans la barre d'état (4 s, 8 s pour une erreur). La modale et les listes s'affichent sans fondu, dans la même image que l'action.

## 5. Conventions

- Commentaires et textes en français. Erreurs : `Result<_, String>` lisible, jamais d'`unwrap` sur les E/S ni le réseau.
- `unsafe` limité aux appels Win32 et COM : `instance.rs`, `testmode.rs`, `images/wic.rs`.
- Dossiers : réglages et `crash.log` dans `%APPDATA%\<App>\`, caches dans `%LOCALAPPDATA%\<App>\` (miniatures : `miniatures\*.qoi`).
- Instance unique : mutex `Local\<App>.Socle.Instance` + tube nommé `\\.\pipe\<App>-<utilisateur>-s<n° de session>-socle-v2`. Message : `SOCLE1\0arg1\0arg2…`.
- Mise à jour : asset zip d'une release GitHub de l'app (suffixe configurable, par exemple `windows-x64.zip`), contenant l'exe. Drapeau de relance `--apres-maj` (l'ancien `--after-update` est reconnu), et `--retour-maj` après un retour arrière.
- Fichiers posés à côté de l'exe pendant une mise à jour :
  - `<exe>.old` : l'ancienne version, gardée pendant l'essai ;
  - `<exe>.essai` : témoin d'essai, avec la version et le nombre de démarrages ;
  - `<exe>.echec` : version abandonnée ;
  - `<exe>.refusee` : version à ne plus proposer.

  Hors essai, `cleanup_old` supprime `.old` et `.echec`.
- Raccourcis fournis : Ctrl+K (palette), F1 (aide), Tab (compléter), ↑↓ / Entrée / Échap.
- **AZERTY** : egui-winit 0.37 transmet la touche logique (Ctrl+3 arrive comme `Key::Quote`). Il faut lire les raccourcis avec `socle::keys` : la rangée des chiffres passe par la touche physique.
- Versions : un tag Git par version. Tant que le socle est en 0.x, un changement d'API peut arriver à chaque version : il est listé dans MIGRATION.md, et chaque app choisit quand suivre. `socle::VERSION` donne la version compilée dans l'exe.
- Git : auteur `Desarrh` uniquement.
- **Tests (règle n° 0)** :
  - captures sans fenêtre (`--capture`) ;
  - tube nommé au nom de test ;
  - fichiers dans `%TEMP%` ;
  - le test de `testmode` affiche une fenêtre à opacité 0 et vérifie que le focus ne bouge pas.

## 6. Fonctionnalités (ce qui marche aujourd'hui)

Testé par `cargo test --release --features full` (43 tests) et `cargo bench --features full`.

Machine de mesure : Intel Core i9-9900K (8 cœurs, 16 threads), SSD NVMe, Windows 11. Les durées dépendent de la machine et de sa charge.

- **Recherche floue** : accents et casse ignorés, préfixe, sous-chaîne, initiales (`cev` → « Capture Écran Vidéos », `ula` → `UltimateLauncher`), lettres sautées, fautes de frappe (1 dès 5 lettres, 2 dès 9), plusieurs mots (`nouv dos`), bonus par entrée (fréquence, épinglé), surlignage.
  Sur 100 000 noms de 24 caractères en moyenne, le pire de 30 essais va de 0,6 à 2,6 ms selon la requête, et l'index se construit en 26 ms. Sur un seul cœur : ~3 ms pour 40 000 entrées.
- **Autocomplétion** : historique fréquence × récence (200 entrées au plus), candidats fournis par l'app, suggestions recalculées quand les candidats changent. Tab écrit la suggestion choisie, sinon le plus long préfixe commun, sinon la meilleure suggestion. 1 à 2 ms sur 100 000 candidats.
- **Palette Ctrl+K** : recherche floue sur le libellé puis sur « groupe + libellé », récentes en tête, commandes désactivées grisées, raccourcis affichés, ouverture avec une saisie pré-remplie (`open_with`). L'aide F1 est générée depuis les mêmes commandes.
- **Images** :
  - formats : PNG, JPEG, GIF, WebP, BMP, ICO, TIFF, TGA (compressé ou non), QOI, HDR, DDS (non compressé en Rust, BC1–3 par `image`, BC4–7 par WIC), PSD/PSB, aperçu RAW, EXR, SVG/SVGZ, JPEG XL, HEIC/AVIF/JXR (WIC) ;
  - format détecté sur les 512 premiers octets, puis par l'extension ;
  - repli sur WIC si le décodeur Rust échoue ;
  - tonification des images flottantes.

  Mesures sur 1 thread : JPEG 24 Mpx en 135 ms en entier, **38 ms en miniature 256** (réduit par WIC) ; PSD 4000×3000 en 143 ms, **13 ms en miniature** ; PNG 4K en 84 ms ; miniature lue dans le cache **0,2 ms** ; SVG en 256 px 0,2 ms.
- **Utilitaires** :
  - réglages : `.tmp` unique par écriture, `sync_all` avant le renommage, fichier abîmé gardé en `.bad`, enregistrement différé `Saver` avec `flush` à la fermeture et erreurs remontées (`take_error`) ;
  - `crash.log` et `pending()` au lancement suivant ; une panique survenue à l'intérieur du `catch_unwind` du `Pool` (le décodage lui-même) est journalisée sans être signalée comme plantage ; ailleurs, y compris dans le rappel `wake` de l'app, elle est signalée ;
  - instance unique : transmission en moins de 500 ms vérifiée en test ;
  - mise à jour : lecture de la release, remplacement de l'exe avec retour arrière s'il échoue (testé) ;
  - essai de la nouvelle version (testé) :
    - l'ancienne est gardée jusqu'à `confirm_started` ;
    - après 2 démarrages sans confirmation, ou une panique pendant l'essai, l'ancienne est remise et relancée ;
    - la version abandonnée n'est plus proposée ;
  - réglages au fil de l'eau (`Saver::debounced`).
- **Sécurité** (audit de la v0.1.4) :
  - **mise à jour** :
    - `curl.exe` appelé par son chemin complet, HTTPS obligatoire même après une redirection, `--` avant l'adresse ;
    - adresse limitée à `https://github.com/` ;
    - empreinte SHA-256 vérifiée contre le `digest` de l'API GitHub ;
    - exe extrait limité à 500 Mo, avec en-tête `MZ` ;
    - dossier temporaire nommé par le PID : la version, venue du réseau, n'entre dans aucun chemin ;
  - **instance** :
    - tube nommé créé avec `FILE_FLAG_FIRST_PIPE_INSTANCE` (refus si le nom est déjà pris) et `PIPE_REJECT_REMOTE_CLIENTS` ;
    - client en `SECURITY_IDENTIFICATION` ;
    - numéro de session dans le nom ;
    - lecture bornée à 1 s et 1 Mo ;
    - passage au premier plan autorisé pour la seule instance principale ;
  - **images** :
    - en-têtes PSD hostiles rejetés sans panique (test de 8 cas) ;
    - allocations bornées : PSD à 1 Gio (~150 Mpx en RVB), WIC et JXL à 268 Mpx, fichier à 1 Go ;
    - `catch_unwind` autour des décodeurs du `Pool`.

## 7. Intégrations

- **Apps qui utilisent le socle** : les quatre. La version suivie par chaque app est le `tag` de son `Cargo.toml`, qui fait foi.

  | App | Features |
  |---|---|
  | explorateur | ui, images, img-svg, img-wic, settings, crash, instance, update, testmode |
  | lecteur-universel | ui, images, img-exr, img-svg, img-wic, settings, crash, instance, update, testmode |
  | vigie | ui, settings, crash, instance, testmode |
  | tube | ui, settings, crash, instance, testmode |

  Ce qu'elles doivent changer pour suivre chaque version est dans MIGRATION.md.
- **Ultimate** : hors socle. Il appelle les apps par la ligne de commande ; `instance` fait arriver un `Tube.exe "recherche"` dans la fenêtre déjà ouverte.
- **Windows** : WIC pour les codecs du Store (HEIF, AV1, RAW, JPEG XL). Les messages d'erreur disent quelle extension installer.

## 8. Limites connues / dette technique

Chaque limite est suivie de ce qui la contient aujourd'hui.

- **PSD** : seulement l'image composite. Un PSD enregistré sans « Maximiser la compatibilité » n'en a pas, et la compression ZIP (rare) n'est pas lue. La transparence est « dématée » du blanc, ce qui est approximatif sur les bords. → Erreur lisible, puis repli sur WIC.
- **RAW** : aperçu JPEG intégré seulement, et l'orientation EXIF n'est appliquée à aucune image. → Piste QoL 3.
- **SVG** : `<text>` ignoré, pour éviter ~1 Mo et le chargement des polices système.
- **AVIF / HEIC / DDS BC4–BC7** : seulement par WIC, donc il faut les extensions du Store. → Le message d'erreur nomme l'extension à installer.
- **JPEG en miniature sans `img-wic`** : décodage complet (~135 ms pour 24 Mpx) puis réduction. → L'explorateur active `img-wic`.
- **Paniques des décodeurs** : `catch_unwind` ne protège que les apps compilées avec `panic = "unwind"`, le défaut de Rust. Une app en `panic = "abort"` (Vigie) qui activerait `images` s'arrêterait sur un fichier piégé. → Vigie n'active pas `images`.
- **Mémoire** :
  - une image complète peut occuper jusqu'à 1 Go (`MAX_PIXELS`) ;
  - le `Pool` n'en décode qu'une à la fois ; plusieurs demandes d'image complète en attente occupent chacune un thread qui attend son tour, ce qui ralentit les miniatures pendant ce temps (la file LIFO sert d'abord la dernière demande) ;
  - la file n'est pas bornée, mais elle est dédoublonnée et vidée par `cancel_pending`.
- **Cache disque** : il ne se vide pas tout seul. → L'app doit appeler `DiskCache::prune(octets)` sur un thread au démarrage (MIGRATION.md).
- **Instance unique** :
  - les instances suivantes sont servies une par une, et un client lent les retarde d'1 s au plus ;
  - si le tube nommé ne peut pas être recréé pendant 1 s, le thread d'écoute s'arrête et relâche le mutex : le lancement suivant devient l'instance principale (nouvelle fenêtre, sans attente).
- **Mise à jour** :
  - l'essai ne marche que si l'app appelle `startup`, `confirm_started` et `rollback_on_panic` (MIGRATION.md). Une version qui se fige sans planter n'est détectée qu'aux lancements suivants : il faut 2 démarrages sans confirmation. Un retour arrière vers une version d'avant la v0.1.6 laisse les fichiers `.echec` et `.refusee`, sans gêne ;
  - l'empreinte vient de la même API GitHub que le zip : elle garantit que le fichier est intact, pas qu'il vient bien de l'auteur. Un compte GitHub compromis pourrait publier un zip et son empreinte. → Signature ed25519 hors ligne à décider avec l'utilisateur ;
  - sans `exe_name`, le premier `.exe` du zip est pris.
- **Recherche** :
  - l'objectif de 5 ms suppose au moins 2 cœurs : sur un seul, une requête avec faute coûte ~7–8 ms pour 100 000 entrées ;
  - les initiales et les bonus de début de mot ne voient que les 64 premiers octets d'une entrée ;
  - les fautes de frappe ne sont tolérées que pour une requête de 64 octets au plus ;
  - la distance se compte en octets : une faute sur un caractère non latin compte double ou triple.
- **Icônes** : `Icon::Help` et `Back5/Fwd5/Back10/Fwd10` contiennent du texte (« ? », « 5 », « 10 ») : leur rendu dépend des polices du thème.
- **Tests d'interface** : il n'y en a pas ; les composants sont vérifiés par les captures de la démo. → Piste QoL 7.
- **Windows uniquement** : `instance`, `testmode`, `img-wic` et `update` (`curl.exe`, `CREATE_NO_WINDOW`). `paths` lit `%APPDATA%` et `%LOCALAPPDATA%`. Le reste est portable.

## 9. Pistes QoL (non implémentées, à valider)

[Inédit] = n'existe nulle part à notre connaissance ; [Existe ailleurs] = classique dans d'autres logiciels. S / M = effort estimé (un jour au plus / quelques jours). Chaque piste dit seulement le besoin : la technique sera choisie après validation.

1. **[Existe ailleurs] Index de recherche modifiable en place** (S) : retirer ou changer une entrée sans tout reconstruire, pour les listes qui changent en direct (fichiers de l'explorateur, processus de Vigie). Seulement si une app mesure une reconstruction trop lente (26 ms pour 100 000 entrées aujourd'hui).
2. **[Inédit] Actions des autres apps dans la palette** (M) : « Ouvrir dans le lecteur », « Chercher dans Tube ». Chaque app doit continuer à marcher seule : action masquée si l'autre app est absente. À trancher avant validation : comment une app trouve les autres sans dépendre de leur emplacement d'installation.
3. **[Existe ailleurs] Orientation EXIF** (S) : appliquer la rotation des photos (JPEG, RAW, HEIC).
4. **[Existe ailleurs] Thème qui suit Windows en direct** (S) : basculer quand Windows change de thème, sans sondage.
5. **[Existe ailleurs] Miniatures animées GIF/WebP** (M), au survol seulement.
6. **[Inédit] Raccourcis appris** (S) : la palette propose le raccourci d'une commande lancée plusieurs fois à la souris.
7. **[Existe ailleurs] Tests d'image de référence** (M) : comparer les captures `--capture` à des PNG de référence dans `cargo test`, avec une tolérance.
8. **[Existe ailleurs] Recherche incrémentale** (S) : quand la saisie prolonge la précédente, ne chercher que dans les résultats précédents. Seulement si l'objectif de 5 ms est dépassé sur une machine réelle (cas d'un seul cœur).
