import contextlib
import io
import json
import os
from pathlib import Path
import stat
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import install


class InstallerTests(unittest.TestCase):
    def test_ui_with_build_tools_still_requires_korean_font_and_respects_consent(self):
        options = install.parser().parse_args([])
        with patch.object(install.platform, "system", return_value="Linux"), \
                patch.object(install, "executable", side_effect=lambda name: "/usr/bin/" + name if name in {"cc", "pkg-config", "apt-get"} else None), \
                patch.object(install, "has_korean_font", return_value=False), \
                patch.object(install.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)), \
                patch.object(install, "consent", return_value=False) as consent, \
                patch.object(install, "run") as run, contextlib.redirect_stdout(io.StringIO()):
            with self.assertRaises(install.SetupError):
                install.install_dependencies("ui", options)
            run.assert_not_called()
            consent.return_value = True
            install.install_dependencies("ui", options)
            self.assertIn("fonts-noto-cjk", run.call_args.args[0])
            run.reset_mock()
            consent.reset_mock()
            install.install_dependencies("agent", options)
            consent.assert_not_called()
            run.assert_not_called()

    def test_launcher_quotes_paths_and_values_without_shell_execution(self):
        with tempfile.TemporaryDirectory(prefix="orangedeck-launcher-test-") as tmp:
            root = Path(tmp)
            output = root / "output.json"
            # Dangerous-looking characters are literal path/argument data.
            program = root / "a 'quoted' $name `echo no`"
            program.write_text("#!/usr/bin/env python3\nimport sys,json,os\nopen(sys.argv[1],'w').write(json.dumps([sys.argv[2:],os.environ['ORANGEDECK_TEST_VALUE']]))\n")
            program.chmod(0o700)
            payload = "$(touch SHOULD_NOT_EXIST); 'hello' `echo bad`"
            launcher = root / "start.sh"
            install.secure_write(launcher, install.launcher(program, [output, payload], {"ORANGEDECK_TEST_VALUE": payload}), 0o700)
            subprocess.run([launcher], cwd=root, check=True)
            self.assertEqual(json.loads(output.read_text()), [[payload], payload])
            self.assertFalse((root / "SHOULD_NOT_EXIST").exists())

    def test_atomic_private_write_and_symlink_rejection(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            target = root / "private/config"
            install.secure_write(target, b"first")
            self.assertEqual(stat.S_IMODE(target.stat().st_mode), 0o600)
            install.secure_write(target, b"second")
            self.assertEqual(target.read_bytes(), b"second")
            link = root / "link"
            link.symlink_to(target)
            with self.assertRaises(install.SetupError):
                install.secure_write(link, b"overwrite")
            self.assertEqual(target.read_bytes(), b"second")

    def test_pairing_permissions_size_and_symlinks(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundle = root / "bundle.toml"
            bundle.write_text("synthetic")
            bundle.chmod(0o644)
            self.assertEqual(install.private_pairing(bundle), bundle)
            self.assertEqual(stat.S_IMODE(bundle.stat().st_mode), 0o600)
            link = root / "symlink"
            link.symlink_to(bundle)
            with self.assertRaises(install.SetupError):
                install.private_pairing(link)
            bundle.write_bytes(b"x" * 16_385)
            with self.assertRaises(install.SetupError):
                install.private_pairing(bundle)

    def test_check_does_not_install_or_contact_accounts(self):
        with patch("sys.argv", ["install.py", "--check"]), patch.object(install, "run") as run, \
                contextlib.redirect_stdout(io.StringIO()):
            install.main()
        run.assert_not_called()

    def test_existing_pairing_is_not_silently_replaced(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = Path(tmp) / "config.toml"
            config.write_text("existing settings")
            args = ["install.py", "--role", "ui", "--config-dir", tmp, "--pairing", "new.toml", "--non-interactive"]
            with patch("sys.argv", args), patch.object(install.platform, "system", return_value="Linux"), \
                    patch.object(install, "run") as run, self.assertRaises(install.SetupError):
                install.main()
            run.assert_not_called()
            self.assertEqual(config.read_text(), "existing settings")

    def test_incomplete_setup_preserves_credentials_before_any_installation(self):
        for role, filename in [("agent", "agent.token"), ("agent", "orangedeck-pairing.toml"), ("ui", "ui.token")]:
            with tempfile.TemporaryDirectory() as tmp:
                credential = Path(tmp) / filename
                credential.write_bytes(b"existing credential")
                args = ["install.py", "--role", role, "--config-dir", tmp, "--non-interactive"]
                with patch("sys.argv", args), patch.object(install, "run") as run, self.assertRaises(install.SetupError):
                    install.main()
                run.assert_not_called()
                self.assertEqual(credential.read_bytes(), b"existing credential")

    def test_noninteractive_install_permissions_are_explicit(self):
        with patch("builtins.input") as prompt:
            self.assertFalse(install.consent("install", False, True))
            self.assertTrue(install.consent("install", True, True))
        prompt.assert_not_called()

    def test_control_characters_in_settings_are_rejected(self):
        for value in ["", "bad\npath", "bad\x00path", None, 42, {}]:
            with self.assertRaises(install.SetupError):
                install.checked_text(value)

    def test_dragged_paths_and_invalid_input_retry_without_evaluation(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder = Path(tmp) / "a folder 'with quotes' $literal"
            folder.mkdir()
            for pasted in [str(folder), install.shlex.quote(str(folder)), str(folder).replace(" ", "\\ ").replace("'", "\\'")]:
                self.assertEqual(install.existing_path(pasted), folder)
            with patch("builtins.input", side_effect=["/does-not-exist", install.shlex.quote(str(folder))]), \
                    contextlib.redirect_stdout(io.StringIO()):
                self.assertEqual(install.ask_path("Folder", None, False, directory=True), folder)

    def test_update_reads_saved_profile_without_running_old_launcher(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            profile = root / "Codex's private profile"
            old = root / "start-agent.command"
            old.write_bytes(install.launcher(Path("/unused"), [], {"CODEX_HOME": str(profile)}))
            with old.open("a") as output:
                output.write("touch SHOULD_NOT_EXIST\n")
            self.assertEqual(install.saved_codex_home(root), str(profile))
            metadata = root / "install-agent.json"
            metadata.write_text(json.dumps({"codex_home": str(root / "new profile")}))
            self.assertEqual(install.saved_codex_home(root), str(root / "new profile"))
            for invalid in [[], {"codex_home": 12}, {"codex_home": "bad\npath"}]:
                metadata.write_text(json.dumps(invalid))
                with self.assertRaises(install.SetupError):
                    install.saved_codex_home(root)
            metadata.unlink()
            metadata.symlink_to(old)
            with self.assertRaises(install.SetupError):
                install.saved_codex_home(root)
            self.assertFalse((root / "SHOULD_NOT_EXIST").exists())

    def test_environment_probe_timeout_does_not_echo_captured_secrets(self):
        error = subprocess.TimeoutExpired(["codex"], 1, output="private-account-value")
        with patch.object(install.subprocess, "run", side_effect=error), self.assertRaises(install.SetupError) as result:
            install.run(["codex", "login", "status"], capture=True, timeout=1)
        self.assertNotIn("private-account-value", str(result.exception))


@unittest.skipUnless(os.environ.get("ORANGEDECK_TEST_BIN_DIR"), "opt in with a directory containing freshly built Agent and UI")
class NativeInstallerTests(unittest.TestCase):
    def test_native_setup_refuses_orphaned_tokens(self):
        binary_dir = Path(os.environ["ORANGEDECK_TEST_BIN_DIR"]).resolve()
        with tempfile.TemporaryDirectory(prefix="orangedeck-orphan-test-") as tmp:
            root = Path(tmp)
            tailscale = root / "test-tailscale"
            tailscale.write_text("#!/bin/sh\nprintf '100.64.0.2\\n'\n")
            tailscale.chmod(0o700)
            env = os.environ.copy()
            env["ORANGEDECK_TAILSCALE_BINARY"] = str(tailscale)
            bundle = root / "received.toml"
            token = "test-only-" * 8
            bundle.write_text(f'protocol_version = 1\nagent_url = "http://100.64.0.2:45831"\nhost_label = "TEST"\ntoken = "{token}"\n')
            bundle.chmod(0o600)
            for role, config, operation in [("agent", "agent.toml", ["init", "--project-path", str(root)]),
                                             ("ui", "config.toml", ["pair", "--bundle", str(bundle)])]:
                directory = root / role
                directory.mkdir()
                credential = directory / (role + ".token")
                credential.write_text("existing credential")
                result = subprocess.run([binary_dir / ("orangedeck-" + role), *operation, "--config", directory / config],
                                        env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
                self.assertNotEqual(result.returncode, 0)
                self.assertEqual(credential.read_text(), "existing credential")
                self.assertFalse((directory / config).exists())

    def test_fresh_install_pairs_native_binaries_and_update_preserves_credentials(self):
        binary_dir = Path(os.environ["ORANGEDECK_TEST_BIN_DIR"]).resolve()
        self.assertTrue((binary_dir / "orangedeck-agent").is_file())
        self.assertTrue((binary_dir / "orangedeck-ui").is_file())
        original_run = install.run
        with tempfile.TemporaryDirectory(prefix="orangedeck-native-setup-") as tmp:
            root = Path(tmp)
            project = root / "project with spaces 'quoted'"
            project.mkdir()
            tailscale = root / "test tailscale"
            tailscale.write_text("#!/bin/sh\nprintf '100.64.0.2\\n'\n")
            tailscale.chmod(0o700)
            codex = root / "test codex"
            codex.write_text("#!/bin/sh\nexit 0\n")
            codex.chmod(0o700)
            fake_cargo = root / "cargo"
            agent_dir = root / "Agent's private settings"
            ui_dir = root / "Display's private settings"
            profile = root / "Codex's test profile"
            profile.mkdir()
            (profile / "hooks.json").write_text(json.dumps({"hooks": {}, "test_preserved": True}))
            observed_profiles = []

            def runner(args, **kwargs):
                if Path(args[0]) == fake_cargo:
                    if "metadata" in args:
                        return json.dumps({"target_directory": str(binary_dir.parent)})
                    return None
                if Path(args[0]) == codex and list(args[1:]) == ["login", "status"]:
                    observed_profiles.append(kwargs["env"]["CODEX_HOME"])
                kwargs["capture"] = True
                return original_run(args, **kwargs)

            def setup(arguments):
                with patch("sys.argv", ["install.py", "--non-interactive", *arguments]), \
                        patch.object(install, "install_dependencies"), \
                        patch.object(install, "cargo_tool", return_value=fake_cargo), \
                        patch.object(install, "tailscale_binary", return_value=tailscale), \
                        patch.object(install, "run", side_effect=runner), \
                        contextlib.redirect_stdout(io.StringIO()):
                    install.main()

            agent_args = ["--role", "agent", "--config-dir", str(agent_dir), "--project-path", str(project),
                          "--host-name", "TEST HOST", "--codex-binary", str(codex)]
            with patch.dict(os.environ, {"CODEX_HOME": str(profile)}):
                setup(agent_args)
            config_before = (agent_dir / "agent.toml").read_bytes()
            token_before = (agent_dir / "agent.token").read_bytes()
            self.assertGreaterEqual(len(token_before), 32)
            received = root / "received.toml"
            received.write_bytes((agent_dir / "orangedeck-pairing.toml").read_bytes())
            received.chmod(0o644)
            setup(["--role", "ui", "--config-dir", str(ui_dir), "--pairing", str(received)])
            self.assertEqual((ui_dir / "ui.token").read_bytes(), token_before)
            self.assertIn("100.64.0.2:45831", (ui_dir / "config.toml").read_text())
            self.assertEqual(stat.S_IMODE(received.stat().st_mode), 0o600)
            # A fresh shell has no CODEX_HOME or --codex-binary override. Updates must
            # preserve both selections from the original installation/configuration.
            clean_env = {key: value for key, value in os.environ.items() if key != "CODEX_HOME"}
            with patch.dict(os.environ, clean_env, clear=True):
                setup(["--role", "agent", "--config-dir", str(agent_dir)])
            self.assertEqual((agent_dir / "agent.toml").read_bytes(), config_before)
            self.assertEqual((agent_dir / "agent.token").read_bytes(), token_before)
            self.assertEqual(observed_profiles, [str(profile), str(profile)])
            for folder, launcher in [(agent_dir, "start-agent.command"), (ui_dir, "start-ui.sh"),
                                     (agent_dir, "check-agent.command"), (ui_dir, "check-ui.sh")]:
                self.assertEqual(stat.S_IMODE((folder / launcher).stat().st_mode), 0o700)
                subprocess.run(["sh", "-n", folder / launcher], check=True)
            self.assertIn(str(codex).replace("'", "'\"'\"'"), (agent_dir / "enable-notifications.command").read_text())
            # Execute only the generated hook installer with an isolated saved profile.
            subprocess.run([agent_dir / "enable-notifications.command"], check=True, capture_output=True)
            hooks = json.loads((profile / "hooks.json").read_text())
            self.assertTrue(hooks["test_preserved"])
            command = hooks["hooks"]["PermissionRequest"][0]["hooks"][0]["command"]
            fields = install.shlex.split(command)
            self.assertEqual(Path(fields[fields.index("--socket") + 1]), agent_dir.resolve() / "hooks/codex.sock")
            self.assertTrue(list(profile.glob("hooks.json.orangedeck-backup-*")))
            # Validate the candidate before replacing an installed executable.
            installed = agent_dir / "bin/orangedeck-agent"
            install.secure_write(installed, b"previous-binary-sentinel", 0o700)
            (agent_dir / "agent.toml").write_text("invalid = [")
            with self.assertRaises(install.SetupError):
                setup(["--role", "agent", "--config-dir", str(agent_dir)])
            self.assertEqual(installed.read_bytes(), b"previous-binary-sentinel")
            self.assertEqual((agent_dir / "agent.token").read_bytes(), token_before)


if __name__ == "__main__":
    unittest.main()
