use std::env;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("../../memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());
    println!("cargo:rerun-if-changed=../../memory.x");
    println!("cargo:rerun-if-changed=build.rs");

    let output = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output();

    let git_hash = match output {
        Ok(out) if out.status.success() => {
            let s = String::from_utf8(out.stdout).unwrap_or_else(|_| "unknown".to_string());
            let trimmed = s.trim();
            if trimmed.is_empty() {
                "unknown".to_string()
            } else {
                trimmed.to_string()
            }
        }
        _ => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!(
        "cargo:rustc-env=FIRMWARE_VERSION=v{}",
        env!("CARGO_PKG_VERSION")
    );
}
