#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::ffi::OsString;
use std::path::PathBuf;

fn import_argument(args: &[OsString]) -> Result<Option<PathBuf>, String> {
    if args.first().is_none_or(|arg| arg != "--import-runtime") {
        return Ok(None);
    }
    if args.len() != 2 {
        return Err("Uso: ro-launcher --import-runtime /ruta/absoluta/runtime.tar.zst".into());
    }
    let path = PathBuf::from(&args[1]);
    if !path.is_absolute() {
        return Err("La importación requiere una ruta absoluta".into());
    }
    Ok(Some(path))
}

fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match import_argument(&args) {
        Ok(Some(path)) => {
            let result = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .map_err(|error| format!("No se pudo iniciar la importación: {error}"))
                .and_then(|runtime| {
                    runtime.block_on(ro_launcher_lib::import_runtime_archive(&path))
                });
            match result {
                Ok(entrypoint) => println!("Runtime verificado: {}", entrypoint.display()),
                Err(error) => {
                    eprintln!("{error}");
                    std::process::exit(1);
                }
            }
        }
        Ok(None) => ro_launcher_lib::run(),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn runtime_import_is_explicit_and_does_not_parse_shell_words() {
        assert_eq!(import_argument(&[]).unwrap(), None);
        assert_eq!(import_argument(&args(&["--other-option"])).unwrap(), None);
        assert_eq!(
            import_argument(&args(&["--import-runtime", "/tmp/My Runtime.tar.zst"])).unwrap(),
            Some(PathBuf::from("/tmp/My Runtime.tar.zst"))
        );
        for values in [
            vec!["--import-runtime"],
            vec!["--import-runtime", "relative.zst"],
            vec!["--import-runtime", "/tmp/runtime.zst", "extra"],
        ] {
            assert!(import_argument(&args(&values)).is_err());
        }
    }
}
