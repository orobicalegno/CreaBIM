#!/usr/bin/env bash
# Volumen's RPM (Fedora, RHEL 10 and the like), from a release build:
#   make-volumen-rpm.sh VERSION TARGET_DIR OUTPUT.rpm
# It installs /usr/bin/volumen and /usr/bin/volumen-cli, a menu entry and
# its icon. Kept apart from make-rpm.sh (CreaBIM's), so merging CreaBIM's
# changes stays simple.
set -euo pipefail
umask 022

if [[ $# -ne 3 ]]; then
    echo "Usage: $0 VERSION TARGET_DIR OUTPUT.rpm" >&2
    exit 2
fi
version=$1
target_dir=$2
output=$3
if [[ ! $version =~ ^[0-9][A-Za-z0-9.+~_]*$ ]]; then
    echo "Invalid RPM package version: $version" >&2
    exit 2
fi

root=$(cd "$(dirname "$0")/../.." && pwd)
cd "$root"
build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
topdir="$build/rpmbuild"
install -d "$topdir"/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}

stage="$topdir/SOURCES/root"
install -d "$stage/usr/bin" "$stage/usr/share/applications" \
    "$stage/usr/share/icons/hicolor/512x512/apps" "$stage/usr/share/doc/volumen"
install -m 0755 "$target_dir/creabim-app" "$stage/usr/bin/volumen"
install -m 0755 "$target_dir/creabim" "$stage/usr/bin/volumen-cli"
install -m 0644 crates/creabim-app/assets/logo/png/volumen-app-icon-512.png \
    "$stage/usr/share/icons/hicolor/512x512/apps/volumen.png"
install -m 0644 LICENSE "$stage/usr/share/doc/volumen/copyright"
install -m 0644 README.md "$stage/usr/share/doc/volumen/README.md"

cat > "$stage/usr/share/applications/volumen.desktop" <<'DESKTOP'
[Desktop Entry]
Type=Application
Name=Volumen
Comment=Open-source BIM modeller with AutoCAD-style 2D drafting
Exec=volumen
TryExec=volumen
Icon=volumen
Terminal=false
Categories=Graphics;3DGraphics;Engineering;
DESKTOP

cat > "$topdir/SPECS/volumen.spec" <<SPEC
Name:           volumen
Version:        $version
Release:        1%{?dist}
Summary:        Open-source BIM modeller with AutoCAD-style 2D drafting
License:        GPL-3.0-or-later
URL:            https://github.com/orobicalegno/CreaBIM
BuildArch:      x86_64
Requires:       glibc >= 2.35
Requires:       gtk3
Requires:       webkit2gtk4.1
Requires:       libxkbcommon-x11

%description
Volumen, a modified CreaBIM: model a building in 3D and draw its plans,
sections and sheets, with AutoCAD's command line, snaps and UCS.

%prep

%build

%install
mkdir -p "%{buildroot}"
cp -a "%{_sourcedir}/root/." "%{buildroot}/"

%files
%{_bindir}/volumen
%{_bindir}/volumen-cli
%{_datadir}/applications/volumen.desktop
%{_datadir}/icons/hicolor/512x512/apps/volumen.png
%dir %{_datadir}/doc/volumen
%license %{_datadir}/doc/volumen/copyright
%{_datadir}/doc/volumen/README.md
SPEC

mkdir -p "$(dirname "$output")"
rpmbuild -bb --define "_topdir $topdir" "$topdir/SPECS/volumen.spec"
rpm_file=$(find "$topdir/RPMS" -type f -name '*.rpm' -print -quit)
if [[ -z $rpm_file ]]; then
    echo "rpmbuild completed without producing an RPM" >&2
    exit 1
fi
cp "$rpm_file" "$output"
echo "Created $output"
