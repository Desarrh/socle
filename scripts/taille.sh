#!/bin/sh
# Mesure la taille ajoutée par chaque feature du socle (exe témoin examples/taille.rs, profil
# release des apps : lto thin, codegen-units 4, strip). Sortie : tableau Markdown.
# Usage (Git Bash) : sh scripts/taille.sh
cd "$(dirname "$0")/.."
base=0
echo "| Features | Exe | Ajout |"
echo "|---|---:|---:|"
while IFS='|' read -r nom feat; do
  if [ -n "$feat" ]; then f="--features $feat"; else f=""; fi
  if cargo build --release --example taille --no-default-features --target-dir target/taille $f </dev/null >/dev/null 2>&1; then
    len=$(stat -c %s target/taille/release/examples/taille.exe)
    [ "$base" -eq 0 ] && base=$len
    echo "| $nom | $((len / 1024)) Ko | +$(((len - base) / 1024)) Ko |"
  else
    echo "| $nom | échec de compilation | |"
  fi
done <<LISTE
base (egui + image png/jpeg/gif/webp/bmp/ico)|
theme|theme
icons|icons
keys|keys
fuzzy (recherche floue + autocomplétion)|fuzzy
ui (composants + palette, inclut theme, icons, fuzzy)|ui
images (tga, dds, hdr, qoi, tiff, psd, raw, cache, pool)|images
images + img-exr|images,img-exr
images + img-svg|images,img-svg
images + img-jxl|images,img-jxl
images + img-wic|images,img-wic
settings|settings
crash|crash
instance|instance
update|update
full (tout)|full
LISTE
