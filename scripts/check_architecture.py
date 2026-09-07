#!/usr/bin/env python3
"""Check inward Rust crate dependencies; Python 3.11+ developer check, no network."""
import argparse
from pathlib import Path
import sys
import tomllib

LAYERS = {
    "orangedeck-domain": ("crates/orangedeck-domain", set()),
    "orangedeck-protocol": ("crates/orangedeck-protocol", set()),
    "orangedeck-application": ("crates/orangedeck-application", {"orangedeck-domain", "orangedeck-protocol"}),
    "orangedeck-infra": ("crates/orangedeck-infra", {"orangedeck-domain", "orangedeck-protocol"}),
    "orangedeck-connector": ("apps/orangedeck-connector", {"orangedeck-domain", "orangedeck-protocol", "orangedeck-application", "orangedeck-infra"}),
    "orangedeck-ui": ("apps/orangedeck-ui", {"orangedeck-domain", "orangedeck-protocol", "orangedeck-application", "orangedeck-infra"}),
}
DATA_LIBRARIES = {"chrono", "serde", "serde_json", "thiserror", "uuid"}


def dependency_tables(manifest):
    for kind in ("dependencies", "build-dependencies"):
        yield manifest.get(kind, {})
    for target in manifest.get("target", {}).values():
        for kind in ("dependencies", "build-dependencies"):
            yield target.get(kind, {})


def check(root):
    issues = []
    for name, (directory, allowed) in LAYERS.items():
        manifest = tomllib.loads((root / directory / "Cargo.toml").read_text())
        for dependencies in dependency_tables(manifest):
            for alias, specification in dependencies.items():
                dependency = specification.get("package", alias) if isinstance(specification, dict) else alias
                if dependency.startswith("orangedeck-") and dependency not in allowed:
                    issues.append(f"{name} must not depend on {dependency}")
                elif name in {"orangedeck-domain", "orangedeck-protocol"} and dependency not in DATA_LIBRARIES:
                    issues.append(f"{name} must remain a pure data/policy crate: review {dependency}")
                if isinstance(specification, dict) and "path" in specification:
                    expected = LAYERS.get(dependency)
                    actual = (root / directory / specification["path"]).resolve()
                    if expected is None or actual != (root / expected[0]).resolve():
                        issues.append(f"{name} has an unreviewed dependency path for {dependency}")
    return issues


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    issues = check(args.root)
    for issue in issues:
        print(issue)
    print(f"Architecture: {len(LAYERS)} crates, {len(issues)} dependency violations.")
    return bool(issues)


if __name__ == "__main__":
    sys.exit(main())
