use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command
};

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

/// Formats a generated file with nightly `rustfmt`, best-effort.
///
/// The generated code compiles unformatted, so a missing nightly toolchain or
/// a `rustfmt` failure only emits a `cargo:warning` and never fails the build.
fn format_best_effort(path: &Path) {
    let Some(path_str) = path.to_str() else {
        println!(
            "cargo:warning=skipping rustfmt on generated code: non-UTF-8 path {}",
            path.display()
        );
        return;
    };
    match Command::new("rustup")
        .args(["run", "nightly", "rustfmt", "--edition", "2024", path_str])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => println!(
            "cargo:warning=rustfmt on generated code exited with {status}; leaving it unformatted"
        ),
        Err(err) => println!(
            "cargo:warning=failed to run nightly rustfmt on generated code ({err}); leaving it \
             unformatted"
        )
    }
}

fn main() -> Result<()> {
    let json = read_spec()?;

    let generator = Generate::new(json)?;
    let _types = generator.generate_types()?;
    let methods = generator.generate_methods()?;

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let methods_path = out_dir.join("methods.rs");
    fs::write(&methods_path, methods)?;
    format_best_effort(&methods_path);

    Ok(())
}
