//! Records the commit and the day the app was built, for About CreaBIM,
//! and on Windows gives the .exe its icon: the weaver and its nest.

use std::process::Command;

fn main() {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".into());
    // the build day, as days since 1970, turned into a date by the app
    let day = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() / 86_400)
        .unwrap_or(0);
    println!("cargo:rustc-env=CREABIM_COMMIT={commit}");
    println!("cargo:rustc-env=CREABIM_BUILD_DAY={day}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs/heads");
    println!("cargo:rerun-if-changed=assets/logo/volumen.ico");
    exe_icon();
}

#[cfg(windows)]
fn exe_icon() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/logo/volumen.ico");
        // who made it, in the .exe's Properties › Details
        res.set("ProductName", "Volumen");
        res.set("FileDescription", "Volumen, open-source BIM modeller");
        res.set("CompanyName", "Regis Benoit Brice Nde Tene");
        res.set(
            "LegalCopyright",
            "Copyright 2026 Regis Benoit Brice Nde Tene. GPL-3.0-or-later.",
        );
        res.set("OriginalFilename", "Volumen.exe");
        if let Err(e) = res.compile() {
            println!("cargo:warning=the .exe icon was not embedded: {e}");
        }
    }
}

#[cfg(not(windows))]
fn exe_icon() {}
