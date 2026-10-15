//! Explicit, ignored Wine leg of F-10a. No game, graphics, provisioning or reset claim.
use std::path::{Path, PathBuf};
use std::time::Duration;

use ro_tools_linux::{find_prefix_processes, verify_process_identity};

use super::*;
use crate::utils::pe::{pe_architecture, PeArchitecture};

pub(crate) async fn qualify_wine716_pe32(installed: &Path) {
    let root = std::env::temp_dir().join(format!("ro-wine716-pe-{}", uuid::Uuid::new_v4()));
    let home = root.join("home");
    std::fs::create_dir_all(&home).unwrap();
    let pe = PathBuf::from(
        std::env::var_os("RO_WINE716_TEST_PE32")
            .expect("set RO_WINE716_TEST_PE32 to the compiled nonce.c"),
    );
    assert!(pe.is_absolute());
    let evidence = root.join("evidence.json");
    let log = std::fs::File::create(root.join("runtime.log")).unwrap();
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(workspace)
        .output()
        .unwrap();
    assert!(commit.status.success());
    let mut command = tokio::process::Command::new(std::env::current_exe().unwrap());
    command
        .env_clear()
        .args([
            "--exact",
            "tools::prefix::setup::qualification::wine716_pe32_child",
            "--ignored",
            "--nocapture",
        ])
        .env("HOME", &home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env(
            "PATH",
            format!(
                "{}:/usr/local/bin:/usr/bin:/bin",
                root.join("appimage/usr/bin").display()
            ),
        )
        .env("APPDIR", root.join("appimage"))
        .env("PYTHONHOME", root.join("appimage/usr"))
        .env("PYTHONPATH", root.join("appimage/usr/python"))
        .env("LD_LIBRARY_PATH", root.join("appimage/usr/lib"))
        // Console PE qualification does not provision optional Mono/Gecko. Avoid their
        // interactive/network installer in the disposable test HOME, including under Xvfb.
        .env("WINEDLLOVERRIDES", "mscoree,mshtml=")
        .env("RO_WINE716_TEST_RUNNER", installed.join("bin/wine"))
        .env("RO_WINE716_TEST_PE32", pe)
        .env("RO_WINE716_EVIDENCE", &evidence)
        .env(
            "RO_WINE716_COMMIT",
            String::from_utf8(commit.stdout).unwrap().trim(),
        )
        .env(
            "RO_FIRST_RUN_SESSIOND",
            workspace.join("target/debug/ro-sessiond"),
        )
        .stdout(log.try_clone().unwrap())
        .stderr(log)
        .kill_on_drop(true);
    for key in [
        "DISPLAY",
        "XAUTHORITY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    let status = tokio::time::timeout(Duration::from_secs(240), command.status())
        .await
        .expect("Wine qualification timed out")
        .unwrap();
    println!(
        "Isolated Wine qualification HOME/log/evidence: {}",
        root.display()
    );
    assert!(status.success(), "see {}/runtime.log", root.display());
    let record: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&evidence).unwrap()).unwrap();
    assert_eq!(record["cleanup"], "PASS");
    if let Some(destination) = std::env::var_os("RO_WINE716_EVIDENCE") {
        let destination = PathBuf::from(destination);
        assert!(destination.is_absolute());
        std::fs::copy(&evidence, destination).unwrap();
    }
    println!("{}", serde_json::to_string_pretty(&record).unwrap());
}

