#!/usr/bin/env python3
"""다운로드 페이지: update 한 번으로 패키징·게시·서버 재사용."""
import argparse
from contextlib import contextmanager
import fcntl
import hashlib
import html
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import ipaddress
import json
import os
from pathlib import Path
import signal
from string import Template
import subprocess
import sys
import threading
import time
import tomllib
import urllib.error
import urllib.request
import uuid

from package import atomic_json, export_github, load_catalog, names, prepare

ROOT = Path(__file__).resolve().parents[1]
SERVICE = "orangedeck-download-page"
STATUS_PATH = "/download-page-status.json"


def settings(root):
    local = root / "download-page/config.local.toml"
    config = tomllib.loads((local if local.exists() else root / "download-page/config.toml").read_text())
    if not config.get("bind") or not config.get("allow"):
        raise ValueError("Copy config.toml to config.local.toml and enter your Tailscale bind/allow addresses.")
    network = ipaddress.ip_network("100.64.0.0/10")
    if any(ipaddress.ip_address(address) not in network for address in [config["bind"], *config["allow"]]):
        raise ValueError("다운로드 페이지 주소는 Tailscale IPv4여야 합니다.")
    if config["bind"] not in config["allow"] or not 1 <= config["port"] <= 65535:
        raise ValueError("Ally 자체 주소 허용과 포트 설정을 확인하세요.")
    return config


def url(config):
    return f"http://{config['bind']}:{config['port']}/"


def runtime(root):
    directory = root / "dist/.download-page"
    directory.mkdir(parents=True, mode=0o700, exist_ok=True)
    return directory


@contextmanager
def manager_lock(root):
    with (runtime(root) / "manager.lock").open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        yield


class Assets:
    def __init__(self, root):
        self.root = root
        self.template = Template((root / "download-page/index.html").read_text())
        self.cached = {}
        self.lock = threading.Lock()

    def asset(self, path, catalog):
        version = catalog["current"]
        filename = names(version)[0]
        if path == "/":
            page = self.template.substitute(version=html.escape(version), archive=html.escape(filename))
            return page.encode(), "text/html; charset=utf-8", None
        if path == "/SHA256SUMS":
            text = f"{catalog['releases'][version]['sha256']}  {filename}\n"
            return text.encode(), "text/plain; charset=utf-8", None
        for release, record in catalog["releases"].items():
            filename = names(release)[0]
            if path != "/" + filename:
                continue
            archive = self.root / "dist" / filename
            if archive.is_symlink():
                raise ValueError("ZIP이 일반 파일이 아닙니다.")
            stamp = archive.stat()
            key = (stamp.st_ino, stamp.st_mtime_ns, stamp.st_size, record["sha256"])
            with self.lock:
                cached = self.cached.get(filename)
                if cached is None or cached[0] != key:
                    data = archive.read_bytes()
                    if len(data) != record["size"] or hashlib.sha256(data).hexdigest() != record["sha256"]:
                        raise ValueError("배포 ZIP의 무결성 검사 실패")
                    self.cached[filename] = (key, data)
                return self.cached[filename][1], "application/zip", filename
        return None


def make_server(root, config, instance):
    assets = Assets(root)

    class Handler(BaseHTTPRequestHandler):
        def do_GET(self):
            self.respond(False)

        def do_HEAD(self):
            self.respond(True)

        def respond(self, head):
            if self.client_address[0] not in config["allow"]:
                self.send_error(403)
                return
            try:
                # One atomic catalog snapshot per request; updates need no restart.
                catalog = load_catalog(root)
                if self.path == STATUS_PATH:
                    data = json.dumps({"service": SERVICE, "pid": os.getpid(), "instance": instance,
                                       "version": catalog["current"]}).encode()
                    asset = data, "application/json", None
                else:
                    asset = assets.asset(self.path, catalog)
                if asset is None:
                    self.send_error(404)
                    return
                data, kind, filename = asset
            except (OSError, ValueError, KeyError, TypeError):
                self.send_error(503, "Release unavailable")
                return
            self.send_response(200)
            self.send_header("Content-Type", kind)
            self.send_header("Content-Length", str(len(data)))
            self.send_header("Cache-Control", "no-store")
            self.send_header("X-Content-Type-Options", "nosniff")
            self.send_header("Content-Security-Policy", "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; frame-ancestors 'none'")
            if filename:
                self.send_header("Content-Disposition", f'attachment; filename="{filename}"')
            self.end_headers()
            if not head:
                self.wfile.write(data)

        def log_message(self, *_args):
            pass

    return ThreadingHTTPServer((config["bind"], config["port"]), Handler)


