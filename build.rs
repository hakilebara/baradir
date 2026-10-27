use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=manager/Cargo.toml");
    println!("cargo:rerun-if-changed=manager/src");
    println!("cargo:rerun-if-changed=manager/wit");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let target_dir = out_dir.join("component-target"); // separate dir avoids build-lock deadlock

    let status = Command::new(env::var("CARGO").unwrap())
        .args([
            "build",
            "--release",
            "--target",
            "wasm32-wasip2",
            "--manifest-path",
        ])
        .arg(manifest_dir.join("manager/Cargo.toml"))
        .arg("--target-dir")
        .arg(&target_dir)
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .status()
        .unwrap();
    assert!(
        status.success(),
        "component build failed (try `rustup target add wasm32-wasip2`)"
    );

    // Copy to a fixed, predictable name in OUT_DIR
    fs::copy(
        target_dir.join("wasm32-wasip2/release/manager.wasm"),
        out_dir.join("manager.wasm"),
    )
    .unwrap();
}
