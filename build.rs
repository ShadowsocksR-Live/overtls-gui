use embed_manifest::manifest::{ActiveCodePage, Setting, SupportedOS::*};
use embed_manifest::{embed_manifest, new_manifest};

fn main() {
    // Tell Cargo to rerun this build script if the build script changes
    println!("cargo::rerun-if-changed=build.rs");

    // Check if we're building for Windows (either natively or cross-compiling)
    let target = std::env::var("TARGET").unwrap_or_default();

    if target.contains("windows") {
        let pkg_name = std::env::var("CARGO_PKG_NAME").unwrap();
        embed_windows_manifest(&pkg_name);
        _ = embed_resource::compile("assets/main.rc", embed_resource::NONE);
    }

    // Set the environment variables GIT_HASH
    if let Ok(git_hash) = get_git_hash() {
        println!("cargo:rustc-env=GIT_HASH={}", git_hash.trim());
    }

    // Get the build time
    let build_time = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S").to_string();
    println!("cargo:rustc-env=BUILD_TIME={build_time}");
}

fn get_git_hash() -> std::io::Result<String> {
    use std::process::Command;
    let git_hash = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output()?.stdout;
    let git_hash = String::from_utf8(git_hash).map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    Ok(git_hash)
}

fn embed_windows_manifest(name: &str) {
    // Create a comprehensive manifest for Windows theming and modern features
    let manifest = new_manifest(name)
        // Enable modern Windows Common Controls (v6) for theming
        // Windows10 is the latest supported in the enum
        .supported_os(Windows7..=Windows10)
        // Set UTF-8 as active code page for better Unicode support
        .active_code_page(ActiveCodePage::Utf8)
        // Enable heap type optimization for better performance (if available)
        .heap_type(embed_manifest::manifest::HeapType::SegmentHeap)
        // Enable high-DPI awareness for crisp displays
        .dpi_awareness(embed_manifest::manifest::DpiAwareness::PerMonitorV2)
        // Enable long path support (if configured in Windows)
        .long_path_aware(Setting::Enabled);

    // Embed the manifest - this works even when cross-compiling!
    if let Err(e) = embed_manifest(manifest) {
        // This should not happen with embed-manifest as it supports cross-compilation
        println!("cargo::warning=Failed to embed manifest: {e}");
        println!("cargo::warning=The application will still work but may lack optimal Windows theming");
    }
}
