"""다운로드 페이지: 패키지 보존과 실행 중 배포 갱신 검사."""
import contextlib
import hashlib
import io
import json
from pathlib import Path
import shutil
import tempfile
import tarfile
import threading
import unittest
from unittest.mock import patch
import urllib.error
import urllib.request
import zipfile

import manage
import package
from release_policy import REVIEWED_SCREENSHOTS


class DownloadPageTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="orangedeck-download-test-", dir="/tmp")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / "download-page").mkdir()
        shutil.copy(Path(__file__).with_name("index.html"), self.root / "download-page/index.html")
        for command in package.COMMANDS:
            path = self.root / command
            path.write_text("#!/bin/zsh\nexit 0\n")
            path.chmod(0o755)
        self.version("0.1.7")

    def version(self, value):
        (self.root / "Cargo.toml").write_text(f'[workspace.package]\nversion = "{value}"\n')

    def prepare(self):
        with contextlib.redirect_stdout(io.StringIO()):
            return package.prepare(self.root)

    def server(self, allow=None):
        config = {"bind": "127.0.0.1", "port": 0, "allow": allow if allow is not None else ["127.0.0.1"]}
        server = manage.make_server(self.root, config, "test-instance")
        thread = threading.Thread(target=server.serve_forever, kwargs={"poll_interval": 0.01}, daemon=True)
        thread.start()
        self.addCleanup(thread.join, 2)
        self.addCleanup(server.server_close)
        self.addCleanup(server.shutdown)
        return f"http://127.0.0.1:{server.server_port}"

    def get(self, base, path, method="GET"):
        request = urllib.request.Request(base + path, method=method)
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        return opener.open(request, timeout=3)

    def test_packaging_excludes_private_files_and_preserves_modes(self):
        for relative in ["target/private", ".codex/credentials.json", ".agents/private", ".cargo-home/private", "sample.token", "test-pairing.toml", "setup_bridge.py", ".env", "debug.log", "config/local/connector.toml", "download-page/config.local.toml", "starter.md", "docs/HANDOFF_015.md", "personal-notes.txt"]:
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("exclude me")
        self.prepare()
        with zipfile.ZipFile(self.root / "dist/OrangeDeck-Mac-v0.1.7.zip") as archive:
            self.assertEqual(len(archive.infolist()), 2 + len(package.COMMANDS))
            self.assertIsNone(archive.testzip())
            for command in package.COMMANDS:
                info = archive.getinfo("OrangeDeck-v0.1.7/streamdeck/" + command)
                self.assertTrue((info.external_attr >> 16) & 0o100)

    def test_same_version_is_reused_and_published_bytes_cannot_change(self):
        self.prepare()
        path = self.root / "dist/OrangeDeck-Mac-v0.1.7.zip"
        before = path.read_bytes()
        (self.root / "new-source.txt").write_text("not silently added to a published version")
        self.prepare()
        self.assertEqual(path.read_bytes(), before)
        with zipfile.ZipFile(path, "a") as archive:
            archive.writestr("OrangeDeck-v0.1.7/streamdeck/unexpected.txt", "changed")
        with self.assertRaises(ValueError):
            self.prepare()

    def test_reviewed_images_survive_export_but_modified_images_are_rejected(self):
        workspace = Path(__file__).resolve().parents[1]
        for relative in REVIEWED_SCREENSHOTS:
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(workspace / relative, target)
        (self.root / "docs/screenshots/personal.png").write_bytes(b"private fixture")
        self.prepare()
        archive, source = (self.root / "dist" / name for name in package.names("0.1.7"))
        manifest = package.validate_archives(archive, source, "0.1.7")
        self.assertNotIn("docs/screenshots/personal.png", manifest)
        for relative in REVIEWED_SCREENSHOTS:
            self.assertIn(relative, manifest)
            self.assertEqual(manifest[relative][0], hashlib.sha256((workspace / relative).read_bytes()).hexdigest())
        self.version("0.1.8")
        (self.root / "docs/screenshots/live.png").write_bytes(b"replacement, not reviewed")
        with self.assertRaises(ValueError):
            self.prepare()

    def test_reused_tar_must_match_zip_even_without_previous_checksums(self):
        for extra in ["starter.md", "README.md", "../escaped.py"]:
            with self.subTest(extra=extra):
                self.prepare()
                source = self.root / "dist/OrangeDeck-source-v0.1.7.tar.gz"
                original = source.read_bytes()
                with tarfile.open(fileobj=io.BytesIO(original), mode="r:gz") as old, tarfile.open(source, "w:gz") as changed:
                    for member in old:
                        changed.addfile(member, old.extractfile(member))
                    entry = tarfile.TarInfo("streamdeck/" + extra)
                    entry.size = len(b"unexpected data")
                    changed.addfile(entry, io.BytesIO(b"unexpected data"))
                sums = self.root / "dist/SHA256SUMS"
                recorded_sums = sums.read_bytes()
                sums.unlink()
                try:
                    with self.assertRaises(ValueError):
                        self.prepare()
                    self.assertFalse(sums.exists(), "failed validation must not create a trusted checksum")
                finally:
                    source.write_bytes(original)
                    sums.write_bytes(recorded_sums)

    def test_zip_rejects_special_modes_and_noncanonical_paths(self):
        self.prepare()
        path = self.root / "dist/OrangeDeck-Mac-v0.1.7.zip"
        original = path.read_bytes()
        for filename, mode in [("README.md", 0o010644), ("README.md", 0o104755),
                               ("scripts//extra.py", 0o100644), ("scripts/./extra.py", 0o100644),
                               ("scripts/back\\slash.py", 0o100644)]:
            with self.subTest(filename=filename, mode=mode):
                path.write_bytes(original)
                with zipfile.ZipFile(path, "a") as archive:
                    member = zipfile.ZipInfo("OrangeDeck-v0.1.7/streamdeck/" + filename)
                    member.create_system = 3
                    member.external_attr = mode << 16
                    archive.writestr(member, "# example")
                with self.assertRaises(ValueError):
                    package.validate_zip(path, "0.1.7")
        path.write_bytes(original)

    def test_server_switches_release_without_restart_and_keeps_old_links(self):
        self.prepare()
        base = self.server()
        old_path = "/OrangeDeck-Mac-v0.1.7.zip"
        with self.get(base, old_path) as response:
            old_bytes = response.read()
        self.version("0.1.8")
        self.prepare()
        with self.get(base, "/") as response:
            self.assertIn(b"OrangeDeck-Mac-v0.1.8.zip", response.read())
        with self.get(base, old_path) as response:
            self.assertEqual(response.read(), old_bytes)
        with self.get(base, "/OrangeDeck-Mac-v0.1.8.zip") as response:
            new_bytes = response.read()
            self.assertIn("0.1.8", response.headers["Content-Disposition"])
        with self.get(base, "/SHA256SUMS") as response:
            self.assertIn(hashlib.sha256(new_bytes).hexdigest().encode(), response.read())
        with self.get(base, manage.STATUS_PATH) as response:
            self.assertEqual(json.load(response)["version"], "0.1.8")
        with self.get(base, "/", "HEAD") as response:
            self.assertEqual(response.read(), b"")

    def test_unpublished_paths_uploads_and_other_clients_are_rejected(self):
        self.prepare()
        base = self.server()
        for path, method, expected in [("/../Cargo.toml", "GET", 404), ("/download-page.json", "GET", 404), ("/", "POST", 501)]:
            with self.assertRaises(urllib.error.HTTPError) as error:
                self.get(base, path, method)
            self.assertEqual(error.exception.code, expected)
            error.exception.close()

        forbidden = self.server(allow=[])
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.get(forbidden, "/")
        self.assertEqual(error.exception.code, 403)
        error.exception.close()

    def test_source_secrets_block_new_packages_without_printing_the_secret(self):
        secret = "ghp_" + "z" * 36
        (self.root / "README.md").write_text("credential: " + secret)
        with self.assertRaises(ValueError) as error:
            self.prepare()
        self.assertNotIn(secret, str(error.exception))
        self.assertFalse((self.root / "dist/OrangeDeck-Mac-v0.1.7.zip").exists())

    def test_private_server_settings_override_the_public_template(self):
        (self.root / "download-page/config.toml").write_text('bind = ""\nport = 45833\nallow = []\n')
        with self.assertRaises(ValueError):
            manage.settings(self.root)
        (self.root / "download-page/config.local.toml").write_text('bind = "100.64.0.1"\nport = 45833\nallow = ["100.64.0.1", "100.64.0.2"]\n')
        self.assertEqual(manage.settings(self.root)["bind"], "100.64.0.1")

    def test_prepare_entrypoint_needs_no_private_server_configuration(self):
        with patch.object(manage, "ROOT", self.root), patch("sys.argv", ["manage.py", "prepare"]), \
                contextlib.redirect_stdout(io.StringIO()):
            manage.main()
        self.assertTrue((self.root / "dist/OrangeDeck-Mac-v0.1.7.zip").is_file())

    def test_github_export_is_separate_complete_and_never_overwrites_changes(self):
        (self.root / "starter.md").write_text("private local history")
        (self.root / ".github/workflows").mkdir(parents=True)
        (self.root / ".github/workflows/checks.yml").write_text("name: Example\n")
        with patch.object(manage, "ROOT", self.root), patch("sys.argv", ["manage.py", "export"]), \
                contextlib.redirect_stdout(io.StringIO()):
            manage.main()
        exported = self.root / "dist/github/OrangeDeck-v0.1.7"
        self.assertTrue((exported / "Cargo.toml").is_file())
        self.assertTrue((exported / ".github/workflows/checks.yml").is_file())
        self.assertFalse((exported / "starter.md").exists())
        self.assertFalse((self.root / ".git").exists())
        manifest = self.root / "dist/github/OrangeDeck-v0.1.7.sha256"
        self.assertIn(".github/workflows/checks.yml", manifest.read_text())
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(package.export_github(self.root), exported)
        config = exported / "Cargo.toml"
        config.write_text("local edit")
        with contextlib.redirect_stdout(io.StringIO()), self.assertRaises(ValueError):
            package.export_github(self.root)
        self.assertEqual(config.read_text(), "local edit")

    def test_changed_archive_and_invalid_catalog_are_rejected(self):
        self.prepare()
        base = self.server()
        path = "/OrangeDeck-Mac-v0.1.7.zip"
        with self.get(base, path) as response:
            response.read()
        (self.root / "dist" / path[1:]).write_bytes(b"changed")
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.get(base, path)
        self.assertEqual(error.exception.code, 503)
        error.exception.close()
        (self.root / "dist/download-page.json").write_text("{")
        with self.assertRaises(urllib.error.HTTPError) as error:
            self.get(base, "/")
        self.assertEqual(error.exception.code, 503)
        error.exception.close()


if __name__ == "__main__":
    unittest.main()
