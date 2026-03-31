fn main() {
    // Build scriptmgr.exe before Tauri compilation
    build_scriptmgr();
    tauri_build::build()
}

fn build_scriptmgr() {
    use std::process::Command;
    use std::path::Path;

    // Only build during release builds
    if !std::env::var("PROFILE").map(|p| p == "release").unwrap_or(false) {
        return;
    }

    // Check if we should skip (already built or no Go toolchain)
    if let Ok(skip) = std::env::var("SKIP_SCRIPTMGR_BUILD") {
        if skip == "1" {
            println!("cargo:warning=Skipping scriptmgr build (SKIP_SCRIPTMGR_BUILD=1)");
            return;
        }
    }

    // Find scriptmgr-go directory (parent of src-tauri)
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default();
    let scriptmgr_src = Path::new(&manifest_dir)
        .parent() // src-tauri
        .and_then(|p| p.parent()) // easyflowhub-app
        .map(|p| p.join("scriptmgr-go"))
        .unwrap_or_else(|| Path::new("..").join("scriptmgr-go"));

    let scriptmgr_exe = scriptmgr_src.join("scriptmgr.exe");

    // Skip if already built and up-to-date
    if scriptmgr_exe.exists() {
        if let Ok(src_mtime) = std::fs::metadata(&scriptmgr_src.join("cmd").join("scriptmgr").join("main.go"))
            .and_then(|m| m.modified())
        {
            if let Ok(dst_mtime) = std::fs::metadata(&scriptmgr_exe).and_then(|m| m.modified()) {
                if dst_mtime > src_mtime {
                    println!("cargo:warning=scriptmgr.exe already up-to-date");
                    return;
                }
            }
        }
    }

    println!("cargo:warning=Building scriptmgr.exe...");
    let output = Command::new("go")
        .args(["build", "-o", "scriptmgr.exe", "./cmd/scriptmgr"])
        .current_dir(&scriptmgr_src)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("cargo:warning=scriptmgr.exe built successfully");
        }
        Ok(out) => {
            let stderr = String::from_utf8_lossy(&out.stderr);
            eprintln!("cargo:warning=Failed to build scriptmgr: {}", stderr);
        }
        Err(e) => {
            eprintln!("cargo:warning=Failed to run go build: {}", e);
        }
    }
}
