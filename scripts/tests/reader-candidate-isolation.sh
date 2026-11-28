#!/usr/bin/env bash
# Regress destructive packaging failures without building a second AppImage.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
fixture=$(mktemp -d)
trap 'rm -rf -- "$fixture"' EXIT
mkdir -p "$fixture"/{bin,scripts,target/release/bundle/appimage}
cp -- "$root/scripts/check-reader-candidate.sh" "$fixture/scripts/"
cat > "$fixture/bin/mise" <<'MOCK'
#!/usr/bin/env bash
set -eu
case "$*" in
  *'tauri build --no-bundle --ci -- --locked')
    printf '%s\n' 'compiled release' > "$CANDIDATE_FIXTURE/target/release/atha-reader-app"
    chmod +x "$CANDIDATE_FIXTURE/target/release/atha-reader-app"
    ;;
  *'cargo metadata --locked --no-deps --format-version 1')
    jq -n --arg target "$CANDIDATE_FIXTURE/target" '{target_directory:$target}'
    ;;
  *'tauri bundle --bundles appimage --ci')
    [[ $CARGO_TARGET_DIR == "$CANDIDATE_FIXTURE"/.tmp/reader-candidate.*/target ]]
    [[ -x $CARGO_TARGET_DIR/release/atha-reader-app ]]
    mkdir -p "$CARGO_TARGET_DIR/release/bundle/appimage"
    printf '%s\n' 'failed bundle output' > "$CARGO_TARGET_DIR/release/bundle/appimage/Atha.AppImage"
    exit 23
    ;;
  *) exit 99 ;;
esac
MOCK
printf '#!/bin/sh\nexit 0\n' > "$fixture/bin/patchelf"
chmod +x "$fixture/bin/"*
printf 'previous candidate\n' > "$fixture/target/release/bundle/appimage/Atha.AppImage"
status=0
CANDIDATE_FIXTURE="$fixture" PATH="$fixture/bin:$PATH" bash "$fixture/scripts/check-reader-candidate.sh" || status=$?
[[ $status == 23 && $(cat "$fixture/target/release/bundle/appimage/Atha.AppImage") == 'previous candidate' ]]
printf '%s\n' 'Failed isolated packaging preserved the previous AppImage.'
