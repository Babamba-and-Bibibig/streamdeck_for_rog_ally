#!/usr/bin/env python3
"""Guided per-user installation. Standard library only; never run this with sudo."""
import argparse
import ipaddress
import json
import os
from pathlib import Path
import platform
import shlex
import shutil
import stat
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
RUST_VERSION = "1.95.0"
TAILNET = ipaddress.ip_network("100.64.0.0/10")


class SetupError(Exception):
    pass


def checked_text(value):
    if not isinstance(value, str) or not value or any(ord(c) < 32 or ord(c) == 127 for c in value):
        raise SetupError("A nonempty value without control characters is required / 빈 값·제어문자 불가")
    return value


def run(args, env=None, capture=False, timeout=None):
    # Arguments are always an array. Never evaluate user input as shell code.
    try:
        result = subprocess.run([str(a) for a in args], cwd=ROOT, env=env, timeout=timeout,
                                stdout=subprocess.PIPE if capture else None,
                                stderr=subprocess.PIPE if capture else None, text=True)
    except subprocess.TimeoutExpired:
        raise SetupError(f"{Path(str(args[0])).name} did not respond. Check that it is running and signed in / 실행·로그인 상태를 확인하세요.") from None
    if result.returncode:
        # Captured output can contain login details or config secrets. Do not echo it.
        raise SetupError(f"{Path(str(args[0])).name} failed (exit {result.returncode}). Review locally and retry.")
    return result.stdout.strip() if capture else None


def ask(label, default, unattended):
    if unattended:
        if default is None:
            raise SetupError(f"Missing setting: {label}")
        return checked_text(str(default))
    hint = f" [{default}]" if default is not None else ""
    value = input(f"{label}{hint}: ").strip() or default
    return checked_text(str(value)) if value is not None else ask(label, default, False)


def existing_path(value):
    """Accept a pasted path or Terminal's quoted/escaped drag-and-drop path."""
    raw = checked_text(value)
    direct = Path(raw).expanduser().absolute()
    if direct.exists():
        return direct
    try:
        parts = shlex.split(raw)
    except ValueError:
        parts = []
    if len(parts) == 1:
        candidate = Path(parts[0]).expanduser().absolute()
        if candidate.exists():
            return candidate
    raise SetupError("Path not found. Paste the full path / 전체 경로를 붙여넣으세요.")


def ask_path(label, default, unattended, directory=False):
    while True:
        try:
            path = existing_path(ask(label, default, unattended))
            if directory and not path.is_dir():
                raise SetupError("Choose a folder / 폴더를 선택하세요.")
            return path
        except SetupError as error:
            if unattended:
                raise
            print(error)
            default = None


def saved_codex_home(directory):
    """Recover only our saved profile path, without executing a previous launcher."""
    metadata = directory / "install-agent.json"
    if metadata.exists() or metadata.is_symlink():
        info = metadata.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_size > 65_536:
            raise SetupError("Cannot safely read previous installation metadata.")
        document = json.loads(metadata.read_text())
        if not isinstance(document, dict):
            raise SetupError("Previous installation metadata must be an object.")
        value = document.get("codex_home")
        if value is not None:
            return checked_text(value)
    # 0.1.17/18 stored CODEX_HOME only in their generated launcher.
    start = directory / "start-agent.command"
    if start.exists() or start.is_symlink():
        info = start.lstat()
        if not stat.S_ISREG(info.st_mode) or info.st_uid != os.getuid() or info.st_size > 65_536:
            raise SetupError("Cannot safely read the previous launcher.")
        for line in start.read_text().splitlines():
            if line.startswith("export CODEX_HOME="):
                fields = shlex.split(line)
                if len(fields) != 2 or not fields[1].startswith("CODEX_HOME="):
                    raise SetupError("Review the previous launcher profile before updating.")
                return checked_text(fields[1].split("=", 1)[1])
    return None


def consent(label, allowed, unattended):
    if allowed:
        return True
    return not unattended and input(f"{label} [y/N]: ").strip().lower() in {"y", "yes"}


