use std::{
    net::IpAddr,
    path::{Path, PathBuf},
    str::FromStr,
};

use orangedeck_domain::{Project, SystemSnapshot};
use thiserror::Error;
use tokio::process::Command;

use crate::{AgentConfig, BindMode};

pub fn tailscale_command() -> Command {
    let binary = std::env::var_os("ORANGEDECK_TAILSCALE_BINARY").map_or_else(
        || {
            #[cfg(target_os = "macos")]
            {
                let app = Path::new("/Applications/Tailscale.app/Contents/MacOS/Tailscale");
                if app.is_file() {
                    return app.to_path_buf();
                }
            }
            PathBuf::from("tailscale")
        },
        PathBuf::from,
    );
    let mut command = Command::new(binary);
    command.env("TAILSCALE_BE_CLI", "1");
    command
}

pub async fn tailscale_ip() -> Result<IpAddr, SystemError> {
    let output = tailscale_command()
        .args(["ip", "-4"])
        .output()
        .await
        .map_err(SystemError::Spawn)?;
    if !output.status.success() {
        return Err(SystemError::Command(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let value = String::from_utf8_lossy(&output.stdout);
    IpAddr::from_str(value.trim()).map_err(|error| SystemError::Parse(error.to_string()))
}

pub async fn tailscale_stdout(args: &[&str]) -> Option<String> {
    let output = tailscale_command().args(args).output().await.ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

pub async fn resolve_bind_address(
    config: &AgentConfig,
) -> Result<std::net::SocketAddr, SystemError> {
    let ip = match config.bind_mode {
        BindMode::Tailscale => tailscale_ip().await?,
        BindMode::Loopback => IpAddr::from([127, 0, 0, 1]),
        BindMode::Explicit => config
            .bind_address
            .ok_or_else(|| SystemError::Parse("explicit bind address is missing".to_owned()))?,
    };
    Ok(std::net::SocketAddr::new(ip, config.port))
}

pub async fn hostname() -> String {
    command_stdout("hostname", &[])
        .await
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown-host".to_owned())
}

pub async fn collect_system_snapshot() -> SystemSnapshot {
    #[cfg(target_os = "linux")]
    {
        collect_linux_system_snapshot().await
    }
    #[cfg(target_os = "macos")]
    {
        collect_macos_system_snapshot().await
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        SystemSnapshot::default()
    }
}

#[cfg(target_os = "linux")]
async fn collect_linux_system_snapshot() -> SystemSnapshot {
    let meminfo = tokio::fs::read_to_string("/proc/meminfo")
        .await
        .unwrap_or_default();
    let total_kib = meminfo_value(&meminfo, "MemTotal:");
    let available_kib = meminfo_value(&meminfo, "MemAvailable:");
    let load = tokio::fs::read_to_string("/proc/loadavg")
        .await
        .unwrap_or_default();
    let uptime = tokio::fs::read_to_string("/proc/uptime")
        .await
        .unwrap_or_default();
    SystemSnapshot {
        cpu_percent: None,
        memory_used_bytes: total_kib
            .zip(available_kib)
            .map(|(total, available)| total.saturating_sub(available) * 1024),
        memory_total_bytes: total_kib.map(|value| value * 1024),
        load_average: load.split_whitespace().next().and_then(|v| v.parse().ok()),
        uptime_seconds: uptime
            .split_whitespace()
            .next()
            .and_then(|value| value.split('.').next())
            .and_then(|value| value.parse().ok()),
    }
}

#[cfg(target_os = "linux")]
fn meminfo_value(input: &str, key: &str) -> Option<u64> {
    input
        .lines()
        .find(|line| line.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

#[cfg(target_os = "macos")]
async fn collect_macos_system_snapshot() -> SystemSnapshot {
    let total = command_stdout("sysctl", &["-n", "hw.memsize"])
        .await
        .and_then(|value| value.parse().ok());
    let uptime = command_stdout("sysctl", &["-n", "kern.boottime"])
        .await
        .and_then(|value| {
            value
                .split("sec = ")
                .nth(1)?
                .split(',')
                .next()?
                .parse()
                .ok()
        })
        .and_then(|boot: u64| {
            u64::try_from(chrono::Utc::now().timestamp())
                .ok()
                .map(|now| now.saturating_sub(boot))
        });
    SystemSnapshot {
        memory_total_bytes: total,
        uptime_seconds: uptime,
        ..SystemSnapshot::default()
    }
}

pub fn open_editor(project: &Project) -> Result<(), SystemError> {
    let path = path_str(&project.path)?;
    #[cfg(target_os = "macos")]
    {
        let mut directories = vec![PathBuf::from("/Applications")];
        if let Some(user_home) = std::env::var_os("HOME") {
            directories.push(PathBuf::from(user_home).join("Applications"));
        }
        for app in ["Zed.app", "Visual Studio Code.app", "VSCodium.app"] {
            for directory in &directories {
                let bundle = directory.join(app);
                if bundle.is_dir() {
                    return spawn_detached("open", &["-a", path_str(&bundle)?, path]);
                }
            }
        }
        Err(SystemError::Command(
            "Install Zed, Visual Studio Code or VSCodium in Applications to use Open editor"
                .to_owned(),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    spawn_first_available(
        &[
            ("zed", &[path]),
            ("zeditor", &[path]),
            ("code", &[path]),
            ("codium", &[path]),
        ],
        "Install Zed, VS Code or VSCodium and expose its command in PATH to use Open editor",
    )
}

pub fn open_terminal(project: &Project) -> Result<(), SystemError> {
    #[cfg(target_os = "macos")]
    return spawn_detached("open", &["-a", "Terminal", path_str(&project.path)?]);

    #[cfg(not(target_os = "macos"))]
    {
        let path = path_str(&project.path)?;
        spawn_first_available(
            &[
                ("konsole", &["--workdir", path]),
                ("gnome-terminal", &["--working-directory", path]),
                ("xfce4-terminal", &["--working-directory", path]),
            ],
            "Install Konsole, GNOME Terminal or Xfce Terminal to use Open terminal",
        )
    }
}

#[cfg(not(target_os = "macos"))]
fn spawn_first_available(candidates: &[(&str, &[&str])], missing: &str) -> Result<(), SystemError> {
    for (program, args) in candidates {
        match spawn_detached(program, args) {
            Err(SystemError::Spawn(error)) if error.kind() == std::io::ErrorKind::NotFound => {}
            result => return result,
        }
    }
    Err(SystemError::Command(missing.to_owned()))
}

pub fn open_project(project: &Project) -> Result<(), SystemError> {
    #[cfg(target_os = "macos")]
    return spawn_detached("open", &[path_str(&project.path)?]);

    #[cfg(not(target_os = "macos"))]
    spawn_detached("xdg-open", &[path_str(&project.path)?])
}

pub fn open_browser(project: &Project) -> Result<(), SystemError> {
    let url = project
        .browser_url
        .as_deref()
        .ok_or(SystemError::MissingBrowserUrl)?;
    #[cfg(target_os = "macos")]
    return spawn_detached("open", &[url]);

    #[cfg(not(target_os = "macos"))]
    spawn_detached("xdg-open", &[url])
}

fn path_str(path: &Path) -> Result<&str, SystemError> {
    path.to_str()
        .ok_or_else(|| SystemError::Parse(format!("path is not valid UTF-8: {}", path.display())))
}

fn spawn_detached(program: &str, args: &[&str]) -> Result<(), SystemError> {
    Command::new(program)
        .args(args)
        .kill_on_drop(false)
        .spawn()
        .map_err(SystemError::Spawn)?;
    Ok(())
}

pub async fn command_stdout(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().await.ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

#[derive(Debug, Error)]
pub enum SystemError {
    #[error("cannot start system command: {0}")]
    Spawn(std::io::Error),
    #[error("system command failed: {0}")]
    Command(String),
    #[error("cannot parse system command output: {0}")]
    Parse(String),
    #[error("selected project has no configured browser_url")]
    MissingBrowserUrl,
}
