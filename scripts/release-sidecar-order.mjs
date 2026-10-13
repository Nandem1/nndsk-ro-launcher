const DEBUG_SIDECARS = /cargo build -p ro-inputd -p ro-sessiond(?! --release)/
const RELEASE_SIDECARS = 'cargo build -p ro-inputd -p ro-sessiond --release'
const CLIPPY = 'cargo clippy --workspace'
const APPIMAGE_BUILD = 'tauri build --bundles appimage'

export function sidecarOrderError(yaml) {
  const debug = yaml.search(DEBUG_SIDECARS)
  const clippy = yaml.indexOf(CLIPPY)
  const release = yaml.indexOf(RELEASE_SIDECARS)
  const bundle = yaml.indexOf(APPIMAGE_BUILD)
  if (debug < 0) {
    return 'release.yml must build debug sidecars (no --release) before clippy'
  }
  if (clippy < 0) {
    return 'release.yml must run cargo clippy'
  }
  if (release < 0) {
    return 'release.yml must build --release sidecars before the AppImage'
  }
  if (bundle < 0) {
    return 'release.yml must run tauri build --bundles appimage'
  }
  if (!(debug < clippy && clippy < release && release < bundle)) {
    return 'release.yml sidecar order must be debug build, clippy, --release build, AppImage build'
  }
  return null
}
