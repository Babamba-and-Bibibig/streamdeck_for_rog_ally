"""One public-source policy shared by packaging and pre-publication checks."""
from pathlib import PurePosixPath
import os
import stat

MAX_PUBLIC_FILE_BYTES = 8 * 1024 * 1024

# Exact, visually reviewed demo captures only. Append replacement hashes after
# review; keep previous approved hashes so complete Git history remains checked.
REVIEWED_SCREENSHOTS = {
    'docs/screenshots/live.png': {'d61c6549325e7cffe22a80f87a8847e98b4ef1a9412d14b862827d4d445ba27e', '298c20f063ae61cc45e2a9c2b8a70591f004cdc98494617a64a7dc655193d6a1', '9211958cfcc11dda3d5c84c03b04fe371c7b42a3fc07328dd117397cecb017a3', 'e76a4259b5dc8481727377acf358993472a24afbd7b62bfd7ab395171ded5f45', '91557220059a1c162c1b22cc93bab7d3ce88b9b07e0d040fbd34ea48050dac92'},
    'docs/screenshots/shortcuts.png': {'ea01f6f9fd47d41a68dde5ede3bcce0d62b801599071d6ab1f8dc19389126882', '967bdb4ddde64586da58d4e826b9e8248b338970869536be3e82ff93f213841e', 'ba8e06ddad93d79568c18f182feb9c0887c660b5516aa287d51c335cb536d344', 'bcd77e9aa8901d8ed96dab6986dd17246cb3a5e34b6d7c49c83cb1d22c0834e6', 'ce66a7e229092e8f8d5bbd108f2ac0447d136ba235de5fba60cd73c6fc03c126', '85299709696c32a125be73a062d4b9faf6aaccc594fd8abc01c9eb8136c2e4f6'},
    'docs/screenshots/projects.png': {'162d64f8687bc218a745343065ce12eb0f245bc54db7d2882cdfb0df3fdf543d', '01b09f744bee788e4bd6e3e7d241d42030989a5f2ee0d07a36e8670bcdbc212d', '1076c70bc5beb2e9478fd3c0e0607ce024a91b2ea616b9bb55840fcf4d8fbe8d', '5646d60100bfa759d8e6180826962bc27f54d6ad964c0da4999c2ae85e0a0ed9', 'd6e17c211edccad50de8a3a4ee2412f8adceedccc06a3d6af40ec02a2912c673'},
    'docs/screenshots/conversations.png': {'5987b3fb5cfd7c6a4d6033aa148cbc9d9cb2d47f2d0310927779ebda2efe47c3', '09eb04a92993a01275bdbc8519eee70700d9b3d91ef495daa2adf61341e319d4', '9e7587beffe9bb8c13116fecc324308b8269fd355695f9c895953420f7070b56', 'c0bc81481f39bb45685e69763156f9345a71d5ecacb4c07f6cb2fb76e4ca160c', '8a140a31f0e489aea19a4b670a94451283e8025c599fb151dae3ce77aa2dbfc8'},
    'docs/screenshots/notifications.png': {'0667ef6b96c2ef094e824b64a19a08355a6acfdd20411df9d74aee678bfa230c', '795442fdfaadf84dcac112d679126fd6da24d03470643b8d27b73657aacf2f87', 'cb1d2cb9f50959608a184d05d72ba8f7b9a34be8d69ab8316ad77e115fedf671'},
    'docs/screenshots/key-editor.png': {'3b06307de26c88028e107c941ffbf0ddce789fb8ad53b4ba272361cc646f2974', 'e2eeb8dd7102912172d47d5bdd953ee481afadb2d1c5def5597ebfa885fe9d8c', 'fff375ecdce5350e5d3c86c8168e6b0e4316d0e107597a1b47111546ffbb9242', 'b2690925ae4d0d5a01c19bcac058ff0320345b737e59ae6fb12fa7058f1b4fa6'},
    'docs/screenshots/en-live.png': {'c7e032e9d9f0ad46349aa2239786db218ffb9d7d5c206434700dd5537f8d4b81', '08ad191a8ac0c5094d992f5bdc221e330997d089d57edd7bd3a2d13143b9e45d', 'dd80cecd82627356ad9dbbb477e3a40b815c53ac54f91833be2ae625d62c1f14', '427f54497ac821bf11dfab9616045f255d0acaf66469c28e19f27d88fa53c5af'},
    'docs/screenshots/en-shortcuts.png': {'155c363772158d46c041c3e45942e988c82f95f10c09cc9b8649c32972505c47', '86969848f4714c903fd3b3a03e44b86ce9834dc70c515a11a11653448ed73173', 'f3a842236c7c321b3cab20844c62cdd91c4a2e3b79e6a1c90e3beb4eb25424f4', '461a745010b1c131ad318db938c1ec94f42650552b826d048fe7e18cec76bd61', 'af02db318eff486eaf1c1470e61fad5540c7cb17c5dca5b9db378811a892c6ae'},
    'docs/screenshots/en-projects.png': {'277361efddc9b849f82d1e817964b774997006999a2438c924ae55f90449f98b', '20c47d325d26d0f450c426283ccc689ecf018a6cf2ecf7b2888e10033a1e28fc', '5b06ce7189a48f15272cd52668057f612a2ea62bb3f3c5a4d9198980cf396bbd', '9e0e4d4c3efd8ceb42942e866c27da4c803154dce501410761fc793e64517cd2'},
    'docs/screenshots/en-conversations.png': {'bd8b7a6c88398b0c21833d3a3ab37311c3494768b7b942aad9fe7cd082b855df', '8a34d6b4cd4a5c48b3a2aa7d79f1afb5f97e8137fd3410e7e11d9adc23728092', 'cd9f57b119447a90938894355020931e764feddb0c373444532fee3a92f76716', '07f39722681d0ead16cc42df811c60bae1fa75342e25fb5bef33a1f328dcc79a'},
    'docs/screenshots/en-notifications.png': {'f24e4e8891fda3da144d4cc0a9efffc17ff7e8ebee043b9721db684d64995ce8', 'fedbf1ca3c75166f426224c766504703d30991edc1c3810d59422c128d4f8a63'},
    'docs/screenshots/en-key-editor.png': {'d836610703be42e2366f53b4258ca59bd17c6725180967d38c8007889df19da3', '5e78db2fb5a0cdf37c2eaf03b7993494332e2ab4a6c58bce3df66f67a5cf74cc', 'b4588fb8298aa822836f4fce2f7fd5e0159b2a4b86a74ba955990877be90a9fe', '288a9e66904d0c9fb022cac9609f5a7f943ad30e0b6d9634504274d22bd99223'},
    'docs/screenshots/response.png': {'8f75c479788f16cd3a5d382a0e9c8a4dfc7c5ebc7297b0e46c3590cd0349b76b', '4a2efc86e099e3190d4634084d5f2fb72f69e4f192dc613346f537007f9c8288', 'f461fc3d567642475deaa401ff654f1233f9042f5f132e9ecec257f40c3d9928'},
    'docs/screenshots/files.png': {'34ed3c30ae938ea2d9ebb2c379d663aa4f33c82e9aa0c41f963019ab6f50292e', 'd4b61baa6bddc39040411a244dfd778f0ccb7ece204d8ce1e3b48d3fbc26631b', '76ef8a4cc05fd59b75a9905733f3111e9f118d09daad6b488bd9bcaf9f95e97f', 'ac128ef2da8ffc1e7a1477634b7aaf47252eb46d3cce18707d9b1546e4738f1c'},
    'docs/screenshots/en-response.png': {'93067849d6247e0ba76afc79a5ab9a66c10cdc718f0e41c90592b5c46de4cf15', '21b30fdd32f39b0695c1f39eec1bd3d715e2cb353265904386f36da82fe6d74d', '47691cf1c9340a22362b65e245d9ed9bb7c11416f5d29864dbb7dc60a7f6fdb8'},
    'docs/screenshots/en-files.png': {'be2db6a9a8f57f4ef9bde3c81310cc2976fe9e30d888aacd04d0b9247b44ea4b', '868cb9ba6ef3c3020dfb8c630b51b8b0f12ad552f8616642a729dfdceab53105', '8ab2fc8f2d90f29d8e1343ba06abd8c99278c95e47cfbc743f904f2a7c99a1b0', 'ac7ccff34b5f28ba2f33963993bf7825c5d6dbc5d8e40d04c36fad0d1228b43f'},
}

