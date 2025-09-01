use std::process::Command;

fn main() {
    // Set build timestamp
    let timestamp = chrono::Utc::now()
        .format("%Y-%m-%d %H:%M:%S UTC")
        .to_string();
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);

    // Get rustc version
    if let Ok(output) = Command::new("rustc").arg("--version").output() {
        let version = String::from_utf8_lossy(&output.stdout);
        println!("cargo:rustc-env=RUSTC_VERSION={}", version.trim());
    }

    // Rerun if build script changes
    println!("cargo:rerun-if-changed=build.rs");
}
