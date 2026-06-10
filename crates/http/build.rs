use std::{env, fs, path::PathBuf, process::Command};

use anyhow::{Context, Result};
use ratma_tg_types_codegen::Generate;

/// Reads the Telegram Bot API spec shipped next to the crate manifest.
///
/// `api.json` is a symlink into the `telegram-bot-api-spec` submodule inside
/// the workspace checkout and a regular file in the published package, so the
/// same `CARGO_MANIFEST_DIR`-relative path works for both.
fn read_spec() -> Result<String> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let spec_path = manifest_dir.join("api.json");
    println!("cargo:rerun-if-changed={}", spec_path.display());
    fs::read_to_string(&spec_path).with_context(|| {
        format!(
            "failed to read the Telegram Bot API spec at {}; in a workspace checkout make sure \
             the `telegram-bot-api-spec` git submodule is initialized",
            spec_path.display()
        )
    })
}

fn main() -> Result<()> {
    let json = read_spec()?;

    let generator = Generate::new(json)?;
    let _types = generator.generate_types()?;
    let methods = generator.generate_methods()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let methods_path = out_dir.join("methods.rs");
    fs::write(&methods_path, methods)?;

    if let Ok(mut handle) = Command::new("rustup")
        .args([
            "run",
            "nightly",
            "rustfmt",
            "--edition",
            "2024",
            methods_path.to_str().unwrap()
        ])
        .spawn()
    {
        handle.wait().unwrap();
    }

    Ok(())
}
