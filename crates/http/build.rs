use std::{env, fs, path::PathBuf, process::Command};

use anyhow::Result;
use ratma_tg_types_codegen::Generate;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=../codegen/src/");
    println!("cargo:rerun-if-changed=../../telegram-bot-api-spec/");
    let json = fs::read_to_string("../../telegram-bot-api-spec/api.json")?;

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
