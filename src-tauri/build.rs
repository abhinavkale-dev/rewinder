#[cfg(target_os = "macos")]
fn build_screencapturekit_helper() {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let source_dir = manifest_dir.join("native").join("sck_capture");
    println!("cargo:rerun-if-changed={}", source_dir.display());

    let mut sources: Vec<PathBuf> = fs::read_dir(&source_dir)
        .unwrap_or_else(|err| {
            panic!(
                "failed to read ScreenCaptureKit helper source dir {}: {err}",
                source_dir.display()
            )
        })
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("swift"))
        .collect();
    sources.sort();
    if sources.is_empty() {
        panic!("no Swift helper sources found in {}", source_dir.display());
    }
    for source in &sources {
        println!("cargo:rerun-if-changed={}", source.display());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("out dir"));
    let helper_dir = manifest_dir.join("target").join("sck-helper");
    let _ = std::fs::create_dir_all(&helper_dir);
    let helper = helper_dir.join("rewinder-sck-capture");
    let module_cache = out_dir.join("swift-module-cache");

    let mut command = Command::new("xcrun");
    command
        .arg("swiftc")
        .arg("-parse-as-library")
        .arg("-O")
        .arg("-module-cache-path")
        .arg(&module_cache);
    for source in &sources {
        command.arg(source);
    }
    let status = command
        .arg("-o")
        .arg(&helper)
        .status()
        .expect("failed to run swiftc for ScreenCaptureKit helper");

    if !status.success() {
        panic!("swiftc failed building ScreenCaptureKit helper");
    }

    println!(
        "cargo:rustc-env=REWINDER_SCK_HELPER_PATH={}",
        helper.display()
    );
}

fn main() {
    #[cfg(target_os = "macos")]
    build_screencapturekit_helper();
}
