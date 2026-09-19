use std::process::Command;

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn main() {
    if let Ok(target) = std::env::var("TARGET") {
        println!("cargo:rustc-env=PULSEBRIDGE_TARGET={target}");
    }
    // Rebuild this stamp after a pull even when the package version is unchanged.
    for reference in [
        Some("HEAD".to_string()),
        git(&["symbolic-ref", "-q", "HEAD"]),
        Some("packed-refs".to_string()),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(path) = git(&["rev-parse", "--git-path", &reference]) {
            if std::path::Path::new(&path).exists() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
    }
    let revision =
        git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_else(|| "source-archive".to_string());
    println!("cargo:rustc-env=PULSEBRIDGE_REVISION={revision}");
    tauri_build::build()
}
