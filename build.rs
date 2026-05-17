use std::{fs, process::Command, sync::Mutex};

use anyhow::Result;
use ratma_tg_types_codegen::Generate;

fn main() -> Result<()> {
    println!("cargo:rerun-if-changed=generate/src/");
    println!("cargo:rerun-if-changed=telegram-bot-api-spec/");
    let mtx = Mutex::new(());
    let guard = mtx.lock().unwrap();
    let json = fs::read_to_string("./telegram-bot-api-spec/api.json")?;

    let generator = Generate::new(json)?;
    let types = generator.generate_types()?;
    let methods = generator.generate_methods()?;
    let out_dir = "./src";
    let methods_path = out_dir.to_owned() + "/gen_methods.rs";
    let types_path = out_dir.to_owned() + "/gen_types.rs";
    println!("cargo:rustc-env=BOT_GEN_DIR={}", out_dir);
    fs::write(&types_path, types)?;

    fs::write(&methods_path, methods)?;

    for path in [&methods_path, &types_path] {
        if let Ok(mut handle) = Command::new("rustup")
            .args(["run", "nightly", "rustfmt", "--edition", "2024", path])
            .spawn()
        {
            handle.wait().unwrap();
        }
    }
    drop(guard);
    Ok(())
}
