#!/usr/bin/env python3
"""Detect common secrets/private data without ever printing matching text.

This is a release guard, not a claim that heuristic scanning proves absence of secrets.
--tracked checks the Git index (including ignored-but-tracked files).
--history additionally checks every reachable Git blob; it never rewrites history.
"""
import argparse
import hashlib
import re
from pathlib import Path
import stat
import subprocess
import sys

from release_policy import MAX_PUBLIC_FILE_BYTES, REVIEWED_DIAGRAMS, REVIEWED_NOTICES, REVIEWED_SCREENSHOTS, private_path, public_path, read_public_file

PATTERNS = {
    "credential": re.compile(r"(?:\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{20,}|AKIA[A-Z0-9]{16})|-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY)"),
    "assigned secret": re.compile(r'''(?i)(?:token|password|secret|api[_-]?key)\s*[=:]\s*["'](?:[0-9a-f]{32,}|[A-Za-z0-9+/=_-]{40,})["']'''),
    "personal home path": re.compile(r"/(?:home|Users)/(?!(?:YOU|YOUR_USER|user|example|demo|test|mac|mock)(?:[/\s\"']|$))[A-Za-z0-9_.-]+"),
    "device address": re.compile(r"(?<![\d.])(?!100\.(?:64\.0\.[012]|127\.255\.254)(?![\d.]))100\.(?:6[4-9]|[7-9]\d|1[01]\d|12[0-7])\.\d{1,3}\.\d{1,3}(?![\d.])"),
    "email address": re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b"),
}
PUBLIC_EMAIL_DOMAINS = {"example.com", "example.org", "users.noreply.github.com"}


def inspect_bytes(data):
    if len(data) > MAX_PUBLIC_FILE_BYTES:
        return [(0, "oversized/unreviewed content")]
    try:
        text = data.decode("utf-8")
    except UnicodeError:
        return [(0, "binary/unreviewed content")]
    findings = []
    for number, line in enumerate(text.splitlines(), 1):
        for kind, pattern in PATTERNS.items():
            for match in pattern.finditer(line):
                if kind == "email address":
                    value = match.group().lower()
                    if value.rsplit("@", 1)[1] in PUBLIC_EMAIL_DOMAINS or value == "noreply@github.com":
                        continue
                findings.append((number, kind))
                break
    return findings


def inspect_content(path, data):
    """Binary approval is bound to both the exact path and reviewed bytes."""
    if str(path) in REVIEWED_NOTICES:
        if len(data) <= MAX_PUBLIC_FILE_BYTES and hashlib.sha256(data).hexdigest() in REVIEWED_NOTICES[str(path)]:
            return [(line, kind) for line, kind in inspect_bytes(data) if kind != "email address"]
        return [(0, "unreviewed dependency notices")]
    if str(path) in REVIEWED_DIAGRAMS:
        if len(data) <= MAX_PUBLIC_FILE_BYTES and hashlib.sha256(data).hexdigest() in REVIEWED_DIAGRAMS[str(path)]:
            return inspect_bytes(data)
        return [(0, "unreviewed diagram bytes")]
    if str(path) in REVIEWED_SCREENSHOTS:
        if len(data) <= MAX_PUBLIC_FILE_BYTES and hashlib.sha256(data).hexdigest() in REVIEWED_SCREENSHOTS[str(path)]:
            return []
        return [(0, "unreviewed screenshot bytes")]
    return inspect_bytes(data)


def git(root, *args):
    return subprocess.check_output(["git", "-C", str(root), *args], stderr=subprocess.PIPE)


def path_findings(path):
    return [(path, 0, kind + " in path") for _, kind in inspect_bytes(path.encode("utf-8"))]


def report_path(path):
    # A filename itself can contain a secret or identifying address.
    if inspect_bytes(path.encode("utf-8")):
        return "[withheld path " + hashlib.sha256(path.encode("utf-8")).hexdigest()[:12] + "]"
    return path


def inspect_blob(root, oid, path):
    size = int(git(root, "cat-file", "-s", oid))
    if size > MAX_PUBLIC_FILE_BYTES:
        return [(0, "oversized/unreviewed content")]
    return inspect_content(path, git(root, "cat-file", "blob", oid))


