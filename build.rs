use std::fs;
use std::path::Path;

fn main() {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let src = Path::new(&manifest_dir).join("assets");
    let out_dir = std::env::var("OUT_DIR").unwrap();
    let profile = std::env::var("PROFILE").unwrap_or_else(|_| "debug".to_string());
    let bin_dir = extract_matching_parent_dir(&out_dir, &profile).expect("Failed to find bin dir");
    let dst = bin_dir.join("assets");
    if src.exists() {
        copy_dir_all(&src, &dst).expect("Failed to copy assets directory");
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if file_type.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

pub fn extract_matching_parent_dir<P: AsRef<std::path::Path>>(path: P, match_name: &str) -> std::io::Result<std::path::PathBuf> {
    path.as_ref()
        .ancestors()
        .find(|p| p.file_name() == Some(std::ffi::OsStr::new(match_name)))
        .map(|p| p.to_path_buf())
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("No parent directory matching '{match_name}' found"),
            )
        })
}