def health(config):
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    try:
        with opener.open(url(config).rstrip("/") + STATUS_PATH, timeout=2) as response:
            result = json.loads(response.read(4096))
        if result.get("service") != SERVICE:
            raise ValueError("이 주소에는 다른 서버가 실행 중입니다. 자동으로 종료하지 않습니다.")
        return result
    except urllib.error.HTTPError as error:
        error.close()
        raise ValueError("이 주소에는 구형 또는 다른 서버가 실행 중입니다. PID 확인 후 한 번 전환해야 합니다.") from error
    except (urllib.error.URLError, TimeoutError):
        return None


def owned_health(root, config):
    current = health(config)
    if current is None:
        return None
    path = runtime(root) / "server.json"
    record = json.loads(path.read_text()) if path.exists() else {}
    if (record.get("pid"), record.get("instance")) != (current["pid"], current["instance"]):
        raise ValueError("이 작업 폴더에서 시작한 서버인지 확인할 수 없습니다. 자동으로 종료하지 않습니다.")
    return current


def start(root, config):
    current = owned_health(root, config)
    if current:
        print(f"다운로드 페이지 재사용: PID {current['pid']} / {current['version']} / {url(config)}")
        return
    with (runtime(root) / "server.log").open("ab") as log:
        child = subprocess.Popen([sys.executable, str(root / "download-page/manage.py"), "serve"],
                                 cwd=root, stdin=subprocess.DEVNULL, stdout=log,
                                 stderr=subprocess.STDOUT, start_new_session=True)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if child.poll() is not None:
            raise ValueError("다운로드 페이지 시작 실패. dist/.download-page/server.log를 확인하세요.")
        current = owned_health(root, config)
        if current:
            print(f"다운로드 페이지 시작: PID {current['pid']} / {current['version']} / {url(config)}")
            return
        time.sleep(0.1)
    raise ValueError("시작 확인 시간 초과. status 명령과 서버 로그를 확인하세요.")


def stop(root, config):
    current = owned_health(root, config)
    if not current:
        print("다운로드 페이지가 실행 중이지 않습니다.")
        return
    os.kill(current["pid"], signal.SIGTERM)
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if health(config) is None:
            print("다운로드 페이지를 종료했습니다.")
            return
        time.sleep(0.1)
    raise ValueError("종료 확인 시간 초과. 다른 프로세스에는 신호를 보내지 않았습니다.")


def serve(root, config):
    if load_catalog(root)["current"] is None:
        raise ValueError("먼저 update 또는 prepare 명령으로 배포 파일을 준비하세요.")
    instance = uuid.uuid4().hex
    server = make_server(root, config, instance)
    state = runtime(root) / "server.json"
    atomic_json(state, {"pid": os.getpid(), "instance": instance})

    def terminate(*_args):
        raise KeyboardInterrupt

    signal.signal(signal.SIGTERM, terminate)
    print(f"다운로드 페이지: {url(config)}", flush=True)
    try:
        server.serve_forever(poll_interval=0.2)
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
        if state.exists() and json.loads(state.read_text()).get("instance") == instance:
            state.unlink()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["update", "prepare", "export", "start", "status", "stop", "serve"])
    args = parser.parse_args()
    try:
        # Public source packaging must work before any private server is configured.
        if args.command in {"prepare", "export"}:
            with manager_lock(ROOT):
                export_github(ROOT) if args.command == "export" else prepare(ROOT)
            return
        config = settings(ROOT)
        if args.command == "serve":
            serve(ROOT, config)
            return
        with manager_lock(ROOT):
            if args.command in ("prepare", "update"):
                prepare(ROOT)
            if args.command in ("start", "update"):
                start(ROOT, config)
            elif args.command == "stop":
                stop(ROOT, config)
            elif args.command == "status":
                current = owned_health(ROOT, config)
                if current:
                    print(f"다운로드 페이지 실행 중: PID {current['pid']} / 버전 {current['version']} / {url(config)}")
                else:
                    print(f"다운로드 페이지 중지됨. 시작: python3 download-page/manage.py start / {url(config)}")
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"다운로드 페이지: {error}\n")


if __name__ == "__main__":
    main()
