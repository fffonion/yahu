use std::process::Command;

fn main() {
    // Try git describe first (CI sets YAHU_VERSION env to skip git)
    if let Ok(ver) = std::env::var("YAHU_VERSION") {
        println!("cargo:rustc-env=YAHU_VERSION={ver}");
        return;
    }
    if let Ok(output) = Command::new("git")
        .args(["describe", "--tags", "--always"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
    {
        if output.status.success() {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !ver.is_empty() {
                println!("cargo:rustc-env=YAHU_VERSION={ver}");
                return;
            }
        }
    }
    // Fallback to Cargo.toml version
    println!("cargo:rustc-env=YAHU_VERSION={}", env!("CARGO_PKG_VERSION"));
}
