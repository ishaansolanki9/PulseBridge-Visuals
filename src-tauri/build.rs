fn main() {
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=PULSEBRIDGE_TARGET={target}");
    }
    tauri_build::build()
}
