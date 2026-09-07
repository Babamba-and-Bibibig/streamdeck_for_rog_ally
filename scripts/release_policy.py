"""One public-source policy shared by packaging and pre-publication checks."""
from pathlib import PurePosixPath
import os
import stat

MAX_PUBLIC_FILE_BYTES = 8 * 1024 * 1024

# Exact, visually reviewed demo captures only. Append replacement hashes after
# review; keep previous approved hashes so complete Git history remains checked.
REVIEWED_SCREENSHOTS = {
    'docs/screenshots/live.png': {'298c20f063ae61cc45e2a9c2b8a70591f004cdc98494617a64a7dc655193d6a1', '9211958cfcc11dda3d5c84c03b04fe371c7b42a3fc07328dd117397cecb017a3', 'e76a4259b5dc8481727377acf358993472a24afbd7b62bfd7ab395171ded5f45'},
    'docs/screenshots/shortcuts.png': {'967bdb4ddde64586da58d4e826b9e8248b338970869536be3e82ff93f213841e', 'ba8e06ddad93d79568c18f182feb9c0887c660b5516aa287d51c335cb536d344', 'bcd77e9aa8901d8ed96dab6986dd17246cb3a5e34b6d7c49c83cb1d22c0834e6'},
    'docs/screenshots/projects.png': {'01b09f744bee788e4bd6e3e7d241d42030989a5f2ee0d07a36e8670bcdbc212d', '1076c70bc5beb2e9478fd3c0e0607ce024a91b2ea616b9bb55840fcf4d8fbe8d', '5646d60100bfa759d8e6180826962bc27f54d6ad964c0da4999c2ae85e0a0ed9'},
    'docs/screenshots/conversations.png': {'09eb04a92993a01275bdbc8519eee70700d9b3d91ef495daa2adf61341e319d4', '9e7587beffe9bb8c13116fecc324308b8269fd355695f9c895953420f7070b56', 'c0bc81481f39bb45685e69763156f9345a71d5ecacb4c07f6cb2fb76e4ca160c'},
    'docs/screenshots/notifications.png': {'0667ef6b96c2ef094e824b64a19a08355a6acfdd20411df9d74aee678bfa230c', '795442fdfaadf84dcac112d679126fd6da24d03470643b8d27b73657aacf2f87', 'cb1d2cb9f50959608a184d05d72ba8f7b9a34be8d69ab8316ad77e115fedf671'},
    'docs/screenshots/key-editor.png': {'e2eeb8dd7102912172d47d5bdd953ee481afadb2d1c5def5597ebfa885fe9d8c', 'fff375ecdce5350e5d3c86c8168e6b0e4316d0e107597a1b47111546ffbb9242'},
    'docs/screenshots/en-live.png': {'08ad191a8ac0c5094d992f5bdc221e330997d089d57edd7bd3a2d13143b9e45d', 'dd80cecd82627356ad9dbbb477e3a40b815c53ac54f91833be2ae625d62c1f14'},
    'docs/screenshots/en-shortcuts.png': {'86969848f4714c903fd3b3a03e44b86ce9834dc70c515a11a11653448ed73173', 'f3a842236c7c321b3cab20844c62cdd91c4a2e3b79e6a1c90e3beb4eb25424f4'},
    'docs/screenshots/en-projects.png': {'20c47d325d26d0f450c426283ccc689ecf018a6cf2ecf7b2888e10033a1e28fc', '5b06ce7189a48f15272cd52668057f612a2ea62bb3f3c5a4d9198980cf396bbd'},
    'docs/screenshots/en-conversations.png': {'8a34d6b4cd4a5c48b3a2aa7d79f1afb5f97e8137fd3410e7e11d9adc23728092', 'cd9f57b119447a90938894355020931e764feddb0c373444532fee3a92f76716'},
    'docs/screenshots/en-notifications.png': {'f24e4e8891fda3da144d4cc0a9efffc17ff7e8ebee043b9721db684d64995ce8', 'fedbf1ca3c75166f426224c766504703d30991edc1c3810d59422c128d4f8a63'},
    'docs/screenshots/en-key-editor.png': {'5e78db2fb5a0cdf37c2eaf03b7993494332e2ab4a6c58bce3df66f67a5cf74cc', 'b4588fb8298aa822836f4fce2f7fd5e0159b2a4b86a74ba955990877be90a9fe'},
    'docs/screenshots/response.png': {'4a2efc86e099e3190d4634084d5f2fb72f69e4f192dc613346f537007f9c8288'},
    'docs/screenshots/files.png': {'d4b61baa6bddc39040411a244dfd778f0ccb7ece204d8ce1e3b48d3fbc26631b'},
    'docs/screenshots/en-response.png': {'21b30fdd32f39b0695c1f39eec1bd3d715e2cb353265904386f36da82fe6d74d'},
    'docs/screenshots/en-files.png': {'868cb9ba6ef3c3020dfb8c630b51b8b0f12ad552f8616642a729dfdceab53105'},
}

# Static, hand-drawn SVGs: reviewed for layout, private data and active content.
# Preserve earlier hashes when a reviewed diagram is replaced.
REVIEWED_DIAGRAMS = {
    'docs/diagrams/device-roles-ko.svg': {'45745816058257029e30b6c181f8b18a5dd18aa5a9f799735222157fd8947810', '90b0dd486128b14d0c56b7b765601e96a090cb56cbab101292e41a42bef30d2d'},
    'docs/diagrams/device-roles-en.svg': {'04342216d152ba87f507334bd3dca4c7d6f045f44c1ca345f5211b6b851350d5', '9e1472e7dcfcc7633e72cac47783d482ef9143ebb68dba5fd48eeda40c9ccaa6'},
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
                 "agent.toml", "connector.toml", "ui.toml", "ui-preferences.toml", "hooks.json", "owned-codex-threads.json", "install-agent.json", "install-connector.json", "install-ui.json",
                 "start-agent.command", "start-connector.command", "enable-notifications.command", "check-agent.command", "check-connector.command", "start-ui.sh", "check-ui.sh",
                 "known_hosts", "authorized_keys"}
ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "README.ko.md", "README.en.md",
    "LICENSE", "SECURITY.md", ".gitignore", ".gitattributes",
    "install.sh", "Setup OrangeDeck.command", "Start OrangeDeck Agent.command", "Start OrangeDeck Connector.command",
    "Enable Codex Notifications.command", "OrangeDeck.desktop",
}
PUBLIC_DOCS = {
    "ARCHITECTURE.md", "CONTROLS.md", "SECURITY.md", "PROTOCOL.md", "ROADMAP.md",
    "QUICKSTART_KO.md", "NOTIFICATIONS_KO.md", "MACOS_SETUP.md", "INSTALL.md", "INSTALL.en.md",
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
        return (str(path) in REVIEWED_SCREENSHOTS or str(path) in REVIEWED_DIAGRAMS
                or (len(path.parts) == 2 and path.name in PUBLIC_DOCS))
    if folder == "scripts":
        return len(path.parts) == 2 and path.suffix in {".py", ".sh", ".zsh"}
    if folder == "download-page":
        return len(path.parts) == 2 and path.name in {
            "README.md", "index.html", "config.toml", "manage.py", "package.py", "test_manage.py",
        }
    return folder == ".github" and path.suffix in {".yml", ".yaml", ".md"}
