//! Explicit, reviewable hook installation. No credential or config.toml edits.
use std::{
    fs,
    io::{self, Write},
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    time::Duration,
};

use serde_json::{Value, json};
use uuid::Uuid;

const MARKER: &str = "OrangeDeck: 알림 및 직접 승인";

fn codex_version(output: &str) -> Option<(u64, u64, u64)> {
    let mut words = output.split_whitespace();
    if !matches!(words.next()?, "codex-cli" | "codex") {
        return None;
    }
    let version = words.next()?;
    if words.next().is_some() {
        return None;
    }
    // Unknown and prerelease builds do not opt in to a newer hook event.
    let mut parts = version.split('.');
    let parsed = (
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
        parts.next()?.parse().ok()?,
    );
    parts.next().is_none().then_some(parsed)
}

async fn probe_version(executable: &Path) -> Option<(u64, u64, u64)> {
    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tokio::process::Command::new(executable)
            .arg("--version")
            .kill_on_drop(true)
            .output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    codex_version(std::str::from_utf8(&output.stdout).ok()?)
}

fn quote(path: &Path) -> io::Result<String> {
    let text = path
        .to_str()
        .ok_or_else(|| io::Error::other("path is not valid UTF-8"))?;
    if text.chars().any(char::is_control) {
        return Err(io::Error::other("unsupported control character in path"));
    }
    Ok(format!("'{}'", text.replace('\'', "'\"'\"'")))
}

fn definition(executable: &Path, socket: &Path, timeout: u32) -> io::Result<Value> {
    Ok(
        json!({"type":"command", "command":format!("{} codex-hook --socket {}", quote(executable)?, quote(socket)?),
        "timeout":timeout, "statusMessage":MARKER}),
    )
}

fn merge(
    mut value: Value,
    executable: &Path,
    socket: &Path,
    include_interrupt: bool,
) -> io::Result<Value> {
    let root = value
        .as_object_mut()
        .ok_or_else(|| io::Error::other("existing hooks file is not an object"))?;
    let hooks = root
        .entry("hooks")
        .or_insert_with(|| json!({}))
        .as_object_mut()
        .ok_or_else(|| io::Error::other("existing hooks table is invalid"))?;
    for event in ["UserPromptSubmit", "Stop", "Interrupt", "PermissionRequest"] {
        let supported = event != "Interrupt" || include_interrupt;
        if !supported && !hooks.contains_key(event) {
            continue;
        }
        let groups = hooks
            .entry(event)
            .or_insert_with(|| json!([]))
            .as_array_mut()
            .ok_or_else(|| io::Error::other("existing hook groups are invalid"))?;
        for group in groups.iter_mut() {
            let handlers = group
                .get_mut("hooks")
                .and_then(Value::as_array_mut)
                .ok_or_else(|| io::Error::other("existing hook handler is invalid"))?;
            handlers.retain(|handler| {
                !(handler.get("statusMessage").and_then(Value::as_str) == Some(MARKER)
                    && handler
                        .get("command")
                        .and_then(Value::as_str)
                        .is_some_and(|command| command.contains(" codex-hook --socket ")))
            });
        }
        groups.retain(|group| {
            group
                .get("hooks")
                .and_then(Value::as_array)
                .is_some_and(|handlers| !handlers.is_empty())
        });
        if supported {
            let timeout = match event {
                "PermissionRequest" => 125,
                "Interrupt" => 3,
                _ => 5,
            };
            groups.push(json!({"hooks":[definition(executable, socket, timeout)?]}));
        } else if groups.is_empty() {
            // Remove the event key too: older Codex does not know Interrupt.
            // User-owned handlers in this event are still preserved.
            hooks.remove(event);
        }
    }
    Ok(value)
}

