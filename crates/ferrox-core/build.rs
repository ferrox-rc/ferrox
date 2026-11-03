use std::process::Command;

fn main() {
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
