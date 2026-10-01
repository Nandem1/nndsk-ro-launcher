const DEBUG_SIDECARS = /cargo build -p ro-inputd -p ro-sessiond(?! --release)/
const RELEASE_SIDECARS = 'cargo build -p ro-inputd -p ro-sessiond --release'
const CLIPPY = 'cargo clippy --workspace'
const TAURI_ACTION = 'tauri-apps/tauri-action'

export function sidecarOrderError(yaml) {
  const debug = yaml.search(DEBUG_SIDECARS)
  const clippy = yaml.indexOf(CLIPPY)
  const release = yaml.indexOf(RELEASE_SIDECARS)
  const tauri = yaml.indexOf(TAURI_ACTION)
  if (debug < 0) {
    return 'release.yml must build debug sidecars (no --release) before clippy'
  }
  if (clippy < 0) {
    return 'release.yml must run cargo clippy'
  }
  if (release < 0) {
    return 'release.yml must build --release sidecars before tauri-action'
  }
  if (tauri < 0) {
    return 'release.yml must run tauri-action'
  }
  if (!(debug < clippy && clippy < release && release < tauri)) {
    return 'release.yml sidecar order must be debug build, clippy, --release build, tauri-action'
  }
  return null
}
