"""One public-source policy shared by packaging and pre-publication checks."""
from pathlib import PurePosixPath
import os
import stat

MAX_PUBLIC_FILE_BYTES = 8 * 1024 * 1024

# Exact, visually reviewed demo captures only. Append replacement hashes after
# review; keep previous approved hashes so complete Git history remains checked.
REVIEWED_SCREENSHOTS = {
    "docs/screenshots/live.png": {"9211958cfcc11dda3d5c84c03b04fe371c7b42a3fc07328dd117397cecb017a3"},
    "docs/screenshots/shortcuts.png": {"967bdb4ddde64586da58d4e826b9e8248b338970869536be3e82ff93f213841e"},
    "docs/screenshots/projects.png": {"01b09f744bee788e4bd6e3e7d241d42030989a5f2ee0d07a36e8670bcdbc212d"},
    "docs/screenshots/conversations.png": {"c0bc81481f39bb45685e69763156f9345a71d5ecacb4c07f6cb2fb76e4ca160c"},
    "docs/screenshots/notifications.png": {"cb1d2cb9f50959608a184d05d72ba8f7b9a34be8d69ab8316ad77e115fedf671"},
}


def read_public_file(path):
    """Read a bounded regular source file without following a final symlink or FIFO."""
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0))
    with os.fdopen(fd, "rb") as source:
        metadata = os.fstat(source.fileno())
        if not stat.S_ISREG(metadata.st_mode) or metadata.st_size > MAX_PUBLIC_FILE_BYTES:
            raise ValueError("public source must be a bounded regular file")
        data = source.read(MAX_PUBLIC_FILE_BYTES + 1)
        if len(data) > MAX_PUBLIC_FILE_BYTES:
            raise ValueError("public source exceeds the size limit")
        return data

PRIVATE_PARTS = {
    "target", "dist", ".git", ".codex", ".agents", ".cargo-home", "local",
    "__pycache__", ".venv", ".pytest_cache", ".idea", ".vscode", ".ssh", "codex-schema",
}
PRIVATE_NAMES = {"AGENTS.md", "starter.md", "setup_bridge.py", "config.local.toml", "auth.json", "credentials.json",
                 "agent.toml", "ui.toml", "hooks.json", "owned-codex-threads.json", "install-agent.json", "install-ui.json",
                 "start-agent.command", "enable-notifications.command", "check-agent.command", "start-ui.sh", "check-ui.sh",
                 "known_hosts", "authorized_keys"}
ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "README.ko.md",
    "LICENSE", "SECURITY.md", ".gitignore", ".gitattributes",
    "install.sh", "Setup OrangeDeck.command", "Start OrangeDeck Agent.command",
    "Enable Codex Notifications.command", "OrangeDeck.desktop",
}
PUBLIC_DOCS = {
    "ARCHITECTURE.md", "CONTROLS.md", "SECURITY.md", "PROTOCOL.md", "ROADMAP.md",
    "QUICKSTART_KO.md", "NOTIFICATIONS_KO.md", "MACOS_SETUP.md", "INSTALL.md",
    "CHANGELOG.md", "PUBLICATION.md", "SCREENSHOTS.md",
}


def private_path(path):
    path = PurePosixPath(path)
    if str(path) in REVIEWED_SCREENSHOTS:
        return False
    return (bool(PRIVATE_PARTS.intersection(path.parts))
            or path.name in PRIVATE_NAMES
            or path.name.startswith((".env", "HANDOFF", "hooks.json.orangedeck-backup-", "id_ed25519", "id_rsa"))
            or path.name in {"PROGRESS.md", "USAGE_KO.md"}
            or path.name.endswith((".token", ".log", ".pyc", ".local.toml", ".pem", ".key",
                                   ".p12", ".pfx", ".db", ".sqlite", ".sqlite3", ".sqlite-wal", ".sqlite-shm",
                                   ".bak", ".zip", ".tar", ".gz", ".tgz", ".7z", ".rar", ".png", ".jpg", ".jpeg", ".jsonl"))
            or ("pairing" in path.name.lower() and path.suffix.lower() in {".toml", ".json"}))


def public_path(path):
    """Allow source and reviewed documentation, never an arbitrary workspace dump."""
    raw = str(path)
    path = PurePosixPath(path)
    if (str(path) != raw or any(ord(c) < 32 or ord(c) == 127 or c in "\\:" for c in raw)
            or path.is_absolute() or ".." in path.parts or private_path(path)):
        return False
    if len(path.parts) == 1:
        return path.name in ROOT_FILES
    folder = path.parts[0]
    if folder in {"apps", "crates"}:
        return path.suffix == ".rs" or path.name == "Cargo.toml"
    if folder == "config":
        return len(path.parts) == 2 and (path.name.endswith(".example.toml") or path.name.endswith(".plist.example"))
    if folder == "docs":
        return str(path) in REVIEWED_SCREENSHOTS or (len(path.parts) == 2 and path.name in PUBLIC_DOCS)
    if folder == "scripts":
        return len(path.parts) == 2 and path.suffix in {".py", ".sh", ".zsh"}
    if folder == "download-page":
        return len(path.parts) == 2 and path.name in {
            "README.md", "index.html", "config.toml", "manage.py", "package.py", "test_manage.py",
        }
    return folder == ".github" and path.suffix in {".yml", ".yaml", ".md"}