# Retired notification captures stay hash-checked in old Git history only.
CURRENT_SCREENSHOTS = frozenset(REVIEWED_SCREENSHOTS) - {
    "docs/screenshots/notifications.png", "docs/screenshots/en-notifications.png",
}

# Static, hand-drawn SVGs: reviewed for layout, private data and active content.
# Preserve earlier hashes when a reviewed diagram is replaced.
REVIEWED_DIAGRAMS = {
    'docs/diagrams/device-roles-ko.svg': {'45745816058257029e30b6c181f8b18a5dd18aa5a9f799735222157fd8947810', '90b0dd486128b14d0c56b7b765601e96a090cb56cbab101292e41a42bef30d2d', '33d379cc93054667d94b145f575efc2551d7ad13c957ccdfb94d6d480ea039e6'},
    'docs/diagrams/device-roles-en.svg': {'04342216d152ba87f507334bd3dca4c7d6f045f44c1ca345f5211b6b851350d5', '9e1472e7dcfcc7633e72cac47783d482ef9143ebb68dba5fd48eeda40c9ccaa6', '77a26d2f40c5a68aee49a77b7fc6d729c5510ab24003a043081942ea918e47f4'},
}


# Exact upstream license texts reviewed against Cargo.lock archives and pinned
# upstream sources. Only their required attribution email addresses are exempt.
REVIEWED_NOTICES = {
    "THIRD_PARTY_NOTICES.md": {"f658c2a6c0df533c386673a404af480a2acbd9ae16c5a180c84ca4ff24e779dc", "94b79addbbee8f70e5e6e0cfa9814310b5fbecb33cc75809a6fb530ca27c58ba"},
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
    "__pycache__", ".venv", ".pytest_cache", ".idea", ".vscode", ".ssh", "codex-schema", "download-page",
}
PRIVATE_NAMES = {"AGENTS.md", "starter.md", "setup_bridge.py", "config.local.toml", "auth.json", "credentials.json",
                 "agent.toml", "connector.toml", "ui.toml", "ui-preferences.toml", "editor-projects.toml", "hooks.json", "owned-codex-threads.json", "install-agent.json", "install-connector.json", "install-ui.json",
                 "start-agent.command", "start-connector.command", "enable-notifications.command", "check-agent.command", "check-connector.command", "start-ui.sh", "check-ui.sh",
                 "known_hosts", "authorized_keys"}
