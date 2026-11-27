#!/usr/bin/env bash
# SPDX-License-Identifier: GPL-3.0-or-later
# Copyright (C) 2026 Valentin PICARD
#
# Construit un paquet Debian/Ubuntu/Linux Mint (.deb) pour FastIpSwitch.
#
# Utilisation :
#   packaging/build-deb.sh
#   SKIP_BUILD=1 packaging/build-deb.sh      # réutilise target/release
#   VERSION=0.2.0 ARCH=arm64 packaging/build-deb.sh
#
# Variables d'environnement :
#   SKIP_BUILD=1            ne lance pas cargo (les binaires doivent exister)
#   VERSION=<x.y.z>         force la version (défaut : Cargo.toml)
#   ARCH=<arch>             force l'architecture (défaut : dpkg --print-architecture)
#   DIST_DIR=<dir>          répertoire de sortie du .deb
#   ALLOW_MISSING_POLKIT=1  accepte de construire sans la policy polkit
#   POLKIT_POLICY=<file>    policy polkit à installer (défaut : packaging/polkit/)
#   SKIP_VALIDATE=1         n'appelle pas desktop-file-validate / appstreamcli
#   CARGO_TARGET_DIR=<dir>  respecté, comme pour cargo
#
set -euo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
PKG_DIR="$ROOT/packaging"
BUILD_DIR="$PKG_DIR/.build"
STAGE_DIR="$BUILD_DIR/stage"
DIST_DIR="${DIST_DIR:-$BUILD_DIR/dist}"

PKG_NAME="fastipswitch"
MAINTAINER="Valentin PICARD <developpeur@talevinn.fr>"
HOMEPAGE="https://codeberg.org/Talevinn/FastIpSwitch"

DESKTOP_ID="org.talevinn.fastipswitch"
POLICY_NAME="$DESKTOP_ID.policy"
ICON_SIZES=(16 24 32 48 64 128 256 512)

die() {
    printf 'erreur : %s\n' "$*" >&2
    exit 1
}

warn() {
    printf 'attention : %s\n' "$*" >&2
}

usage() {
    awk 'NR > 1 { if ($0 == "set -euo pipefail") exit; sub(/^# ?/, ""); print }' \
        "${BASH_SOURCE[0]}"
}

case "${1:-}" in
    -h | --help)
        usage
        exit 0
        ;;
    "") ;;
    *)
        die "argument inconnu : $1 (voir --help)"
        ;;
esac

command -v dpkg-deb >/dev/null 2>&1 || die "dpkg-deb est requis (paquet dpkg-dev)"

# Un umask restrictif produirait des répertoires en 0700 dans le paquet.
umask 022

# --- Version ---------------------------------------------------------------
VERSION="${VERSION:-$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$ROOT/Cargo.toml" | head -n 1)}"
[ -n "$VERSION" ] || die "impossible de lire la version depuis $ROOT/Cargo.toml"
case "$VERSION" in
    [0-9]*) ;;
    *) die "version Debian invalide : « $VERSION »" ;;
esac
case "$VERSION" in
    *[!0-9A-Za-z.+~:-]*) die "version Debian invalide : « $VERSION »" ;;
esac

# --- Architecture ----------------------------------------------------------
ARCH="${ARCH:-$(dpkg --print-architecture)}"
[ -n "$ARCH" ] || die "architecture introuvable (utiliser ARCH=...)"

# --- Binaires --------------------------------------------------------------
if [ "${CARGO_TARGET_DIR:-}" = "" ]; then
    CARGO_TARGET_DIR="$ROOT/target"
