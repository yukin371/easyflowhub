use std::ffi::OsStr;
use std::path::Path;
use std::process::Command;

fn main() {
    build_scriptmgr();
    tauri_build::build()
}

fn build_scriptmgr() {
    if std::env::var("PROFILE").as_deref() != Ok("release") {
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR must be set by Cargo");
    let scriptmgr_src = Path::new(&manifest_dir)
        .parent()
        .and_then(Path::parent)
        .expect("scriptmgr-go must be a sibling of easyflowhub-app")
        .join("scriptmgr-go");

    watch_go_inputs(&scriptmgr_src, false)
        .unwrap_or_else(|error| panic!("failed to watch Go sidecar inputs: {error}"));

    println!("cargo:warning=Building scriptmgr.exe for release...");
    let output = Command::new("go")
        .args(["build", "-o", "scriptmgr.exe", "./cmd/scriptmgr"])
        .current_dir(&scriptmgr_src)
        .output()
        .unwrap_or_else(|error| panic!("failed to run Go sidecar build: {error}"));

    if !output.status.success() {
        panic!(
            "Go sidecar build failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    println!("cargo:warning=scriptmgr.exe built successfully");
}

fn watch_go_inputs(directory: &Path, watch_directory: bool) -> std::io::Result<()> {
    if watch_directory {
        println!("cargo:rerun-if-changed={}", directory.display());
    }

    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;

        if file_type.is_dir() {
            if entry.file_name() != OsStr::new(".git") {
                watch_go_inputs(&path, true)?;
            }
            continue;
        }

        let is_go_source = path.extension() == Some(OsStr::new("go"));
        let is_module_file = matches!(
            path.file_name().and_then(OsStr::to_str),
            Some("go.mod" | "go.sum" | "go.work" | "go.work.sum")
        );
        if is_go_source || is_module_file {
            println!("cargo:rerun-if-changed={}", path.display());
        }
    }

    Ok(())
}