#[tokio::test]
#[ignore = "child of the verified Wine archive test; requires disposable HOME and nonce PE32"]
async fn wine716_pe32_child() {
    use sha2::{Digest, Sha256};

    assert!(
        crate::tools::environment::host::wine716_loader_check(
            Path::new("/lib/ld-linux.so.2").is_file()
        )
        .is_none(),
        "host-elf32: missing multilib loader"
    );
    let runner = PathBuf::from(std::env::var_os("RO_WINE716_TEST_RUNNER").unwrap());
    let pe = PathBuf::from(std::env::var_os("RO_WINE716_TEST_PE32").unwrap());
    let pe_bytes = std::fs::read(&pe).unwrap();
    assert_eq!(pe_architecture(&pe_bytes), Some(PeArchitecture::I386));
    let resolved = resolve_runner(runner.to_str().unwrap()).unwrap();
    assert!(resolved.is_wine_7_16());
    assert_eq!(resolved.wine_sync_mode(), WineSyncMode::Fsync);
    let probe = crate::tools::runtime::probe_runner(&resolved);
    let server = serde_json::from_value(serde_json::json!({"id":"wine716-pe-gate", "name":"Wine PE qualification", "executablePath":"/unused/game.exe", "prefixMode":"isolated"})).unwrap();
    let identity =
        crate::tools::runtime::resolve_prefix_binding(Some(&server), &resolved, &probe, true)
            .unwrap();
    let ctx = WineContext {
        prefix: identity.location.path.clone(),
        location: identity.location.clone(),
        identity,
        resolved,
        probe,
    };
    let anchor = crate::tools::runtime::session_anchor_from_context(&ctx);
    assert!(!crate::utils::app_data_dir().exists());
    let sessions = RunnerSessionRegistry::with_sidecar_for_test(PathBuf::from(
        std::env::var_os("RO_FIRST_RUN_SESSIOND").unwrap(),
    ));
    let game = GameProcessHandle::new();
    let mut cycles = Vec::new();
    for cycle in 0..2 {
        println!("Wine PE32 cycle={cycle}: begin");
        let op = RunnerOperation::begin(None, &sessions, &game, &ctx, &anchor)
            .await
            .unwrap();
        let supervisor = op.lease().supervisor_identity();
        let guard = OperationGuard::acquire("prefix", Path::new(&ctx.prefix)).unwrap();
        let managed =
            ManagedPrefix::new(&ctx.location, Some(&super::super::desired_identity(&ctx))).unwrap();
        if cycle == 0 {
            managed
                .begin_provisioning(&super::super::desired_identity(&ctx))
                .unwrap();
        }
        let cwd = std::env::temp_dir().join(format!("ro-wine716-nonce-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&cwd).unwrap();
        let nonce = uuid::Uuid::new_v4().to_string();
        let result = tokio::time::timeout(Duration::from_secs(90), async {
            if cycle == 0 {
                super::initialize_prefix(&op).await?;
                println!("Wine PE32 cycle={cycle}: initialized");
            } else if !inspect_prefix(&ctx.prefix).configured {
                return Err("reuse lost its manifest".into());
            }
            let invocation = ctx.resolved.game_invocation(
                &ctx.prefix,
                pe.to_str().unwrap(),
                [&nonce],
                cwd.to_str().unwrap(),
            )?;
            let command = invocation.clone().into_command();
            let removed: Vec<String> = command
                .as_std()
                .get_envs()
                .filter(|(_, value)| value.is_none())
                .map(|(key, _)| key.to_string_lossy().into_owned())
                .collect();
            for key in ["APPDIR", "PYTHONHOME", "PYTHONPATH", "LD_LIBRARY_PATH"] {
                if !removed.iter().any(|removed| removed == key) {
                    return Err(format!("environment key not sanitized: {key}"));
                }
            }
            let code = op.run(invocation, "PE32 nonce").await?;
            Ok((removed, code))
        })
        .await
        .unwrap_or_else(|_| Err("Wine PE32 cycle exceeded 90 seconds".into()));
        let cleanup = op.into_cleanup();
        let quiesced = cleanup.quiesce().await;
        let (removed, code) = result
            .map_err(|error| cleanup.annotate_failure(error))
            .unwrap();
        quiesced.unwrap();
        assert!(!verify_process_identity(&supervisor));
        assert!(find_prefix_processes(&ctx.prefix).is_empty());
        println!("Wine PE32 cycle={cycle}: exit={code}, quiesced");
        assert_eq!(code, 42);
        assert_eq!(
            std::fs::read_to_string(cwd.join("nonce.txt")).unwrap(),
            nonce
        );
        let registry = std::fs::read_to_string(Path::new(&ctx.prefix).join("system.reg")).unwrap();
        // Production leaves Wine's default win64 prefix policy intact. PE32 executes
        // through the artifact's old-WoW64 leg; do not inject a test-only WINEARCH.
        assert!(registry.lines().any(|line| line == "#arch=win64"));
        if cycle == 0 {
            // Ownership-only manifest, no installed-component claim and no reset/recovery leg.
            managed.revoke_rollback_authority().unwrap();
            write_prefix_manifest_v3(
                &ctx.prefix,
                &PrefixManifestV3 {
                    schema_version: PREFIX_SCHEMA_V3,
                    scope: ctx.location.scope,
                    server_id: ctx.location.server_id.clone(),
                    runner_kind: ctx.resolved.kind_label().into(),
                    runner_path: runner.to_string_lossy().into_owned(),
                    components: vec![],
                    prefix_fingerprint: PrefixFingerprintEnvelope {
                        schema_version: 1,
                        algorithm: "sha256".into(),
                        digest: ctx.identity.desired_fingerprint.digest.hex_digest(),
                    },
                },
            )
            .unwrap();
            super::super::recover_with_guard_held(
                &ctx.location,
                Some(&super::super::desired_identity(&ctx)),
            )
            .unwrap();
        }
        let manifest = inspect_prefix(&ctx.prefix).manifest.unwrap();
        assert_eq!(manifest.runner_path(), runner.to_str().unwrap());
        let manifest_json: serde_json::Value = serde_json::from_slice(
            &std::fs::read(Path::new(&ctx.prefix).join(".ro-launcher-prefix.json")).unwrap(),
        )
        .unwrap();
        cycles.push(serde_json::json!({"cycle":cycle, "exitCode":42, "nonce":nonce, "prefixArchitecture":"win64", "prefixManifest":manifest_json, "sanitizedEnvironmentKeys":removed, "testDllOverrides":"mscoree,mshtml=", "sessiond":{"pid":supervisor.pid, "startTime":supervisor.start_time}, "cleanup":"PASS"}));
        drop(guard);
    }
    assert!(sessions.shutdown_all().await.is_empty());
    let record = serde_json::json!({"commit":std::env::var("RO_WINE716_COMMIT").unwrap(), "runnerDigest":crate::tools::artifacts::expected_digest_hex(crate::tools::artifacts::catalog_descriptor(crate::tools::artifacts::MANAGED_WINE716_ID).unwrap().digest), "peDigest":format!("{:x}", Sha256::digest(&pe_bytes)), "wineVersion":ctx.resolved.reported_version(), "cycles":cycles, "cleanup":"PASS"});
    std::fs::write(
        std::env::var_os("RO_WINE716_EVIDENCE").unwrap(),
        serde_json::to_vec_pretty(&record).unwrap(),
    )
    .unwrap();
}
