fn main() {
    // Windows resources must rebuild when branding changes, even with --no-bundle.
    for icon in ["icons/icon.ico", "icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png"] {
        println!("cargo:rerun-if-changed={icon}");
    }
    tauri_build::build()
}
