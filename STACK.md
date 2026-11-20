# Socle — stack technique (document de référence)

Bibliothèque commune aux applis egui (`explorateur`, `lecteur-universel`, `vigie`, `tube`). Version 0.1.4. Dépôt public : https://github.com/Desarrh/socle. Pas encore utilisée par les apps : voir [MIGRATION.md](MIGRATION.md).

## 1. But et direction

- **À quoi ça sert** : arrêter les 4 copies de `theme.rs` et les 2 copies d'`icons.rs`, et écrire une seule fois ce que toutes les apps veulent : palette Ctrl+K, autocomplétion floue (QoL T4), toasts, décodage d'images étendu (QoL T6), réglages, `crash.log`, instance unique, mise à jour.
- **Ce n'est pas une dépendance à l'exécution.** Le socle est compilé dans chaque exe : chaque app reste un exe unique et marche seule (DIRECTION.md §2, règle 4).
- **Principes** :
  - extrêmement rapide : recherche < 5 ms sur 100 000 entrées (mesuré), aucun sondage, travail lourd sur des threads annulables ;
  - chaque app n'embarque que ce qu'elle utilise (features Cargo, rien par défaut sauf `theme`) ;
  - reprendre le meilleur code existant plutôt que réinventer.

## 2. Stack

| Domaine | Choix |
|---|---|
| Langage | Rust, édition 2024, testé avec rustc 1.99.1 (MSVC) |
| UI | `egui` 0.37.0 (comme les apps), sans dépendance à winit dans la bibliothèque |
| Recherche floue | maison + `memchr` 2.8.3 (memmem SIMD) ; distance d'édition bit-parallèle de Myers |
| Images | `image` 0.25.10 (png, jpeg, gif, webp, bmp, ico, tiff, tga, dds, hdr, qoi), `qoi` 0.4.1 (cache) ; PSD et aperçu RAW maison ; optionnels : `exr` via `image/exr`, `resvg` 0.50 (svgz, sans texte), `jxl-oxide` 0.12.6 ; WIC via `windows` 0.62 |
| Réglages, mise à jour | `serde` 1, `serde_json` 1, `zip` 9 (deflate) ; réseau par `curl.exe` (fourni avec Windows) |
| API Windows | `windows-sys` 0.61.2 (mutex, tube nommé), `windows` 0.62 (WIC) |
| Démo | `egui-winit` 0.37.0, `winit` 0.30.13, `softbuffer` 0.4.8, `hdrshot-egui-backend` 0.2.0 (dev-dependencies seulement) |

**Features** :

| Feature | Contenu | Dépendances ajoutées |
|---|---|---|
| `theme` (défaut) | palettes, `apply`, polices, tailles, arrondis | egui |
| `icons` | 107 icônes vectorielles, `button`, `wide` | egui |
| `keys` | raccourcis compatibles AZERTY (Ctrl+chiffre par la touche physique) | egui |
| `fuzzy` | `fuzzy`, `autocomplete` (sans egui) | memchr |
| `ui` | composants + palette Ctrl+K + aide F1 (= theme + icons + keys + fuzzy) | — |
| `images` | décodage, miniatures, cache disque, pool | image, qoi, memchr |
| `img-exr` / `img-svg` / `img-jxl` / `img-wic` | formats lourds, optionnels | exr / resvg / jxl-oxide / windows |
| `settings` | réglages JSON | serde, serde_json |
| `crash` | `crash.log` | — |
| `instance` | instance unique | windows-sys |
| `update` | GitHub Releases | serde_json, zip |
| `testmode` | mode test (règle n° 0) : fenêtre invisible à l'écran (opacité 0) sans focus, capture PrintWindow, focus egui forcé, priorité basse | windows-sys, egui |
| `full` | tout (démo, tests) | |

**Taille ajoutée par feature** (mesurée par `sh scripts/taille.sh` : exe témoin qui imite une app, egui + `image` png/jpeg/gif/webp/bmp/ico = 4 602 Ko, même profil release que les apps) :

