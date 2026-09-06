from pathlib import Path
import subprocess
import tempfile
import unittest
import os
import shutil

import check_public
from release_policy import REVIEWED_SCREENSHOTS, public_path


class PublicSourceTests(unittest.TestCase):
    def test_gitignore_stages_public_source_and_excludes_private_state_by_default(self):
        workspace = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            subprocess.run(["git", "init", "-q", tmp], check=True)
            shutil.copy(workspace / ".gitignore", root / ".gitignore")
            public = ["README.md", "Cargo.lock", "apps/orangedeck-agent/src/paths.rs", "crates/orangedeck-domain/Cargo.toml",
                      "config/agent.example.toml", "scripts/install.py", "download-page/config.toml", ".github/workflows/checks.yml",
                      "docs/SCREENSHOTS.md", *REVIEWED_SCREENSHOTS]
            private = ["AGENTS.md", "starter.md", "notes.md", "screenshot.png", "agent.toml", "auth.json", "received-Pairing.json",
                       "config/local/github-ssh/id_ed25519", "download-page/config.local.toml", "scripts/start-ui.sh",
                       "apps/example/local/private.rs", "crates/example/.codex/private.rs", "scripts/credentials.json",
                       "docs/HANDOFF_latest.md", "dist/private.zip", "new-folder/personal.txt",
                       "docs/screenshots/private.png", "docs/screenshots/local/live.png", "docs/screenshots/live.jpg"]
            for relative in public + private:
                path = root / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text("fixture")
            subprocess.run(["git", "-C", tmp, "add", "."], check=True)
            staged = set(subprocess.check_output(["git", "-C", tmp, "ls-files", "-z"]).decode().strip("\0").split("\0"))
            self.assertEqual(staged, set(public) | {".gitignore"})
            for relative in private:
                self.assertFalse(public_path(relative), relative)

    def test_private_paths_are_never_public_even_under_a_source_directory(self):
        for path in ["starter.md", "config/local/agent.toml", "download-page/config.local.toml",
                     "crates/x/.env", "scripts/credentials.json", "docs/HANDOFF_015.md",
                     "docs/PROGRESS.md", "notes.txt", "dist/old.zip", "apps/x/pairing.toml"]:
            self.assertFalse(public_path(path), path)
        for path in ["README.ko.md", "apps/x/src/main.rs", "config/agent.example.toml"]:
            self.assertTrue(public_path(path), path)

    def test_patterns_report_type_and_line_without_disclosing_the_secret(self):
        value = "ghp_" + "z" * 36
        findings = check_public.inspect_bytes(("first line\ncredential: " + value).encode())
        self.assertEqual(findings, [(2, "credential")])
        self.assertNotIn(value, repr(findings))

    def test_only_reviewed_screenshot_bytes_at_the_reviewed_path_are_allowed(self):
        workspace = Path(__file__).resolve().parents[1]
        for path in REVIEWED_SCREENSHOTS:
            with self.subTest(path=path):
                data = (workspace / path).read_bytes()
                self.assertEqual(check_public.inspect_content(path, data), [])
                self.assertTrue(check_public.inspect_content(path, data + b"private metadata"))
                self.assertTrue(check_public.inspect_content(path, b"replacement picture"))
                self.assertTrue(check_public.inspect_content("README.md", data))
                self.assertTrue(check_public.inspect_content("docs/screenshots/private.png", data))

    def test_screenshot_history_checks_aliases_and_rejected_bytes_after_removal(self):
        workspace = Path(__file__).resolve().parents[1]
        relative = "docs/screenshots/live.png"
        with tempfile.TemporaryDirectory(prefix="orangedeck-image-history-") as tmp:
            root = Path(tmp)
            def git(*args):
                return subprocess.check_output(["git", "-C", tmp, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.com")
            image = root / relative
            image.parent.mkdir(parents=True)
            data = (workspace / relative).read_bytes()
            image.write_bytes(data)
            git("add", ".")
            git("commit", "-qm", "reviewed demo image")
            self.assertEqual(check_public.scan(root, tracked=True, history=True)[1], [])
            # A valid image hash cannot make the same blob safe under a text path.
            (root / "README.md").write_bytes(data)
            git("add", ".")
            git("commit", "-qm", "image alias fixture")
            image.write_bytes(data + b"unreviewed metadata")
            git("add", ".")
            git("commit", "-qm", "changed image fixture")
            image.write_bytes(data)
            git("rm", "README.md")
            git("add", ".")
            git("commit", "-qm", "restore reviewed content")
            self.assertEqual(check_public.scan(root, tracked=True)[1], [])
            findings = check_public.scan(root, tracked=True, history=True)[1]
            self.assertTrue(any("README.md" in path and kind == "binary/unreviewed content" for path, _, kind in findings))
            self.assertTrue(any(kind == "unreviewed screenshot bytes" for _, _, kind in findings))

    def test_git_source_archive_preserves_only_reviewed_screenshot_paths(self):
        import io
        import tarfile
        workspace = Path(__file__).resolve().parents[1]
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            def git(*args):
                return subprocess.check_output(["git", "-C", tmp, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.com")
            shutil.copy(workspace / ".gitattributes", root / ".gitattributes")
            for relative in REVIEWED_SCREENSHOTS:
                image = root / relative
                image.parent.mkdir(parents=True, exist_ok=True)
                shutil.copy(workspace / relative, image)
            (root / "private.png").write_bytes(b"unreviewed fixture")
            git("add", ".")
            git("commit", "-qm", "archive fixture")
            with tarfile.open(fileobj=io.BytesIO(git("archive", "HEAD"))) as archive:
                members = archive.getnames()
                self.assertNotIn("private.png", members)
                for relative in REVIEWED_SCREENSHOTS:
                    self.assertEqual(archive.extractfile(relative).read(), (workspace / relative).read_bytes())

    def test_device_addresses_allow_only_named_examples_and_range_notation(self):
        for value in ["100.64.0.0/10", "100.64.0.1", "100.64.0.2", "100.127.255.254"]:
            self.assertEqual(check_public.inspect_bytes(value.encode()), [])
        for subnet in [64, 127, 90]:
            value = f"100.{subnet}.8.9"
            self.assertEqual(check_public.inspect_bytes(value.encode()), [(1, "device address")])

    def test_index_and_history_detect_ignored_already_tracked_files(self):
        with tempfile.TemporaryDirectory(prefix="orangedeck-public-test-") as tmp:
            root = Path(tmp)
            def git(*args):
                return subprocess.check_output(["git", "-C", tmp, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.com")
            (root / "starter.md").write_text("private handoff")
            git("add", "starter.md")
            git("commit", "-qm", "fixture")
            (root / ".gitignore").write_text("starter.md\n")
            _, findings = check_public.scan(root, tracked=True)
            self.assertTrue(any(kind == "non-public tracked file" for _, _, kind in findings))
            git("rm", "--cached", "starter.md")
            git("add", ".gitignore")
            git("commit", "-qm", "ignore private handoff")
            _, latest = check_public.scan(root, tracked=True)
            self.assertEqual(latest, [])
            _, history = check_public.scan(root, tracked=True, history=True)
            self.assertTrue(any(kind == "non-public file in history" for _, _, kind in history))

    def test_history_checks_each_path_even_when_files_share_one_blob(self):
        with tempfile.TemporaryDirectory(prefix="orangedeck-history-alias-") as tmp:
            root = Path(tmp)
            def git(*args):
                return subprocess.check_output(["git", "-C", tmp, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("config", "user.name", "Test")
            git("config", "user.email", "test@example.com")
            (root / "README.md").write_text("same content")
            (root / "starter.md").write_text("same content")
            git("add", "README.md", "starter.md")
            git("commit", "-qm", "fixture")
            git("rm", "starter.md")
            git("commit", "-qm", "remove private alias")
            _, findings = check_public.scan(root, tracked=True, history=True)
            self.assertTrue(any("starter.md" in path and kind == "non-public file in history"
                                for path, _, kind in findings))

    def test_filename_secrets_are_detected_and_report_paths_are_redacted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "scripts").mkdir()
            secret = "ghp_" + "z" * 36
            (root / "scripts" / (secret + ".py")).write_text("# example")
            _, findings = check_public.scan(root)
            self.assertTrue(any(kind == "credential in path" for _, _, kind in findings))
            for path, _, _ in findings:
                self.assertNotIn(secret, check_public.report_path(path))

    def test_workspace_refuses_symlink_directories_and_special_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            (root / "scripts").mkdir()
            os.mkfifo(root / "scripts" / "fifo.py")
            (root / "apps").symlink_to(root / "scripts", target_is_directory=True)
            _, findings = check_public.scan(root)
            self.assertTrue(any(kind == "non-regular public source" for _, _, kind in findings))
            self.assertTrue(any(kind == "symlink in source directory" for _, _, kind in findings))

    def test_history_checks_commit_and_tag_metadata_without_printing_values(self):
        with tempfile.TemporaryDirectory() as tmp:
            def git(*args):
                return subprocess.check_output(["git", "-C", tmp, *args], stderr=subprocess.DEVNULL)
            git("init", "-q")
            git("config", "user.name", "Test")
            email = "private-owner" + "@" + "mail.invalid"
            git("config", "user.email", email)
            (Path(tmp) / "README.md").write_text("public content")
            git("add", "README.md")
            git("commit", "-qm", "fixture")
            secret = "ghp_" + "z" * 36
            git("tag", "-a", "fixture-tag", "-m", secret)
            _, findings = check_public.scan(Path(tmp), tracked=True, history=True)
            self.assertTrue(any("commit metadata" in path and kind == "email address" for path, _, kind in findings))
            self.assertTrue(any("tag metadata" in path and kind == "credential" for path, _, kind in findings))
            self.assertNotIn(email, repr(findings))
            self.assertNotIn(secret, repr(findings))

    def test_email_example_exceptions_match_complete_domains(self):
        for value in ["test@example.com", "test@users.noreply.github.com", "noreply@github.com"]:
            self.assertEqual(check_public.inspect_bytes(value.encode()), [])
        value = "test@" + "example.com.extra.invalid"
        self.assertEqual(check_public.inspect_bytes(value.encode()), [(1, "email address")])


if __name__ == "__main__":
    unittest.main()
