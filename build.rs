fn main() {
    create_rtf_license();
    _ = embed_resource::compile("assets/main.rc", std::iter::empty::<&std::ffi::OsStr>());
}

fn create_rtf_license() {
    let license_content = std::fs::read_to_string("LICENSE").expect("Failed to read LICENSE file");
    let rtf_safe_content = license_content
        .replace('\\', "\\\\")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('\n', "\\par\n");
    let rtf_output = format!(
        r#"{{\rtf1\ansi\deff0
{{\fonttbl{{\f0 Arial;}}}}
\fs20
{}
}}"#,
        rtf_safe_content
    );
    let assets_dir = std::path::Path::new("assets");
    if !assets_dir.exists() {
        std::fs::create_dir(assets_dir).expect("Failed to create assets directory");
    }
    std::fs::write("assets/License.rtf", rtf_output).expect("Failed to write assets/License.rtf");
}