def scan(root, tracked=False, history=False):
    findings = []
    count = 0
    if tracked:
        entries = git(root, "ls-files", "--stage", "-z").split(b"\0")
        for entry in filter(None, entries):
            metadata, raw_path = entry.split(b"\t", 1)
            mode, oid, stage = metadata.split()
            path = raw_path.decode("utf-8", "replace")
            count += 1
            findings.extend(path_findings(path))
            if mode != b"100644" and mode != b"100755":
                findings.append((path, 0, "non-regular or unresolved index entry"))
                continue
            if stage != b"0" or not public_path(path):
                findings.append((path, 0, "non-public tracked file"))
            findings.extend((path, line, kind) for line, kind in inspect_blob(root, oid.decode(), path))
    else:
        # Walk only source roots; do not read credential/runtime/private directories.
        import os
        for directory, dirs, files in os.walk(root, followlinks=False):
            dirs[:] = sorted(d for d in dirs if not private_path(Path(directory).relative_to(root) / d))
            for name in dirs[:]:
                path = Path(directory) / name
                if path.is_symlink():
                    findings.append((path.relative_to(root).as_posix(), 0, "symlink in source directory"))
                    dirs.remove(name)
            for name in sorted(files):
                path = Path(directory) / name
                relative = path.relative_to(root).as_posix()
                if not public_path(relative):
                    continue
                count += 1
                findings.extend(path_findings(relative))
                if not stat.S_ISREG(path.lstat().st_mode):
                    findings.append((relative, 0, "non-regular public source"))
                    continue
                try:
                    data = read_public_file(path)
                except (OSError, ValueError):
                    findings.append((relative, 0, "unreadable or oversized public source"))
                    continue
                findings.extend((relative, line, kind) for line, kind in inspect_content(relative, data))
    if history:
        if git(root, "rev-parse", "--is-shallow-repository").strip() == b"true":
            raise ValueError("complete history is required")
        # rev-list --objects prints only one name per blob. Walk unique root trees
        # so a private filename cannot hide behind a public alias with the same data.
        seen_entries, blobs, checked_oids = set(), {}, set()
        trees = git(root, "rev-list", "--all", "--format=%T", "--no-commit-header").splitlines()
        for tree in sorted(set(trees)):
            for entry in filter(None, git(root, "ls-tree", "-rz", "--full-tree", tree.decode()).split(b"\0")):
                metadata, raw_path = entry.split(b"\t", 1)
                mode, object_type, oid = metadata.split()
                if entry in seen_entries:
                    continue
                seen_entries.add(entry)
                path = raw_path.decode("utf-8", "replace")
                label = f"history:{oid.decode()[:12]}:{path}"
                findings.extend((label, line, kind) for _, line, kind in path_findings(path))
                if not public_path(path, historical=True):
                    findings.append((label, 0, "non-public file in history"))
                if mode not in {b"100644", b"100755"} or object_type != b"blob":
                    findings.append((label, 0, "non-regular file in history"))
                    continue
                key = (oid, path)
                if key not in blobs:
                    blobs[key] = inspect_blob(root, oid.decode(), path)
                checked_oids.add(oid)
                findings.extend((label, line, kind) for line, kind in blobs[key])
        # Commit/tag messages and author emails are public too. Include blobs pointed
        # to directly by a tag, even when they are absent from any commit's tree.
        for entry in git(root, "rev-list", "--objects", "--all").splitlines():
            oid = entry.partition(b" ")[0]
            if oid in checked_oids:
                continue
            kind = git(root, "cat-file", "-t", oid.decode()).strip()
            if kind not in {b"commit", b"tag", b"blob"}:
                continue
            label = f"history:{oid.decode()[:12]}:{kind.decode()} metadata/content"
            size = int(git(root, "cat-file", "-s", oid.decode()))
            checks = ([(0, "oversized/unreviewed content")] if size > MAX_PUBLIC_FILE_BYTES else
                      inspect_bytes(git(root, "cat-file", kind.decode(), oid.decode())))
            findings.extend((label, line, finding) for line, finding in checks)
    return count, findings


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--tracked", action="store_true")
    parser.add_argument("--history", action="store_true")
    args = parser.parse_args()
    try:
        count, findings = scan(args.root, args.tracked, args.history)
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"Cannot finish scan: {type(error).__name__}. Check repository access and fetch complete history.", file=sys.stderr)
        return 2
    for path, line, kind in findings:
        # No source lines or matched values: logs must not become a second leak.
        print(f"{report_path(path)!r}:{line}: {kind}")
    print(f"Scanned {count} public/index files; {len(findings)} findings. Match values are withheld.")
    return 1 if findings else 0


if __name__ == "__main__":
    sys.exit(main())
