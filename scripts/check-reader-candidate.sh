#!/usr/bin/env bash
# Description: Build and exercise the Linux AppImage internal reader candidate.

set -Eeuo pipefail

script_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
readonly script_root
repo_root=$(cd -- "$script_root/.." && pwd -P)
readonly repo_root
bundle_root=$repo_root/target/release/bundle/appimage

die() {
  printf 'check-reader-candidate: %s\n' "$1" >&2
  exit 1
}

say() {
  printf '[reader-candidate] %s\n' "$1" >&2
}

for command in awk find jq mise realpath sha256sum stat; do
  command -v "$command" >/dev/null || die "$command is required"
done
[[ $(uname -s) == Linux ]] || die 'this gate requires Linux'

cd -- "$repo_root"
mkdir -p .tmp
run_root=$(mktemp -d -p "$repo_root/.tmp" reader-candidate.XXXXXXXX)
cleanup() {
  local status=$?
  trap - EXIT INT TERM
  rm -rf -- "$run_root"
  exit "$status"
}
trap cleanup EXIT INT TERM

say 'building a clean AppImage candidate'
rm -rf -- "$bundle_root"
if ! command -v patchelf >/dev/null; then
  linuxdeploy=${TAURI_LINUXDEPLOY:-$HOME/.cache/tauri/linuxdeploy-x86_64.AppImage}
  [[ -x $linuxdeploy ]] || die 'patchelf or a cached linuxdeploy AppImage is required'
  tooling_root=$run_root/linuxdeploy-tooling
  mkdir -p -- "$tooling_root"
  (
    cd -- "$tooling_root"
    "$linuxdeploy" --appimage-extract >/dev/null
  )
  [[ -x $tooling_root/squashfs-root/usr/bin/patchelf ]] || die 'cached linuxdeploy has no patchelf'
  export PATH="$tooling_root/squashfs-root/usr/bin:$PATH"
fi
[[ -x /usr/lib/gstreamer-1.0/gst-plugin-scanner ]] &&
  export GSTREAMER_HELPERS_DIR=/usr/lib/gstreamer-1.0
if [[ -f /usr/lib/gstreamer-1.0/libgstpython.so ]]; then
  filtered_plugins=$run_root/gstreamer-plugins
  mkdir -p -- "$filtered_plugins"
  for plugin in /usr/lib/gstreamer-1.0/*; do
    [[ $plugin == */libgstpython.so ]] || ln -s -- "$plugin" "$filtered_plugins/"
  done
  export GSTREAMER_PLUGINS_DIR=$filtered_plugins
fi
# linuxdeploy's bundled strip predates the RELR sections used by rolling-release system libraries.
NO_STRIP=1 mise exec -- pnpm --dir reader/app tauri build --bundles appimage --ci >&2

shopt -s nullglob
artifacts=("$bundle_root"/*.AppImage)
(( ${#artifacts[@]} == 1 )) || die 'the build did not produce exactly one AppImage'
artifact=${artifacts[0]}
[[ -s $artifact && -x $artifact ]] || die 'the AppImage is empty or not executable'
artifact_sha=$(sha256sum -- "$artifact" | awk '{print $1}')
artifact_size=$(stat -c '%s' -- "$artifact")
artifact_relative=$(realpath --relative-to="$repo_root" -- "$artifact")

say 'extracting and checking candidate metadata'
(
  cd -- "$run_root"
  "$artifact" --appimage-extract >/dev/null
)
app_dir=$run_root/squashfs-root
launcher=$app_dir/AppRun
[[ -x $launcher ]] || die 'the extracted candidate has no executable AppRun'
mapfile -d '' desktop_files < <(find "$app_dir/usr/share/applications" -maxdepth 1 -type f -name '*.desktop' -print0)
(( ${#desktop_files[@]} == 1 )) || die 'the candidate does not contain exactly one desktop entry'
desktop_file=${desktop_files[0]}
exec_line=$(awk -F= '$1 == "Exec" {sub(/^Exec=/, ""); print; exit}' "$desktop_file")
[[ $exec_line == *'%F'* || $exec_line == *'%U'* ]] ||
  die 'the desktop entry has no multi-file association placeholder'
mime_line=$(awk -F= '$1 == "MimeType" {sub(/^MimeType=/, ""); print; exit}' "$desktop_file")
[[ -n $mime_line ]] || die 'the desktop entry has no MIME associations'
for mime in \
  application/epub+zip \
  application/vnd.comicbook+zip \
  application/x-fictionbook+xml \
  application/x-fictionbook+zip \
  application/x-mobipocket-ebook \
  application/vnd.amazon.mobi8-ebook \
  text/markdown \
  text/plain; do
  [[ ";$mime_line;" == *";$mime;"* ]] || die "desktop entry is missing $mime"
done

jq -e '
  ([.bundle.fileAssociations[].ext[]] | sort) ==
  (["azw", "azw3", "cbz", "epub", "fb2", "fbz", "markdown", "md", "mobi", "txt"] | sort)
' reader/app/src-tauri/tauri.windows.conf.json >/dev/null ||
  die 'Windows packaging does not declare all supported extensions'
jq -e '.bundle.fileAssociations? == null' reader/app/src-tauri/tauri.android.conf.json >/dev/null ||
  die 'Android must not inherit desktop file associations'

say 'running the complete Linux gate against the extracted candidate launcher'
reader_evidence=$(ATHA_GDK_BACKEND=x11 bash scripts/check-reader-linux.sh \
  --application "$launcher" \
  --association-launch)
jq -e '
  .evidence == "Linux Tauri GUI" and
  .fileAssociation.coldStart == true and
  .fileAssociation.validImported == 2 and
  .fileAssociation.invalidSkipped == true and
  .fileAssociation.repeatCountStable == true and
  .fileAssociation.ordinaryLaunchLibrary == true and
  .libraryEntry.syntheticTauriEvents == true and
  .libraryEntry.duplicateStable == true and
  .desktopWorkspace.focusVerified == true and
  .gestures.measurements == 220 and
  .appLogPrivacy == "pass"
' <<<"$reader_evidence" >/dev/null || die 'candidate reader evidence is incomplete'

jq -n \
  --arg artifact "$artifact_relative" \
  --arg sha256 "$artifact_sha" \
  --argjson sizeBytes "$artifact_size" \
  --arg desktopEntry "$(basename -- "$desktop_file")" \
  --arg launcher 'AppRun' \
  --argjson reader "$reader_evidence" \
  '{
    evidence: "Linux AppImage candidate",
    artifact: $artifact,
    sha256: $sha256,
    sizeBytes: $sizeBytes,
    desktopEntry: $desktopEntry,
    launcher: $launcher,
    fileAssociations: "pass",
    windowsConfig: "static-pass",
    androidAssociationIsolation: "pass",
    reader: $reader
  }'
