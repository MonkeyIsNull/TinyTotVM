use std::fs;
use std::path::Path;
use std::process::Command;

#[test]
fn run_all_ttvms() {
    let dir = Path::new("examples");

    for entry in fs::read_dir(dir).expect("Cannot read examples directory") {
        let entry = entry.expect("Invalid entry");
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) == Some("ttvm") {
            let file_name = path.to_string_lossy();
            let output = Command::new("cargo")
                .args(&["run", "--quiet", "--", &file_name])
                .output()
                .expect("Failed to run command");

            assert!(
                output.status.success(),
                "Program {} failed:\nstdout:\n{}\nstderr:\n{}",
                file_name,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}