def executable(name, explicit=None, extra=()):
    if explicit:
        candidate = shutil.which(explicit) or str(Path(explicit).expanduser())
        candidates = [candidate]
    else:
        candidates = [shutil.which(name), *extra]
    for candidate in candidates:
        if candidate and Path(candidate).is_file() and os.access(candidate, os.X_OK):
            # Do not resolve rustup's cargo/rustc symlinks; invocation name is meaningful.
            return Path(os.path.abspath(candidate))
    return None


def secure_write(path, data, mode=0o600):
    if path.parent.is_symlink():
        raise SetupError(f"Refusing symlink directory / 심볼릭 링크 폴더 거부: {path.parent.name}")
    path.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
    if path.is_symlink():
        raise SetupError(f"Refusing symlink / 심볼릭 링크 거부: {path.name}")
    fd, temporary = tempfile.mkstemp(prefix=".orangedeck-install-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(data)
            output.flush()
            os.fchmod(output.fileno(), mode)
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def launcher(binary, arguments, env):
    exports = "".join(f"export {key}={shlex.quote(value)}\n" for key, value in env.items())
    return ("#!/bin/sh\nset -eu\n" + exports + "exec "
            + " ".join(shlex.quote(str(x)) for x in [binary, *arguments]) + '\n').encode()


def install_dependencies(role, options):
    if platform.system() == "Darwin":
        if subprocess.run(["xcode-select", "-p"], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode:
            if consent("Open Apple Command Line Tools installer? / Apple 개발 도구 설치 창 열기?",
                       options.install_deps, options.non_interactive):
                run(["xcode-select", "--install"])
            raise SetupError("Finish Apple Command Line Tools installation, then rerun / Apple 개발 도구 설치 후 다시 실행하세요.")
        return
    missing = not executable("cc") or not executable("pkg-config")
    if role == "ui" and not missing:
        missing = subprocess.run(["pkg-config", "--exists", "libudev", "xkbcommon", "wayland-client"],
                                 stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode != 0
    if not missing:
        return
    if executable("apt-get"):
        packages = ["build-essential", "pkg-config", "curl"]
        if role == "ui":
            packages += ["libudev-dev", "libxkbcommon-dev", "libwayland-dev", "libx11-dev", "libxi-dev", "libgl1-mesa-dev", "libdbus-1-dev", "libnotify-bin"]
        commands = [["sudo", "apt-get", "update"], ["sudo", "apt-get", "install", "-y", *packages]]
    elif executable("dnf"):
        packages = ["gcc", "gcc-c++", "make", "pkgconf-pkg-config", "curl"]
        if role == "ui":
            packages += ["systemd-devel", "libxkbcommon-devel", "wayland-devel", "libX11-devel", "libXi-devel", "mesa-libGL-devel", "dbus-devel", "libnotify"]
        commands = [["sudo", "dnf", "install", "-y", *packages]]
    elif executable("pacman"):
        packages = ["base-devel", "pkgconf", "curl"]
        if role == "ui":
            packages += ["systemd", "libxkbcommon", "wayland", "libx11", "libxi", "mesa", "dbus", "libnotify"]
        commands = [["sudo", "pacman", "-S", "--needed", *packages]]
    else:
        raise SetupError("Install a C toolchain and pkg-config; Linux UI also needs libudev, XKB, Wayland and OpenGL development libraries. See docs/INSTALL.md.")
    print("\n".join(shlex.join(command) for command in commands))
    if not consent("Install missing build dependencies? / 필요한 빌드 패키지 설치?",
                   options.install_deps, options.non_interactive):
        raise SetupError("Install the dependencies above and retry / 위 패키지 설치 후 다시 실행하세요.")
    for command in commands:
        run(command)


def cargo_tool(options):
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    cargo = executable("cargo", extra=[cargo_home / "bin/cargo"])
    rustup = executable("rustup", extra=[cargo_home / "bin/rustup"])
    if not cargo or not rustup:
        if not consent("Download Rust from sh.rustup.rs and install for this user? / 공식 Rust 설치?",
                       options.install_rust, options.non_interactive):
            raise SetupError("Install Rust with rustup: https://rust-lang.org/tools/install/")
        curl = executable("curl")
        if not curl:
            raise SetupError("curl is required to download Rust over HTTPS.")
        with tempfile.TemporaryDirectory(prefix="orangedeck-rustup-") as tmp:
            script = Path(tmp) / "rustup-init.sh"
            run([curl, "--proto", "=https", "--tlsv1.2", "--fail", "--silent", "--show-error",
                 "--location", "--proto-redir", "=https", "https://sh.rustup.rs", "--output", script])
            run(["sh", script, "-y", "--profile", "minimal", "--no-modify-path", "--default-toolchain", RUST_VERSION])
        cargo = executable("cargo", extra=[cargo_home / "bin/cargo"])
        rustup = executable("rustup", extra=[cargo_home / "bin/rustup"])
    if not cargo:
        raise SetupError("Cargo installation was not found.")
    if not rustup:
        raise SetupError("rustup is required for the pinned toolchain.")
    # A distro Cargo on PATH is not a rustup proxy and does not understand +version.
    cargo = executable("cargo", str(rustup.parent / "cargo"))
    if not cargo:
        raise SetupError("Cannot find the Cargo proxy next to rustup.")
    installed = run([rustup, "toolchain", "list"], capture=True)
    if not any(line.startswith(RUST_VERSION + "-") for line in installed.splitlines()):
        run([rustup, "toolchain", "install", RUST_VERSION, "--profile", "minimal"])
    version = run([cargo, f"+{RUST_VERSION}", "--version"], capture=True)
    print(version)
    return cargo


def tailscale_binary():
    result = executable("tailscale", extra=[
        "/usr/local/bin/tailscale", "/opt/homebrew/bin/tailscale",
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
    ])
    if not result:
        raise SetupError("Install and sign in to Tailscale on both devices: https://tailscale.com/download")
    return result


def config_directory(value=None):
    base = Path(os.environ.get("XDG_CONFIG_HOME", str(Path.home() / ".config")))
    return Path(value).expanduser().absolute() if value else base.expanduser().absolute() / "orangedeck"


def private_pairing(path):
    path = Path(path).expanduser().absolute()
    metadata = path.lstat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > 16_384 or metadata.st_uid != os.getuid():
        raise SetupError("Pairing must be your own regular file, at most 16 KiB / 본인 소유의 작은 일반 페어링 파일이 필요합니다.")
    path.chmod(0o600)
    return path


def parser():
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("--role", choices=["agent", "ui"])
    result.add_argument("--project-path")
    result.add_argument("--project-name")
    result.add_argument("--host-name")
    result.add_argument("--port", type=int, default=45831)
    result.add_argument("--codex-binary")
    result.add_argument("--pairing")
    result.add_argument("--config-dir")
    result.add_argument("--install-rust", action="store_true", help="Allow downloading the official Rust installer")
    result.add_argument("--install-deps", action="store_true", help="Allow the displayed system package installation")
    result.add_argument("--non-interactive", action="store_true")
    result.add_argument("--start", action="store_true", help="Start the installed app in this terminal")
    result.add_argument("--check", action="store_true", help="Show platform/prerequisite availability without installing or contacting Codex")
    return result


def main():
    options = parser().parse_args()
    system = platform.system()
    if system not in {"Linux", "Darwin"} or sys.version_info < (3, 9):
        raise SetupError("Supported: macOS Agent / Linux Agent or desktop UI, Python 3.9+. Windows is not supported.")
    if os.geteuid() == 0:
        raise SetupError("Run as your normal account, without sudo / 일반 계정으로 실행하세요.")
    if options.check:
        print(f"Platform: {system}; Python {sys.version_info.major}.{sys.version_info.minor}")
        for name in ["cargo", "rustup", "cc", "pkg-config", "codex", "tailscale"]:
            print(f"{name}: {'on PATH' if executable(name) else 'not on PATH (installer also searches standard locations)'}")
        return
    role = options.role or ("agent" if system == "Darwin" else ask("Role / 역할 (agent or ui)", "ui", options.non_interactive))
    if role not in {"agent", "ui"} or (role == "ui" and system != "Linux"):
        raise SetupError("The desktop installer currently supports Linux UI only / 화면 앱 설치는 Linux에서 지원합니다.")
    if not 1 <= options.port <= 65535:
        raise SetupError("Port must be 1–65535.")
    directory = config_directory(options.config_dir)
    checked_text(str(directory))
    if directory.is_symlink():
        raise SetupError("Config directory must not be a symlink.")
    config = directory / ("agent.toml" if role == "agent" else "config.toml")
    if config.is_symlink():
        raise SetupError("Config must not be a symlink.")
    if not config.exists():
        credentials = ["agent.token", "orangedeck-pairing.toml"] if role == "agent" else ["ui.token"]
        if any(os.path.lexists(directory / name) for name in credentials):
            raise SetupError("Credentials exist without a config. Restore/review the existing setup or choose a new config directory / 설정 없이 남은 인증 파일이 있습니다. 기존 설정을 복원·확인하거나 새 설정 폴더를 사용하세요.")
    if config.exists() and options.pairing:
        raise SetupError("Already paired. Setup preserves existing credentials; use the documented explicit re-pair command.")
    if config.exists():
        print("Existing config and token will be preserved / 기존 설정·토큰 유지. Project/host/port options apply only to first setup.")
    project = codex = host = project_name = pairing = None
    if role == "agent" and not config.exists():
        project = ask_path("Project folder / 프로젝트 폴더", options.project_path, options.non_interactive, directory=True).resolve(strict=True)
        host = ask("Host label / 기기 표시 이름", options.host_name or "CODEX HOST", options.non_interactive)
        project_name = ask("Project label / 프로젝트 표시 이름", options.project_name or project.name, options.non_interactive)
    if role == "agent" and not config.exists():
        codex = executable("codex", options.codex_binary, [Path.home()/".local/bin/codex", "/opt/homebrew/bin/codex", "/usr/local/bin/codex"])
        if not codex:
            codex = executable("codex", ask("Codex executable / Codex 실행 파일 경로", options.codex_binary, options.non_interactive))
        if not codex:
            raise SetupError("Install Codex CLI and sign in first: https://learn.chatgpt.com/docs/cli")
    elif not config.exists():
        pairing = private_pairing(ask_path("Pairing file from Agent / Agent의 페어링 파일 경로", options.pairing, options.non_interactive))
    print("\n[1/4] Checking your environment / 실행 환경 확인")
    tailscale = tailscale_binary()
    process_env = os.environ.copy()
    process_env["TAILSCALE_BE_CLI"] = "1"
    if role == "agent":
        profile = os.environ.get("CODEX_HOME") or saved_codex_home(directory) or str(Path.home() / ".codex")
        process_env["CODEX_HOME"] = checked_text(str(Path(profile).expanduser().absolute()))
    address = ipaddress.ip_address(run([tailscale, "ip", "-4"], env=process_env, capture=True, timeout=10))
    if address not in TAILNET:
        raise SetupError("Tailscale is not connected / Tailscale 연결을 확인하세요.")
    if codex:
        run([codex, "login", "status"], env=process_env, capture=True, timeout=15)
    install_dependencies(role, options)
    cargo = cargo_tool(options)
    # Persist lookup directories and the selected profile, including on later updates.
    process_env["PATH"] = os.pathsep.join(dict.fromkeys([str(cargo.parent), str(tailscale.parent),
                            *([str(codex.parent)] if codex else []), *os.environ.get("PATH", "").split(os.pathsep)]))
    saved_env = {"PATH": process_env["PATH"], "TAILSCALE_BE_CLI": "1", "ORANGEDECK_TAILSCALE_BINARY": str(tailscale)}
    process_env["ORANGEDECK_TAILSCALE_BINARY"] = str(tailscale)
    if role == "agent":
        saved_env["CODEX_HOME"] = process_env["CODEX_HOME"]
    name = f"orangedeck-{role}"
    print("\n[2/4] Building OrangeDeck; first build can take several minutes / 첫 빌드는 몇 분 이상 걸릴 수 있습니다")
    run([cargo, f"+{RUST_VERSION}", "build", "--locked", "--release", "-p", name], env=process_env)
    # Cargo can be configured to put builds outside target/. Ask Cargo for the actual path.
    metadata = json.loads(run([cargo, f"+{RUST_VERSION}", "metadata", "--no-deps", "--format-version", "1", "--locked"], env=process_env, capture=True))
    source = Path(metadata["target_directory"]) / "release" / name
    binary = directory / "bin" / name
    if config.exists():
        previous = json.loads(run([source, "check-config", "--config", config], env=process_env, capture=True))
        if role == "agent":
            codex = executable("codex", previous["codex_binary"])
            if not codex:
                raise SetupError("The Codex path in your existing agent.toml is unavailable. Update that private setting and retry.")
            run([codex, "login", "status"], env=process_env, capture=True, timeout=15)
            process_env["PATH"] = os.pathsep.join(dict.fromkeys([str(codex.parent), *process_env["PATH"].split(os.pathsep)]))
            saved_env["PATH"] = process_env["PATH"]
    print("\n[3/4] Installing private settings and launchers / 개인 설정과 실행기 설치")
    secure_write(binary, source.read_bytes(), 0o700)
    if not config.exists():
        if role == "agent":
            run([binary, "init", "--config", config, "--project-path", project, "--project-name", project_name,
                 "--host-name", host, "--port", str(options.port), "--cargo-binary", cargo, "--codex-binary", codex], env=process_env)
        else:
            run([binary, "pair", "--config", config, "--bundle", pairing], env=process_env)
    start = directory / ("start-agent.command" if role == "agent" else "start-ui.sh")
    secure_write(start, launcher(binary, ["serve" if role == "agent" else "run", "--config", config], saved_env), 0o700)
    installation = {"role": role, "binary": str(binary), "config": str(config), "launcher": str(start), "source": str(ROOT)}
    if role == "agent":
        installation["codex_home"] = saved_env["CODEX_HOME"]
    secure_write(directory / f"install-{role}.json", json.dumps(installation, indent=2).encode())
    check = directory / ("check-agent.command" if role == "agent" else "check-ui.sh")
    secure_write(check, launcher(binary, ["doctor" if role == "agent" else "verify", "--config", config], saved_env), 0o700)
    print("\n[4/4] Setup ready — next steps / 설치 준비 완료 · 다음 단계")
    if role == "agent":
        enable = directory / "enable-notifications.command"
        secure_write(enable, launcher(binary, ["codex-hooks", "--install", "--config", config, "--codex-binary", codex], saved_env), 0o700)
        print(f"\nInstalled / 설치됨: {binary}\n1. Start Agent / Agent 실행: {shlex.quote(str(start))}")
        print(f"2. In another terminal, enable notifications / 다른 터미널에서 알림 설치: {shlex.quote(str(enable))}")
        print("3. In your usual Codex session, open /hooks and review/trust OrangeDeck / 평소 Codex에서 /hooks 검토·신뢰")
        print(f"4. Privately transfer {directory / 'orangedeck-pairing.toml'} to the UI device; run sh install.sh there.")
        print("Pairing contains a secret. Keep it out of GitHub/chat/logs / 페어링 파일은 비밀이며 공개하면 안 됩니다.")
        print(f"Agent check / Agent 진단: {shlex.quote(str(check))}")
    else:
        desktop = directory / "OrangeDeck.desktop"
        # Desktop Exec quoting differs from shell quoting. Escape reserved field characters.
        desktop_path = str(start).replace("\\", "\\\\").replace('"', '\\"').replace("`", "\\`").replace("$", "\\$").replace("%", "%%")
        secure_write(desktop, ("[Desktop Entry]\nType=Application\nName=OrangeDeck\n"
                     f'Exec="{desktop_path}"\nIcon=applications-development\nTerminal=false\nCategories=Development;\n').encode(), 0o700)
        print(f"\nInstalled / 설치됨: {binary}\nStart / 실행: {shlex.quote(str(start))}\nDesktop shortcut / 바로가기: {desktop}")
        print(f"Connection check / 연결 검사: {shlex.quote(str(check))}")
    print("Verify a real notification, token reading and explicit approval on your devices before relying on them.")
    if options.start:
        run([start])


if __name__ == "__main__":
    try:
        main()
    except (SetupError, OSError, ValueError, EOFError) as error:
        print(f"Setup stopped / 설치 중단: {error}", file=sys.stderr)
        sys.exit(1)
    except KeyboardInterrupt:
        print("\nSetup cancelled / 설치 취소", file=sys.stderr)
        sys.exit(130)
