//! Prefer gilrs' device-directory discovery when udev lists inaccessible gamepads.
//! A Steam/isolated input setup can retain a blocked original pad alongside a usable virtual pad.

#[cfg(target_os = "linux")]
fn directory_backend_needed(
    sys_input: &std::path::Path,
    input: &std::path::Path,
    udev: &std::path::Path,
) -> bool {
    use std::os::unix::fs::PermissionsExt;
    let Ok(entries) = sys_input.read_dir() else {
        return false;
    };
    entries.flatten().any(|entry| {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("event") {
            return false;
        }
        let Ok(device) = std::fs::read_to_string(entry.path().join("dev")) else {
            return false;
        };
        let record = udev.join(format!("c{}", device.trim()));
        if !std::fs::read_to_string(record)
            .is_ok_and(|text| text.lines().any(|line| line == "E:ID_INPUT_JOYSTICK=1"))
        {
            return false;
        }
        std::fs::metadata(input.join(name)).map_or(true, |metadata| {
            let mode = metadata.permissions().mode();
            mode & 0o444 == 0 || mode & 0o222 == 0
        })
    })
}

/// Re-exec before GUI/controller threads exist. Child-only env avoids unsafe process env mutation.
pub fn prepare() -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        use std::{os::unix::process::CommandExt, path::Path};
        if std::env::var_os("GILRS_DISABLE_UDEV").is_none()
            && directory_backend_needed(
                Path::new("/sys/class/input"),
                Path::new("/dev/input"),
                Path::new("/run/udev/data"),
            )
        {
            let error = std::process::Command::new(std::env::current_exe()?)
                .args(std::env::args_os().skip(1))
                .env("GILRS_DISABLE_UDEV", "1")
                .exec();
            return Err(error);
        }
    }
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use std::{fs, os::unix::fs::PermissionsExt};

    #[test]
    fn inaccessible_original_pad_uses_directory_discovery_without_changing_permissions() {
        let root = std::env::temp_dir().join(format!("orangedeck-input-{}", uuid::Uuid::new_v4()));
        let sys = root.join("sys");
        let input = root.join("input");
        let udev = root.join("udev");
        for path in [sys.join("event7"), input.clone(), udev.clone()] {
            fs::create_dir_all(path).unwrap();
        }
        fs::write(sys.join("event7/dev"), "13:71\n").unwrap();
        fs::write(udev.join("c13:71"), "E:ID_INPUT_JOYSTICK=1\n").unwrap();
        let node = input.join("event7");
        fs::write(&node, "").unwrap();
        fs::set_permissions(&node, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(!directory_backend_needed(&sys, &input, &udev));
        fs::set_permissions(&node, fs::Permissions::from_mode(0o000)).unwrap();
        assert!(directory_backend_needed(&sys, &input, &udev));
        assert_eq!(fs::metadata(&node).unwrap().permissions().mode() & 0o777, 0);
        fs::remove_file(node).unwrap();
        assert!(directory_backend_needed(&sys, &input, &udev));
        fs::write(udev.join("c13:71"), "E:ID_INPUT_KEYBOARD=1\n").unwrap();
        assert!(!directory_backend_needed(&sys, &input, &udev));
        fs::remove_dir_all(root).unwrap();
    }
}
