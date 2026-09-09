//! Editor adapters for recorded files inside their Codex conversation's working folder.
use serde::{Deserialize, Serialize};
use std::{
    path::{Component, Path, PathBuf},
    process::Stdio,
    time::Duration,
};
use tokio::process::Command;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditorKind {
    #[default]
    Auto,
    Zed,
    VsCode,
    Cursor,
    Vscodium,
}

impl std::str::FromStr for EditorKind {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "zed" => Ok(Self::Zed),
            "vs_code" | "vscode" => Ok(Self::VsCode),
            "cursor" => Ok(Self::Cursor),
            "vscodium" => Ok(Self::Vscodium),
            _ => Err("Choose auto, zed, vs_code, cursor or vscodium".to_owned()),
        }
    }
}

impl EditorKind {
    fn candidates(self) -> &'static [Self] {
        match self {
            Self::Auto => &[Self::Zed, Self::VsCode, Self::Cursor, Self::Vscodium],
            Self::Zed => &[Self::Zed],
            Self::VsCode => &[Self::VsCode],
            Self::Cursor => &[Self::Cursor],
            Self::Vscodium => &[Self::Vscodium],
        }
    }

    #[cfg(target_os = "macos")]
    fn bundle_cli(self) -> &'static str {
        match self {
            Self::Zed | Self::Auto => "Zed.app/Contents/MacOS/cli",
            Self::VsCode => "Visual Studio Code.app/Contents/Resources/app/bin/code",
            Self::Cursor => "Cursor.app/Contents/Resources/app/bin/cursor",
            Self::Vscodium => "VSCodium.app/Contents/Resources/app/bin/codium",
        }
    }

    #[cfg(not(target_os = "macos"))]
    fn binary(self) -> &'static str {
        match self {
            Self::Zed | Self::Auto => "zed",
            Self::VsCode => "code",
            Self::Cursor => "cursor",
            Self::Vscodium => "codium",
        }
    }
}

pub fn conversation_editor_workspace(cwd: &str) -> Result<PathBuf, String> {
    let path = Path::new(cwd);
    if !path.is_absolute()
        || path.parent().is_none()
        || cwd.len() > 4096
        || cwd.chars().any(char::is_control)
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("Codex 대화의 작업 폴더 경로가 올바르지 않습니다 / This Codex conversation has an invalid working folder".to_owned());
    }
    let workspace = path.canonicalize().map_err(|_| {
        format!("이 대화의 작업 폴더를 Mac에서 찾을 수 없습니다: {cwd} / This conversation's working folder is unavailable on the Mac: {cwd}")
    })?;
    if !workspace.is_dir() || workspace.parent().is_none() {
        return Err("Codex 대화의 작업 폴더가 유효한 폴더가 아닙니다 / This Codex conversation's working folder is not a valid directory".to_owned());
    }
    Ok(workspace)
}

pub fn resolve_editor_file(workspace: &Path, recorded_path: &str) -> Result<PathBuf, String> {
    let path = Path::new(recorded_path);
    if recorded_path.chars().any(char::is_control)
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err("수정 파일 경로가 올바르지 않습니다 / Invalid changed-file path".to_owned());
    }
    // The caller supplies this conversation's canonical cwd. Do not follow a
    // replacement root symlink and silently switch to a different directory.
    let root = workspace;
    let target = if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    };
    let target = target
        .canonicalize()
        .map_err(|_| "파일이 삭제되었거나 이동했습니다 / File was removed or moved")?;
    if !target.starts_with(root) || !target.is_file() {
        return Err("이 대화의 작업 폴더 안에 있는 수정 파일만 열 수 있습니다 / Only changed files inside this conversation's working folder can be opened".to_owned());
    }
    Ok(target)
}

fn editor_arguments(editor: EditorKind, path: &Path, line: u32) -> Vec<String> {
    let target = format!("{}:{}:1", path.display(), line.clamp(1, 10_000_000));
    if editor == EditorKind::Zed {
        vec!["--existing".to_owned(), target]
    } else {
        vec!["--reuse-window".to_owned(), "--goto".to_owned(), target]
    }
}

pub async fn open_changed_file(
    workspace: &Path,
    path: &str,
    line: u32,
    editor: EditorKind,
) -> Result<(), String> {
    let target = resolve_editor_file(workspace, path)?;
    for &candidate in editor.candidates() {
        #[cfg(target_os = "macos")]
        let programs = {
            let mut directories = vec![PathBuf::from("/Applications")];
            if let Some(home) = std::env::var_os("HOME") {
                directories.push(PathBuf::from(home).join("Applications"));
            }
            directories
                .into_iter()
                .map(|directory| directory.join(candidate.bundle_cli()))
                .filter(|path| path.is_file())
                .collect::<Vec<_>>()
        };
        #[cfg(not(target_os = "macos"))]
        let programs = vec![PathBuf::from(candidate.binary())];
        for program in programs {
            let mut command = Command::new(program);
            command
                .kill_on_drop(true)
                .args(editor_arguments(candidate, &target, line))
                .current_dir(workspace)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            // The CLI exits after handing the request to the editor; do not use --wait.
            // A timeout drops only our handoff process, avoiding late queued CLI requests.
            match tokio::time::timeout(Duration::from_secs(12), command.status()).await {
                Ok(Ok(status)) if status.success() => return Ok(()),
                Ok(Err(error)) if error.kind() == std::io::ErrorKind::NotFound => {},
                Err(_) => return Err("편집기 응답이 늦습니다. Mac에서 실행 상태를 확인하세요 / Editor response timed out; check your Mac".to_owned()),
                _ => return Err("편집기에 파일을 열지 못했습니다 / Could not open the file in your editor".to_owned()),
            }
        }
    }
    Err("Mac의 connector.toml에서 editor를 확인하세요 (zed / vs_code / cursor / vscodium) / Check your Mac editor setting and installation".to_owned())
}
