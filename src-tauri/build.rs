use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn ro_inputd_artifact(workspace: &Path, target: &str, profile: &str) -> PathBuf {
    let with_target = workspace
        .join("target")
        .join(target)
        .join(profile)
        .join("ro-inputd");
    if with_target.exists() {
        return with_target;
    }
    workspace.join("target").join(profile).join("ro-inputd")
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
    println!("cargo:rerun-if-env-changed=RO_LAUNCHER_DISCORD_APPLICATION_ID");
    println!("cargo:rerun-if-env-changed=DISCORD_APPLICATION_ID");

    // No invocar `cargo build` aquí: bloquea el lock del build padre.
    // El sidecar se compila con `cargo build -p ro-inputd` (ver npm scripts).
    let built = ro_inputd_artifact(workspace, &target, &profile);
    if built.exists() {
        let bin_dir = manifest_dir.join("binaries");
        if fs::create_dir_all(&bin_dir).is_ok() {
            let sidecar = bin_dir.join(format!("ro-inputd-{target}"));
            let _ = fs::copy(&built, &sidecar);
        }
    } else {
        println!(
            "cargo:warning=ro-inputd no encontrado en {}; ejecuta `cargo build -p ro-inputd` antes de empaquetar",
            built.display()
        );
    }

    tauri_build::build();
}