fn write_private_new(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn install(
    path: &Path,
    executable: &Path,
    socket: &Path,
    include_interrupt: bool,
) -> io::Result<Option<PathBuf>> {
    let existing = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && metadata.len() <= 262_144 => Some(fs::read(path)?),
        Ok(_) => {
            return Err(io::Error::other(
                "refusing non-regular or oversized hooks file",
            ));
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(error),
    };
    let value = existing
        .as_deref()
        .map_or_else(|| Ok(json!({})), serde_json::from_slice)
        .map_err(io::Error::other)?;
    let merged = merge(value, executable, socket, include_interrupt)?;
    if existing.as_deref().is_some_and(|bytes| {
        serde_json::from_slice::<Value>(bytes).is_ok_and(|value| value == merged)
    }) {
        return Ok(None);
    }
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing hooks directory"))?;
    // Codex must already be installed; never create an invented CODEX_HOME.
    if !parent.is_dir() {
        return Err(io::Error::other("Codex home does not exist"));
    }
    let backup = if let Some(bytes) = &existing {
        let backup = parent.join(format!("hooks.json.orangedeck-backup-{}", Uuid::new_v4()));
        write_private_new(&backup, bytes)?;
        Some(backup)
    } else {
        None
    };
    let temporary = parent.join(format!(".orangedeck-hooks-{}.tmp", Uuid::new_v4()));
    let mut bytes = serde_json::to_vec_pretty(&merged).map_err(io::Error::other)?;
    bytes.push(b'\n');
    write_private_new(&temporary, &bytes)?;
    // Abort if another editor changed the configuration since the reviewed read.
    if fs::read(path).ok() != existing {
        let _ = fs::remove_file(&temporary);
        return Err(io::Error::other(
            "hooks changed while installing; retry after reviewing the file",
        ));
    }
    fs::rename(&temporary, path)?;
    Ok(backup)
}

pub async fn run(
    apply: bool,
    hooks_file: Option<&Path>,
    codex_binary: &Path,
    config_path: &Path,
) -> io::Result<()> {
    let executable = std::env::current_exe()?;
    let socket = crate::paths::ConnectorPaths::for_config(config_path)?.hook_socket;
    let root = std::env::var_os("CODEX_HOME")
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".codex")))
        .ok_or_else(|| io::Error::other("cannot locate Codex home"))?;
    let path = hooks_file.map_or_else(|| root.join("hooks.json"), Path::to_path_buf);
    let version = probe_version(codex_binary).await;
    // Interrupt first shipped in stable Codex CLI 0.150.0.
    // https://learn.chatgpt.com/docs/changelog (2026-08-26)
    let include_interrupt = version.is_some_and(|version| version >= (0, 150, 0));
    if let Some((major, minor, patch)) = version {
        println!("Codex CLI: {major}.{minor}.{patch}");
    } else {
        println!("Codex CLI 버전을 확인하지 못했습니다.");
    }
    if !include_interrupt {
        println!(
            "Interrupt 훅은 Codex 0.150.0 이상에서만 설치합니다. 중단 감지는 새 세션 로그 기록을 사용합니다."
        );
    }
    if apply {
        let backup = install(&path, &executable, &socket, include_interrupt)?;
        println!("OrangeDeck 알림 연결 설정: {}", path.display());
        if let Some(backup) = backup {
            println!("기존 훅 원본 백업: {}", backup.display());
        }
        println!("Mac Codex에서 /hooks를 열어 OrangeDeck 훅을 검토하고 신뢰하세요.");
        println!(
            "/hooks가 없는 Codex 버전은 이 연결을 지원하지 않습니다. Codex를 자동 업데이트하지 않았습니다."
        );
        println!(
            "승인은 Ally에서 사용자가 누를 때만 적용됩니다. 연결 실패·120초 무응답은 기존 Mac 승인으로 돌아갑니다."
        );
    } else {
        println!("설치 대상: {}", path.display());
        println!(
            "{}",
            serde_json::to_string_pretty(&merge(
                json!({}),
                &executable,
                &socket,
                include_interrupt,
            )?)
            .map_err(io::Error::other)?
        );
        println!(
            "설치하려면 같은 명령에 --install을 추가하세요. 기존 훅은 보존하고 변경 전에 백업합니다."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn merges_only_our_hooks_and_is_idempotent() {
        let user = json!({"description":"user config", "hooks":{"Stop":[{"hooks":[{"type":"command","command":"user-hook"}]}]}});
        let exe = Path::new("/tmp/Orange Deck/connector");
        let socket = Path::new("/tmp/private/hooks.sock");
        let merged = merge(user, exe, socket, true).unwrap();
        assert_eq!(merged["description"], "user config");
        assert_eq!(
            merged["hooks"]["Stop"][0]["hooks"][0]["command"],
            "user-hook"
        );
        assert_eq!(merge(merged.clone(), exe, socket, true).unwrap(), merged);
        assert_eq!(
            merged["hooks"]["PermissionRequest"][0]["hooks"][0]["timeout"],
            125
        );
        assert_eq!(merged["hooks"]["Interrupt"][0]["hooks"][0]["timeout"], 3);
    }

    #[test]
    fn only_known_stable_codex_versions_enable_interrupt() {
        for output in ["codex-cli 0.146.0", "codex-cli 0.149.1"] {
            assert!(codex_version(output).is_some_and(|version| version < (0, 150, 0)));
        }
        for output in ["codex-cli 0.150.0\n", "codex-cli 0.153.2", "codex 1.0.0"] {
            assert!(codex_version(output).is_some_and(|version| version >= (0, 150, 0)));
        }
        for output in [
            "",
            "unknown 0.153.2",
            "codex-cli unknown",
            "codex-cli 0.150.0-alpha.1",
            "codex-cli 0.150",
            "codex-cli 0.150.0.1",
            "codex-cli 0.153.2 extra",
        ] {
            assert_eq!(codex_version(output), None, "{output}");
        }
    }

    #[test]
    fn older_codex_removes_only_our_interrupt_and_keeps_other_hooks() {
        let exe = Path::new("/tmp/connector");
        let socket = Path::new("/tmp/private/hooks.sock");
        let newer = merge(json!({}), exe, socket, true).unwrap();
        let older = merge(newer.clone(), exe, socket, false).unwrap();
        assert!(older["hooks"].get("Interrupt").is_none());
        for event in ["UserPromptSubmit", "Stop", "PermissionRequest"] {
            assert_eq!(older["hooks"][event], newer["hooks"][event]);
        }
        assert_eq!(merge(older.clone(), exe, socket, false).unwrap(), older);
        assert_eq!(merge(older, exe, socket, true).unwrap(), newer);

        let mut user = newer;
        user["hooks"]["Interrupt"]
            .as_array_mut()
            .unwrap()
            .push(json!({"hooks":[{"type":"command","command":"user-interrupt"}]}));
        let preserved = merge(user, exe, socket, false).unwrap();
        assert_eq!(
            preserved["hooks"]["Interrupt"],
            json!([{"hooks":[{"type":"command","command":"user-interrupt"}]}])
        );
    }

    #[test]
    fn reinstall_for_older_codex_backs_up_previous_file_and_is_idempotent() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("hooks.json");
        let exe = Path::new("/tmp/connector");
        let socket = Path::new("/tmp/private/hooks.sock");
        install(&path, exe, socket, true).unwrap();
        let original = fs::read(&path).unwrap();
        let backup = install(&path, exe, socket, false).unwrap().unwrap();
        assert_eq!(fs::read(&backup).unwrap(), original);
        let installed: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert!(installed["hooks"].get("Interrupt").is_none());
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(install(&path, exe, socket, false).unwrap().is_none());
    }

    #[test]
    fn shell_paths_are_quoted_and_malformed_config_is_rejected() {
        assert_eq!(
            quote(Path::new("/tmp/a'b$(id)`x`")).unwrap(),
            "'/tmp/a'\"'\"'b$(id)`x`'"
        );
        assert!(
            merge(
                json!({"hooks":null}),
                Path::new("/tmp/a"),
                Path::new("/tmp/b"),
                false,
            )
            .is_err()
        );
    }
}
