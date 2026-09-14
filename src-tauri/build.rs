use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn target_root(workspace: &Path) -> PathBuf {
    env::var("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| workspace.join("target"))
}

fn sidecar_artifact(workspace: &Path, target: &str, profile: &str, name: &str) -> PathBuf {
    let root = target_root(workspace);
    let with_target = root.join(target).join(profile).join(name);
    if with_target.exists() {
        return with_target;
    }
    root.join(profile).join(name)
}

fn copy_sidecar(workspace: &Path, manifest_dir: &Path, target: &str, profile: &str, name: &str) {
    let built = sidecar_artifact(workspace, target, profile, name);
    if built.exists() {
        let bin_dir = manifest_dir.join("binaries");
        if fs::create_dir_all(&bin_dir).is_ok() {
            let sidecar = bin_dir.join(format!("{name}-{target}"));
            let _ = fs::copy(&built, &sidecar);
        }
    } else {
        println!(
            "cargo:warning={name} no encontrado en {}; ejecuta `cargo build -p {name}` antes de empaquetar",
            built.display()
        );
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let workspace = manifest_dir.parent().expect("workspace root");
    let target = env::var("TARGET").unwrap();
    let profile = env::var("PROFILE").unwrap();

    println!(
        "cargo:rerun-if-changed={}",
        workspace.join("crates/ro-inputd").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace.join("crates/ro-sessiond").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        workspace.join("crates/ro-session-protocol").display()
    );
    println!("cargo:rerun-if-env-changed=RO_LAUNCHER_DISCORD_APPLICATION_ID");
    println!("cargo:rerun-if-env-changed=DISCORD_APPLICATION_ID");

    // No invocar `cargo build` aquí: bloquea el lock del build padre.
    // Los sidecars se compilan con `cargo build -p ro-inputd -p ro-sessiond` (ver npm scripts).
    copy_sidecar(workspace, &manifest_dir, &target, &profile, "ro-inputd");
    copy_sidecar(workspace, &manifest_dir, &target, &profile, "ro-sessiond");

    tauri_build::build();
}
