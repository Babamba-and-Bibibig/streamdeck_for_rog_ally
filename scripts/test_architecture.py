from pathlib import Path
import tempfile
import unittest

import check_architecture


class ArchitectureTests(unittest.TestCase):
    def test_current_workspace_respects_inward_dependencies(self):
        self.assertEqual(check_architecture.check(Path(__file__).resolve().parents[1]), [])

    def test_rejects_outer_adapters_in_inner_layers_even_under_aliases_and_targets(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for directory, _ in check_architecture.LAYERS.values():
                folder = root / directory
                folder.mkdir(parents=True)
                (folder / "Cargo.toml").write_text("[dependencies]\n")
            domain = root / "crates/orangedeck-domain/Cargo.toml"
            domain.write_text('[dependencies]\nhttp = { package = "axum", version = "0.8" }\n')
            application = root / "crates/orangedeck-application/Cargo.toml"
            application.write_text('[target.\'cfg(unix)\'.dependencies]\nadapter = { package = "orangedeck-infra", path = "../orangedeck-infra" }\n')
            issues = check_architecture.check(root)
            self.assertTrue(any("axum" in issue for issue in issues))
            self.assertTrue(any("application" in issue and "infra" in issue for issue in issues))


if __name__ == "__main__":
    unittest.main()
