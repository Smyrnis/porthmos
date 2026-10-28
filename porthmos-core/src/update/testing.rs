use std::{fs, os::unix::fs::PermissionsExt, path::Path, time::Duration};

use crate::Paths;

pub(crate) fn script(version: &str) -> String {
    format!("#!/bin/sh\necho \"porthmos {version}\"\n")
}

pub(crate) fn write_program(path: &Path, version: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, script(version)).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

pub(crate) fn run_version(path: &Path) -> String {
    let mut attempts = 0;
    let output = loop {
        match std::process::Command::new(path).arg("--version").output() {
            Err(error) if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 100 => {
                attempts += 1;
                std::thread::sleep(Duration::from_millis(20));
            }
            other => break other.unwrap(),
        }
    };
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub(crate) fn stage_on_disk(paths: &Paths, version: &str) -> porthmos_update::Staged {
    let binary = paths.update_dir().join(version).join("porthmos");
    write_program(&binary, version);
    let staged = porthmos_update::Staged {
        version: version.parse().unwrap(),
        binary: binary.clone(),
        sha256: porthmos_update::sha256_hex_of_file(&binary).unwrap(),
        size: fs::metadata(&binary).unwrap().len(),
    };
    porthmos_update::record_staged(&porthmos_update::UpdateDir::new(paths.update_dir()), &staged).unwrap();
    staged
}