fi
case "$CARGO_TARGET_DIR" in
    /*) TARGET_ROOT="$CARGO_TARGET_DIR" ;;
    *) TARGET_ROOT="$ROOT/$CARGO_TARGET_DIR" ;;
esac
BIN_DIR="$TARGET_ROOT/release"

if [ "${SKIP_BUILD:-0}" = "1" ]; then
    warn "SKIP_BUILD=1 : binaires attendus dans $BIN_DIR"
else
    printf '==> cargo build --release (%s)\n' "$VERSION"
    cargo build \
        --manifest-path "$ROOT/Cargo.toml" \
        --release \
        --bin fastipswitch \
        --bin fastipswitch-helper
fi

for bin in fastipswitch fastipswitch-helper; do
    [ -x "$BIN_DIR/$bin" ] || die "binaire manquant ou non exécutable : $BIN_DIR/$bin"
done

# --- Validation des fichiers sources ---------------------------------------
DESKTOP_SRC="$PKG_DIR/deb/$DESKTOP_ID.desktop"
METAINFO_SRC="$PKG_DIR/appstream/$DESKTOP_ID.metainfo.xml"
POLICY_SRC="${POLKIT_POLICY:-$PKG_DIR/polkit/$POLICY_NAME}"

[ -f "$DESKTOP_SRC" ] || die "fichier desktop manquant : $DESKTOP_SRC"
[ -f "$METAINFO_SRC" ] || die "métadonnées AppStream manquantes : $METAINFO_SRC"
[ -f "$PKG_DIR/deb/DEBIAN/control" ] || die "modèle control manquant"
[ -f "$PKG_DIR/deb/copyright" ] || die "fichier copyright manquant"
[ -f "$PKG_DIR/deb/changelog" ] || die "changelog manquant"

for size in "${ICON_SIZES[@]}"; do
    [ -f "$ROOT/assets/logo/ip-swap-$size.png" ] ||
        die "icône manquante : assets/logo/ip-swap-$size.png"
done

if [ "${SKIP_VALIDATE:-0}" != "1" ]; then
    if command -v desktop-file-validate >/dev/null 2>&1; then
        desktop-file-validate "$DESKTOP_SRC" ||
            die "fichier .desktop invalide (desktop-file-validate)"
    else
        warn "desktop-file-validate absent : validation .desktop ignorée"
    fi
    if command -v appstreamcli >/dev/null 2>&1; then
        appstreamcli validate --no-net "$METAINFO_SRC" >/dev/null ||
            die "métadonnées AppStream invalides (appstreamcli validate --no-net)"
    else
        warn "appstreamcli absent : validation AppStream ignorée"
    fi
fi

# --- Staging ---------------------------------------------------------------
# Garde-fou : le staging doit rester dans packaging/.build/.
case "$STAGE_DIR" in
    "$PKG_DIR"/.build/*) ;;
    *) die "répertoire de staging inattendu : $STAGE_DIR" ;;
esac
printf '==> préparation de %s\n' "$STAGE_DIR"
rm -rf "$STAGE_DIR"
install -d -m 0755 \
    "$STAGE_DIR/DEBIAN" \
    "$STAGE_DIR/usr/bin" \
    "$STAGE_DIR/usr/libexec/fastipswitch" \
    "$STAGE_DIR/usr/share/doc/$PKG_NAME" \
    "$STAGE_DIR/usr/share/man/man1" \
    "$STAGE_DIR/usr/share/applications" \
    "$STAGE_DIR/usr/share/metainfo"

# Binaire GUI et helper privilégié.
install -m 0755 "$BIN_DIR/fastipswitch" "$STAGE_DIR/usr/bin/fastipswitch"
install -m 0755 "$BIN_DIR/fastipswitch-helper" \
    "$STAGE_DIR/usr/libexec/fastipswitch/fastipswitch-helper"

# Entrée de menu et icônes hicolor.
install -m 0644 "$DESKTOP_SRC" "$STAGE_DIR/usr/share/applications/$DESKTOP_ID.desktop"
for size in "${ICON_SIZES[@]}"; do
    install -D -m 0644 "$ROOT/assets/logo/ip-swap-$size.png" \
        "$STAGE_DIR/usr/share/icons/hicolor/${size}x${size}/apps/fastipswitch.png"
done

# Métadonnées AppStream.
install -m 0644 "$METAINFO_SRC" \
    "$STAGE_DIR/usr/share/metainfo/$DESKTOP_ID.metainfo.xml"

# Page de manuel (facultative).
MAN_SRC="$PKG_DIR/man/fastipswitch.1"
if [ -f "$MAN_SRC" ]; then
    gzip -9n -c "$MAN_SRC" >"$STAGE_DIR/usr/share/man/man1/fastipswitch.1.gz"
    chmod 0644 "$STAGE_DIR/usr/share/man/man1/fastipswitch.1.gz"
else
    warn "page de manuel absente : $MAN_SRC"
fi

# Policy polkit : fournie par packaging/polkit/, indispensable à pkexec.
if [ -f "$POLICY_SRC" ]; then
    # Une policy erronée (mauvais chemin, autorisation trop large) est pire
    # qu'une policy absente : on la valide avant de l'embarquer.
    if command -v xmllint >/dev/null 2>&1; then
        xmllint --noout "$POLICY_SRC" || die "policy polkit XML invalide : $POLICY_SRC"
    fi
    grep -q '/usr/libexec/fastipswitch/fastipswitch-helper' "$POLICY_SRC" ||
        die "la policy polkit ne cible pas /usr/libexec/fastipswitch/fastipswitch-helper"
    install -D -m 0644 "$POLICY_SRC" \
        "$STAGE_DIR/usr/share/polkit-1/actions/$POLICY_NAME"
else
    if [ "${ALLOW_MISSING_POLKIT:-0}" = "1" ]; then
        warn "policy polkit absente ($POLICY_SRC) : l'élévation pkexec ne fonctionnera pas"
    else
        die "policy polkit absente : $POLICY_SRC
       (définir ALLOW_MISSING_POLKIT=1 pour un paquet de test sans élévation)"
    fi
fi

# Documentation : copyright, changelog Debian compressé.
install -m 0644 "$PKG_DIR/deb/copyright" "$STAGE_DIR/usr/share/doc/$PKG_NAME/copyright"
sed "s|@VERSION@|$VERSION|g" "$PKG_DIR/deb/changelog" |
    gzip -9n >"$STAGE_DIR/usr/share/doc/$PKG_NAME/changelog.Debian.gz"

# Scripts de maintenance.
for script in postinst prerm postrm; do
    src="$PKG_DIR/deb/DEBIAN/$script"
    [ -f "$src" ] || die "script de maintenance manquant : $src"
    install -m 0755 "$src" "$STAGE_DIR/DEBIAN/$script"
done

# --- Fichier control -------------------------------------------------------
INSTALLED_SIZE="$(du -sk "$STAGE_DIR/usr" | cut -f 1)"
sed \
    -e "s|@VERSION@|$VERSION|g" \
    -e "s|@ARCH@|$ARCH|g" \
    -e "s|@INSTALLED_SIZE@|$INSTALLED_SIZE|g" \
    "$PKG_DIR/deb/DEBIAN/control" |
    grep -v '^#' >"$STAGE_DIR/DEBIAN/control"
chmod 0644 "$STAGE_DIR/DEBIAN/control"
grep -q '^Version: ' "$STAGE_DIR/DEBIAN/control" ||
    die "génération de control invalide"

# --- Construction du .deb --------------------------------------------------
DPKG_ARGS=(-Zxz)
if dpkg-deb --help 2>/dev/null | grep -q -- '--root-owner-group'; then
    # Tous les fichiers appartiennent à root:root dans l'archive.
    DPKG_ARGS+=(--root-owner-group)
else
    # Sans cette option, le helper pourrait appartenir à l'UID du build et
    # être remplacé par un utilisateur local, puis exécuté en root via pkexec.
    die "dpkg-deb ne prend pas en charge --root-owner-group : mise à jour de dpkg requise"
fi

mkdir -p "$DIST_DIR"
DEB="$DIST_DIR/${PKG_NAME}_${VERSION}_${ARCH}.deb"
rm -f "$DEB"
printf '==> dpkg-deb --build\n'
dpkg-deb "${DPKG_ARGS[@]}" --build "$STAGE_DIR" "$DEB" >/dev/null

printf '\nPaquet créé : %s\n' "$DEB"
printf 'Vérifications :\n'
printf '  dpkg-deb --info %q\n' "$DEB"
printf '  dpkg-deb --contents %q\n' "$DEB"
printf 'Installation : sudo apt install ./%s\n' "$(basename "$DEB")"
