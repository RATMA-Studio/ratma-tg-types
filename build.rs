use std::fs;

use anyhow::Result;
use std::process::Command;
use std::sync::Mutex;
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

    match Command::new("rustfmt")
        .args(["--edition", "2024", &methods_path])
        .spawn()
    {
        Err(_) => {
            //println!("rustfmt not installed, skipping");
        }
        Ok(mut handle) => {
            handle.wait().unwrap();
        }
    }

    match Command::new("rustfmt")
        .args(["--edition", "2024", &types_path])
        .spawn()
    {
        Err(_) => {
            //println!("rustfmt not installed, skipping");
        }
        Ok(mut handle) => {
            handle.wait().unwrap();
        }
    }
    drop(guard);
    Ok(())
}