ROOT_FILES = {
    "Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "README.md", "README.ko.md", "README.en.md",
    "LICENSE", "THIRD_PARTY_NOTICES.md", ".gitignore", ".gitattributes",
    "install.sh", "Setup OrangeDeck.command", "Start OrangeDeck Agent.command", "Start OrangeDeck Connector.command",
    "Enable Codex Notifications.command", "OrangeDeck.desktop",
}
PUBLIC_DOCS = {
    "CONTROLS.md",
    "QUICKSTART_KO.md", "NOTIFICATIONS_KO.md", "MACOS_SETUP.md", "INSTALL.md", "INSTALL.en.md",
    "CHANGELOG.md", "SCREENSHOTS.md",
}

PUBLIC_SCRIPTS = {
    "install.py", "build-macos-connector.zsh", "run-ally.zsh", "run-mac.zsh",
    "check_public.py", "release_policy.py",
}

# Previously published generic tools/docs remain content-scanned in old commits.
# They are excluded from the current index, source exports and new archives.
RETIRED_PUBLIC_PATHS = {
    "SECURITY.md", "docs/SECURITY.md", "docs/ARCHITECTURE.md", "docs/PROTOCOL.md",
    "docs/ROADMAP.md", "docs/PUBLICATION.md", "scripts/package-source.zsh",
    "scripts/serve-update.py", "download-page/README.md", "download-page/index.html",
    "download-page/config.toml", "download-page/manage.py", "download-page/package.py",
    "download-page/test_manage.py",
    "scripts/test_install.py", "scripts/test_architecture.py", "scripts/test_public.py",
    "scripts/check_architecture.py", "scripts/run-demo.zsh",
    "scripts/build-macos-agent.zsh",
    "apps/orangedeck-ui/src/app/tests.rs", "apps/orangedeck-ui/src/test_support.rs",
    "crates/orangedeck-infra/src/file_capture/tests.rs",
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


def public_path(path, *, historical=False):
    """Allow runtime source, installation tools and reviewed user documentation."""
    raw = str(path)
    path = PurePosixPath(path)
    if (str(path) != raw or any(ord(c) < 32 or ord(c) == 127 or c in "\\:" for c in raw)
            or path.is_absolute() or ".." in path.parts):
        return False
    if raw in RETIRED_PUBLIC_PATHS:
        return historical
    if private_path(path):
        return False
    if len(path.parts) == 1:
        return path.name in ROOT_FILES
    folder = path.parts[0]
    if folder in {"apps", "crates"}:
        if (set(path.parts) & {"tests", "benches", "fixtures"}
                or path.stem in {"tests", "test_support"}
                or path.stem.startswith("test_") or path.stem.endswith("_tests")):
            return False
        return path.suffix == ".rs" or path.name == "Cargo.toml"
    if folder == "config":
        return len(path.parts) == 2 and (path.name.endswith(".example.toml") or path.name.endswith(".plist.example"))
    if folder == "docs":
        return (str(path) in REVIEWED_SCREENSHOTS or str(path) in REVIEWED_DIAGRAMS
                or (len(path.parts) == 2 and path.name in PUBLIC_DOCS))
    if folder == "scripts":
        return len(path.parts) == 2 and path.name in PUBLIC_SCRIPTS
    return folder == ".github" and path.suffix in {".yml", ".yaml", ".md"}
