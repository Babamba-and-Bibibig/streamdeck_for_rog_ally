"""다운로드 페이지의 재사용 가능한 소스 패키징과 배포 목록 관리."""
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
import tempfile
import time
import tomllib
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from release_policy import MAX_PUBLIC_FILE_BYTES, private_path, public_path, read_public_file
from check_public import inspect_bytes, inspect_content, report_path

VERSION = re.compile(r"[0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.-]+)?")
COMMANDS = ("Start OrangeDeck Connector.command", "Enable Codex Notifications.command", "Setup OrangeDeck.command")
MAX_ARCHIVE_FILES = 4096
MAX_ARCHIVE_BYTES = 64 * 1024 * 1024


def excluded(path):
    return private_path(path)


def atomic_json(path, value):
    atomic_bytes(path, (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode())


def atomic_bytes(path, data):
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary = tempfile.mkstemp(prefix=".download-page-", dir=path.parent)
    try:
        with os.fdopen(fd, "wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary, path)
    finally:
        if os.path.exists(temporary):
            os.unlink(temporary)


def workspace_version(root):
    version = tomllib.loads((root / "Cargo.toml").read_text())["workspace"]["package"]["version"]
    if not VERSION.fullmatch(version):
        raise ValueError("Cargo.toml의 배포 버전 형식을 확인하세요.")
    return version


def names(version):
    if not VERSION.fullmatch(version):
        raise ValueError("잘못된 배포 버전입니다.")
    return f"OrangeDeck-Mac-v{version}.zip", f"OrangeDeck-source-v{version}.tar.gz"


def build(root, version):
    dist = root / "dist"
    dist.mkdir(exist_ok=True)
    archive, source = (dist / name for name in names(version))
    if archive.is_symlink() or source.is_symlink():
        raise ValueError("배포 파일에 심볼릭 링크를 사용할 수 없습니다.")
    if archive.exists() or source.exists():
        if not archive.is_file() or not source.is_file():
            raise ValueError("같은 버전의 패키지가 일부만 있습니다. 기존 파일을 덮어쓰지 않습니다.")
        print(f"기존 {version} 패키지 재사용. 소스 변경을 배포하려면 Cargo.toml의 버전을 올리세요.")
        return archive, source
    with tempfile.TemporaryDirectory(prefix=".package-", dir=dist) as staging:
        staged_zip, staged_tar = (Path(staging) / name for name in names(version))
        with zipfile.ZipFile(staged_zip, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as zipped, tarfile.open(staged_tar, "w:gz") as tarred:
            for directory, dirs, files in os.walk(root):
                dirs[:] = sorted(name for name in dirs if not excluded(Path(directory).relative_to(root) / name))
                for name in sorted(dirs + files):
                    path = Path(directory) / name
                    relative = path.relative_to(root)
                    if excluded(relative):
                        continue
                    if path.is_symlink():
                        raise ValueError(f"패키지에 심볼릭 링크를 넣지 않습니다: {relative}")
                    if path.is_dir():
                        continue
                    if not public_path(relative) or relative.name == "AGENTS.md":
                        continue
                    metadata = path.stat()
                    if not stat.S_ISREG(metadata.st_mode):
                        raise ValueError(f"일반 소스 파일이 아닙니다: {relative}")
                    data = read_public_file(path)
                    if inspect_content(relative, data) or inspect_bytes(relative.as_posix().encode()):
                        raise ValueError(f"Public-source privacy check failed: {report_path(relative.as_posix())}. Run scripts/check_public.py; secret values are withheld.")
                    member = tarfile.TarInfo("streamdeck/" + relative.as_posix())
                    # Never carry local owner IDs, writable/special mode bits into a release.
                    member.size, member.mode, member.mtime = len(data), 0o755 if metadata.st_mode & 0o111 else 0o644, int(metadata.st_mtime)
                    tarred.addfile(member, io.BytesIO(data))
                    info = zipfile.ZipInfo(f"OrangeDeck-v{version}/" + member.name, time.localtime(max(metadata.st_mtime, 315532800))[:6])
                    info.create_system = 3
                    info.external_attr = (stat.S_IFREG | member.mode) << 16
                    info.compress_type = zipfile.ZIP_DEFLATED
                    zipped.writestr(info, data, compresslevel=9)
        validate_archives(staged_zip, staged_tar, version)
        staged_zip.chmod(0o600)
        staged_tar.chmod(0o600)
        # Hard links publish complete files without overwriting an existing version.
        os.link(staged_tar, source)
        os.link(staged_zip, archive)
    return archive, source


def member_path(name, prefix, seen):
    if not name.startswith(prefix):
        raise ValueError("Archive path is outside its source root")
    relative = name[len(prefix):]
    if (not public_path(relative) or PurePosixPath(relative).name == "AGENTS.md"
            or relative.casefold() in seen or inspect_bytes(relative.encode())):
        raise ValueError("Archive contains an unsafe, duplicate or non-public source path")
    seen.add(relative.casefold())
    if len(seen) > MAX_ARCHIVE_FILES:
        raise ValueError("Archive has too many source files")
    return relative


def check_member_size(size, total):
    if not 0 <= size <= MAX_PUBLIC_FILE_BYTES or total + size > MAX_ARCHIVE_BYTES:
        raise ValueError("Archive exceeds the reviewed source size limits")
    return total + size


def validate_zip(path, version):
    if path.is_symlink() or not path.is_file():
        raise ValueError("배포 ZIP은 일반 파일이어야 합니다.")
    prefix = f"OrangeDeck-v{version}/streamdeck/"
    with zipfile.ZipFile(path) as archive:
        seen, manifest, total = set(), {}, 0
        for member in archive.infolist():
            relative = member_path(member.filename, prefix, seen)
            total = check_member_size(member.file_size, total)
            mode = member.external_attr >> 16
            if (not stat.S_ISREG(mode) or stat.S_IMODE(mode) not in {0o644, 0o755}
                    or member.extra or member.comment):
                raise ValueError("ZIP contains non-regular permissions or unreviewed metadata")
            data = archive.read(member)  # Reading checks CRC after the size bound above.
            if inspect_content(relative, data):
                raise ValueError("ZIP privacy check failed; matching contents are withheld")
            manifest[relative] = (hashlib.sha256(data).hexdigest(), len(data), stat.S_IMODE(mode))
        if archive.comment:
            raise ValueError("ZIP contains an unreviewed archive comment")
        packaged = tomllib.loads(archive.read(prefix + "Cargo.toml").decode())
        if packaged["workspace"]["package"]["version"] != version:
            raise ValueError("ZIP과 Cargo.toml의 버전이 다릅니다.")
        for command in COMMANDS:
            if not (archive.getinfo(prefix + command).external_attr >> 16) & stat.S_IXUSR:
                raise ValueError("Mac command 실행 권한이 없습니다.")
        return manifest


def validate_archives(archive, source, version):
    """Validate both formats, including reused releases, against the same file manifest."""
    manifest = validate_zip(archive, version)
    if source.is_symlink() or not source.is_file():
        raise ValueError("Source TAR must be a regular file")
    seen, tar_manifest, total = set(), {}, 0
    with tarfile.open(source, "r:gz") as tarred:
        for member in tarred:
            relative = member_path(member.name, "streamdeck/", seen)
            total = check_member_size(member.size, total)
            if (not member.isfile() or member.mode not in {0o644, 0o755}
                    or member.uid or member.gid or member.uname or member.gname
                    or set(member.pax_headers) - {"path"}):
                raise ValueError("TAR contains a special file, unsafe permissions or unreviewed metadata")
            with tarred.extractfile(member) as content:
                data = content.read(MAX_PUBLIC_FILE_BYTES + 1)
            if len(data) != member.size or inspect_content(relative, data):
                raise ValueError("TAR privacy/integrity check failed; matching contents are withheld")
            tar_manifest[relative] = (hashlib.sha256(data).hexdigest(), len(data), member.mode)
    if tar_manifest != manifest:
        raise ValueError("ZIP and TAR source files, contents or permissions differ")
    return manifest


def load_catalog(root):
    path = root / "dist" / "download-page.json"
    if not path.exists():
        return {"schema_version": 1, "current": None, "releases": {}}
    value = json.loads(path.read_text())
    if value.get("schema_version") != 1 or not isinstance(value.get("releases"), dict):
        raise ValueError("다운로드 페이지 배포 목록 형식이 잘못됐습니다.")
    for version, release in value["releases"].items():
        names(version)
        if not re.fullmatch(r"[0-9a-f]{64}", release["sha256"]):
            raise ValueError("배포 목록의 해시 형식이 잘못됐습니다.")
    if value["current"] not in value["releases"]:
        raise ValueError("현재 배포 버전이 목록에 없습니다.")
    return value


def prepare(root):
    version = workspace_version(root)
    archive, source = build(root, version)
    validate_archives(archive, source, version)
    catalog = load_catalog(root)
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    record = {"sha256": digest, "size": archive.stat().st_size}
    if version in catalog["releases"] and catalog["releases"][version] != record:
        raise ValueError("이미 배포한 버전의 파일이 달라졌습니다. 새 버전을 사용하세요.")
    sums = root / "dist" / "SHA256SUMS"
    entries = dict((name, sha) for sha, name in (line.split(maxsplit=1) for line in sums.read_text().splitlines())) if sums.exists() else {}
    for path in (archive, source):
        actual = hashlib.sha256(path.read_bytes()).hexdigest()
        if path.name in entries and entries[path.name] != actual:
            raise ValueError("기록된 SHA256과 파일이 다릅니다. 기존 배포를 덮어쓰지 않습니다.")
        entries[path.name] = actual
    atomic_bytes(sums, "".join(f"{entries[name]}  {name}\n" for name in sorted(entries)).encode())
    catalog["releases"][version] = record
    catalog["current"] = version
    atomic_json(root / "dist" / "download-page.json", catalog)
    print(f"다운로드 페이지 배포 준비 완료: {version} / {record['size']} bytes / SHA256 {digest}")
    return catalog


def validate_export(directory, manifest):
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError("GitHub export must be a regular directory")
    actual = {}
    for parent, dirs, files in os.walk(directory, followlinks=False):
        for name in dirs + files:
            path = Path(parent) / name
            if path.is_symlink():
                raise ValueError("GitHub export contains a symlink")
        for name in files:
            path = Path(parent) / name
            relative = path.relative_to(directory).as_posix()
            if relative not in manifest:
                raise ValueError("GitHub export contains an unexpected file; existing exports are never overwritten")
            data = read_public_file(path)
            actual[relative] = (hashlib.sha256(data).hexdigest(), len(data), stat.S_IMODE(path.stat().st_mode))
    if actual != manifest:
        raise ValueError("GitHub export differs from the checked release; existing exports are never overwritten")


def export_github(root):
    """Make a separate, reviewed source tree; never initialize Git in a private workspace."""
    catalog = prepare(root)
    version = catalog["current"]
    archive, source = (root / "dist" / name for name in names(version))
    manifest = validate_archives(archive, source, version)
    exports = root / "dist/github"
    if exports.is_symlink():
        raise ValueError("GitHub export parent must not be a symlink")
    exports.mkdir(exist_ok=True)
    destination = exports / f"OrangeDeck-v{version}"
    if destination.exists() or destination.is_symlink():
        validate_export(destination, manifest)
    else:
        with tempfile.TemporaryDirectory(prefix=".github-export-", dir=exports) as temporary:
            staged = Path(temporary) / "source"
            staged.mkdir()
            prefix = f"OrangeDeck-v{version}/streamdeck/"
            with zipfile.ZipFile(archive) as zipped:
                # member paths/types/data have been validated; do not use extractall.
                for relative, (_, _, mode) in sorted(manifest.items()):
                    path = staged / relative
                    path.parent.mkdir(parents=True, exist_ok=True)
                    with path.open("xb") as output:
                        output.write(zipped.read(prefix + relative))
                    path.chmod(mode)
            validate_export(staged, manifest)
            if destination.exists() or destination.is_symlink():
                raise ValueError("GitHub export appeared concurrently; refusing to replace it")
            staged.rename(destination)
    manifest_path = exports / f"OrangeDeck-v{version}.sha256"
    atomic_bytes(manifest_path, "".join(f"{manifest[path][0]}  {path}\n" for path in sorted(manifest)).encode())
    print(f"GitHub 공개 후보: {destination} / {len(manifest)} files\n파일별 SHA256: {manifest_path}\n아직 GitHub에 업로드하지 않았습니다. / No GitHub upload performed.")
    return destination