| Features | Ajout |
|---|---:|
| `theme` | +30 Ko |
| `icons` | +47 Ko |
| `keys` | +6 Ko |
| `fuzzy` | +90 Ko |
| `ui` (inclut theme, icons, keys, fuzzy) | +349 Ko |
| `images` (tga, dds, hdr, qoi, tiff, psd, raw, cache, pool) | +1 061 Ko |
| `images` + `img-wic` | +1 075 Ko (WIC : +14 Ko) |
| `images` + `img-exr` | +1 761 Ko (EXR : +700 Ko) |
| `images` + `img-svg` | +2 176 Ko (SVG : +1 115 Ko) |
| `images` + `img-jxl` | +3 317 Ko (JPEG XL : +2 256 Ko) |
| `settings` | +76 Ko |
| `crash` | +33 Ko |
| `instance` | +33 Ko |
| `update` | +393 Ko (zip + serde_json : ~0 pour l'explorateur et le lecteur, qui les ont déjà) |
| `full` | +5 845 Ko |

`egui` est pris **sans** `default_fonts` : les polices viennent de Windows, comme dans les apps (les polices d'egui coûteraient ~630 Ko).

**RAM des polices** (`theme::FontOptions`, mesurée par `examples/polices.rs` : mémoire privée après installation et rendu d'une page de texte) :

| Polices | RAM | Défaut |
|---|---:|---|
| Segoe UI + Consolas (toujours) | +1,7 Mo | oui |
| `symbols` : Segoe UI Symbol (flèches, ⋯, ●) | +2,4 Mo | oui (ce que chargeait l'explorateur) |
| `bold` : Segoe UI Bold (vrai gras de la famille `bold`) | +1,0 Mo | non : `bold` est alors un alias du normal |
| `emoji` : Segoe UI Emoji | **+12,1 Mo** | non |

`install_fonts(ctx)` = défaut (+4,1 Mo). La v0.1.2 chargeait tout (+17,1 Mo), soit 13 Mo de trop pour des apps passées aux icônes vectorielles (constaté par l'explorateur : 29 → 43 Mo).

**Commandes** : `cargo test --release --features full` · `cargo bench --features full` · `cargo run --release --example demo --features full [-- dossier]` · `cargo run --release --example demo --features full -- --capture <dossier>` (captures PNG hors écran) · `sh scripts/taille.sh` (taille par feature).

## 3. Architecture

| Fichier | Rôle |
|---|---|
| `src/lib.rs` | modules derrière leurs features, `Cancel` (jeton d'annulation partagé) |
| `src/theme.rs` | `Palette` (19 rôles), `palette(dark)`, `apply`, `install_fonts`, `blend`, `system_dark`, constantes `taille`, `arrondi`, `dim` |
| `src/icons.rs` | `enum Icon`, `paint`, `button`, `wide`, `Icon::for_extension` |
| `src/keys.rs` | `effective`, `pressed`, `consume`, `consume_ctrl_digit` : touche physique pour la rangée des chiffres (constat de la session Tube) |
| `src/fuzzy.rs` | `fold` (accents, casse), `Index` (entrées repliées une fois), `search`, `score`, `kind`, `highlight` |
| `src/autocomplete.rs` | `History` (fréquence + récence, sérialisable), `Autocomplete` (`suggest`, `complete` pour Tab) |
| `src/ui/mod.rs` | `trunc`, `paint_text_left/right`, `ago`, `highlighted`, `section_header`, `Toasts`, `segmented`, `modal`, `search_field`, `autocomplete_field` |
| `src/ui/palette.rs` | `Command`, `Palette` (Ctrl+K), `help_window` (F1) |
| `src/images/mod.rs` | `detect`, `decode`, `thumbnail`, `fit`, `DiskCache`, `thumbnail_cached`, `Pool` |
| `src/images/dds.rs` | DDS non compressés (masques de bits, DX10 8 bits par canal) |
| `src/images/psd.rs` | image composite PSD/PSB (RLE ou brut, 8/16/32 bits, RVB/gris/CMJN/indexé) |
| `src/images/raw.rs` | JPEG d'aperçu des RAW |
| `src/images/svg.rs`, `wic.rs` | resvg ; WIC (complet + réduit au décodage) |
| `src/testmode.rs` | `enabled`, `hide`, `capture`, `foreground`, `prepare_input` / `force_focus`, `low_priority` |
| `examples/testmode_check.rs` | vraie fenêtre winit + egui en mode test : compte les images dessinées, capture, vérifie le focus |
| `src/paths.rs` | `config_dir`, `cache_dir`, `write_atomic` |
| `src/settings.rs`, `crash.rs`, `instance.rs`, `update.rs` | utilitaires |
| `examples/demo.rs` | démo complète (boucle winit + rendu CPU des apps, mode `--capture`) |
| `examples/polices.rs` | RAM de chaque police facultative |
| `examples/taille.rs`, `scripts/taille.sh` | mesure de la taille ajoutée par feature |
| `benches/perf.rs` | benchmarks (échoue si l'objectif de 5 ms est dépassé) |

**Flux** : comme dans les apps, les composants **renvoient** ce qui s'est passé (`Option<String>` pour la palette, `AutoResponse`, `Modal { close }`, `bool` pour le segmenté) ; l'app applique l'action ensuite.

**Threads** : la recherche se répartit sur les cœurs (au plus 8, `std::thread::scope`) au-delà de 40 000 entrées. `images::Pool` : N threads, file LIFO (ce qui est à l'écran passe d'abord), génération + `Cancel` pour tout abandonner, `wake` appelé à chaque résultat. `instance` : un thread qui dort dans `ConnectNamedPipe`. `settings::Saver` écrit sur un thread ponctuel.

## 4. Direction artistique

Le socle **porte** la DA commune (DIRECTION.md §3) : la table des couleurs est celle d'`explorateur/STACK.md` §4, inchangée.

- **Thème** : copie de `explorateur/src/theme.rs` plus ce que le lecteur et Tube avaient ajouté (famille `bold` = Segoe UI Bold, police emoji en secours), devenu facultatif en 0.1.3 (`install_fonts_with(ctx, FontOptions { bold, emoji, symbols })`). Diff fait sur les 4 copies : rien d'autre ne différait (Vigie et Tube ne différaient que par les fins de ligne CRLF et `#![allow(dead_code)]`).
- **Ajouts** : `Palette::seg_on()` (bouton actif du segmenté, `#2B4470` en sombre, en dur avant dans l'explorateur), `clear_u32()` (vidage du tampon softbuffer), constantes `taille` (15 / 13,5 / 12,5 / 12 / 11), `arrondi` (6 / 8 / 10 / 12) et `dim` (ligne 28, bouton 32).
- **Icônes** : grille 24 × 24, trait 1,8 (au moins 1,2 px), couleur passée par l'app. Les icônes du lecteur gardent leur dessin. Celles de Tube aussi, converties de leur repère centré (1 unité = 8,8 de grille, contre 7,44 avant, pour avoir la même taille optique). Les nouvelles (fichiers, navigation, Vigie) suivent le même style : contour, coins arrondis, rares aplats. Captures dans la démo (`--capture`).
- **Composants** : contrôle segmenté (fond `soft`, arrondi 9, marge 3, bouton actif arrondi 7), modale (fond noir à 35 %, carte `surface` arrondie 12, ombre 12/40, sans fondu pour s'afficher dans la même image), palette (largeur 640, à 90 px du haut, saisie en 17, lignes de 40, raccourcis en « touches » `soft`), liste d'autocomplétion (lignes de 28, icône horloge pour l'historique), toasts dans la barre d'état (12 px, icône info / coche / alerte, `del` pour les erreurs, fondu sur 400 ms, 4 s ou 8 s pour une erreur).

## 5. Conventions

- Commentaires et textes en français. Erreurs : `Result<_, String>` lisible, jamais d'`unwrap` sur les E/S ni le réseau.
- `unsafe` limité à `instance.rs` (Win32) et `images/wic.rs` (COM).
- Dossiers : réglages et `crash.log` dans `%APPDATA%\<App>\`, caches dans `%LOCALAPPDATA%\<App>\` (miniatures : `miniatures\*.qoi`).
- Instance unique : mutex `Local\<App>.Socle.Instance` + tube `\\.\pipe\<App>-<utilisateur>-socle-v1` ; message `SOCLE1\0arg1\0arg2…`.
- Mise à jour : drapeau de relance unifié `--apres-maj` (l'ancien `--after-update` est reconnu), ancien exe `.exe.old` (et `.old`) supprimé au démarrage.
- Raccourcis fournis : Ctrl+K (palette), F1 (aide), Tab (compléter), ↑↓ / Entrée / Échap.
- **AZERTY** : egui-winit 0.37 transmet la touche logique (Ctrl+3 arrive comme `Key::Quote`). Lire les raccourcis avec `socle::keys` (`consume`, `pressed`, `consume_ctrl_digit`) : la rangée des chiffres passe par la touche physique, et Ctrl+chiffre redevient utilisable.
- Git : auteur `Desarrh` uniquement. Dépôt public https://github.com/Desarrh/socle, versions par tag (`v0.1.0` → `v0.1.4`).
- **Tests (règle n° 0)** : jamais de fenêtre visible, de focus pris, d'entrée simulée ni d'écriture dans les apps ou réglages de l'utilisateur. Captures hors écran (`--capture`), tube d'instance au nom de test, fichiers dans `%TEMP%`. Le test de `testmode` affiche une fenêtre à opacité 0, sans activation, et vérifie que le focus ne bouge pas. `examples/testmode_check.rs` montre qu'une fenêtre hors écran ne marche pas (capture blanche) et qu'une fenêtre invisible à l'écran marche.

## 6. Fonctionnalités (ce qui marche aujourd'hui)

Testé par `cargo test --release --features full` (40 tests) et `cargo bench --features full` :

- **Recherche floue** : accents et casse ignorés, préfixe, sous-chaîne, initiales (`cev` → « Capture Écran Vidéos », `ula` → `UltimateLauncher`), lettres sautées, fautes de frappe (1 dès 5 lettres, 2 dès 9), plusieurs mots (`nouv dos`), bonus par entrée (fréquence, épinglé), surlignage des lettres trouvées.
  Mesures sur 100 000 noms (24 caractères en moyenne), max sur 30 essais : de 0,6 à 2,1 ms selon la requête ; index construit en 26 ms.
- **Autocomplétion** : historique fréquence × récence (borné à 200), candidats fournis par l'app, Tab = suggestion choisie, sinon plus long préfixe commun, sinon meilleure suggestion. 1 à 2 ms sur 100 000 candidats.
- **Palette Ctrl+K** : recherche floue sur le libellé puis sur « groupe + libellé », récentes en tête, commandes désactivées grisées, raccourcis affichés, `query()` pour les commandes dynamiques. Aide F1 générée depuis les mêmes commandes.
- **Images** : PNG, JPEG, GIF, WebP, BMP, ICO, TIFF, TGA (compressé ou non), QOI, HDR, DDS (non compressé et BC1–3 en Rust, BC4–7 par WIC), PSD/PSB, aperçu RAW, EXR, SVG/SVGZ, JPEG XL, HEIC/AVIF/JXR (WIC). Détection par signature puis extension, repli WIC si le décodeur Rust échoue, tonification des images flottantes.
  Mesures (1 thread) : JPEG 24 Mpx 135 ms en entier, **38 ms en miniature 256** (réduit par WIC) ; PSD 4000×3000 143 ms en entier, **13 ms en miniature** ; PNG 4K 84 ms ; miniature lue dans le cache disque **0,2 ms** ; SVG en 256 px 0,2 ms.
- **Utilitaires** : réglages (`.tmp` unique par écriture, fichier abîmé gardé en `.bad`, enregistrement différé `Saver` avec `flush` à la fermeture), `crash.log` + `pending()` au lancement suivant, instance unique (transmission vérifiée < 500 ms en test, < 1 ms en pratique), mise à jour (lecture de la release, remplacement avec retour arrière testé).
- **Sécurité** (audit de la v0.1.4) :
  - **mise à jour** : `curl.exe` appelé par son chemin complet (`%SystemRoot%\System32`), HTTPS obligatoire même après une redirection, `--` avant l'adresse, adresse limitée à `https://github.com/`, empreinte SHA-256 vérifiée contre le `digest` de l'API GitHub, exe extrait limité à 500 Mo avec en-tête `MZ`, dossier temporaire nommé par le PID. Pas encore de signature ed25519 : il faudrait une clé privée en CI ;
  - **instance** : tube créé avec `FILE_FLAG_FIRST_PIPE_INSTANCE` (refus si le nom est déjà pris) et `PIPE_REJECT_REMOTE_CLIENTS`, client en `SECURITY_IDENTIFICATION`, numéro de session dans le nom, lecture bornée à 1 s et 1 Mo ;
  - **images** : en-têtes PSD hostiles rejetés sans panique (test de 8 cas), allocations bornées (PSD ~256 Mpx, WIC et JXL 268 Mpx, fichier 1 Go), format détecté sur 512 octets, `catch_unwind` autour des décodeurs du `Pool`.

## 7. Intégrations

- **Explorateur, lecteur, Vigie, Tube** : consommateurs prévus (MIGRATION.md). Aucun n'est migré.
- **Ultimate** (Tauri) : hors socle (autre DA, autre langage d'UI). Il peut appeler les apps par la ligne de commande ; `instance` garantit qu'un `Tube.exe "recherche"` arrive dans la fenêtre ouverte.
- **Windows** : WIC pour les codecs du Store (HEIF, AV1, RAW, JPEG XL) ; les messages d'erreur disent quelle extension installer.

## 8. Limites connues / dette technique

- **PSD** : seulement l'image composite. Un PSD enregistré sans « Maximiser la compatibilité » n'en a pas (l'image est vide ou blanche) ; la compression ZIP (rare) n'est pas lue. La transparence est « dématée » du blanc, ce qui est approximatif sur les bords.
- **RAW** : aperçu JPEG intégré uniquement (pas de développement) ; l'orientation EXIF n'est pas appliquée (ni pour les JPEG en général).
- **SVG** : sans le texte (`<text>` ignoré) pour éviter ~1 Mo et le chargement des polices système.
- **AVIF / HEIC** : seulement par WIC (il faut les extensions du Store). Pas de décodeur Rust pur acceptable (dav1d est en C).
- **DDS BC4–BC7** : seulement par WIC.
- **JPEG en miniature sans `img-wic`** : décodage complet (~135 ms pour 24 Mpx) puis réduction.
- **Recherche** : le parallélisme démarre à 40 000 entrées. Sur un seul cœur, une requête avec faute coûte ~3 ms pour 40 000 entrées, donc ~7–8 ms pour 100 000 : l'objectif de 5 ms suppose au moins 2 cœurs (toujours le cas sur les PC visés).
- **Icônes** : `Icon::Help` et `Back5/Fwd5/Back10/Fwd10` utilisent du texte (« ? », « 5 », « 10 ») : il faut les polices du thème pour un rendu identique.
- Pas de tests d'interface : composants vérifiés par les captures de la démo (`--capture`).
- Windows uniquement pour `instance` et `img-wic` ; le reste est portable.

## 9. Pistes QoL (non implémentées, à valider)

1. **[Existe ailleurs] Index de recherche incrémental** (S) : `Index::remove` / mise à jour en place, pour les listes qui changent en direct (flux `notify` de l'explorateur, processus de Vigie). ⚡ Tampon avec trous + compactage différé.
2. **[Inédit] Palette partagée entre apps** (M) : Ctrl+K liste aussi les actions des autres apps installées (« Ouvrir dans le lecteur », « Chercher dans Tube »), lancées par ligne de commande. ⚡ Liste statique dans chaque exe, détection de présence de l'exe au premier affichage seulement.
3. **[Existe ailleurs] Orientation EXIF** (S) : appliquer la rotation des photos (JPEG, RAW, HEIC). ⚡ Lecture de quelques octets de l'en-tête.
4. **[Existe ailleurs] Thème qui suit Windows en direct** (S) : basculer quand Windows change de thème. ⚡ Événement `ThemeChanged` de winit, aucun sondage.
5. **[Existe ailleurs] Miniatures animées GIF/WebP** (M). ⚡ Seulement au survol, images décodées à la volée.
6. **[Inédit] Raccourcis appris** (S) : la palette propose le raccourci d'une commande lancée 3 fois à la souris. ⚡ Compteur par id dans `Palette::recent`.
