#!/usr/bin/env python3
"""다운로드 페이지의 이전 진입점. 구현과 사용법: download-page/README.md."""
from pathlib import Path
import runpy
import sys

directory = Path(__file__).resolve().parents[1] / "download-page"
sys.path.insert(0, str(directory))
if len(sys.argv) == 1:
    sys.argv.append("serve")
runpy.run_path(str(directory / "manage.py"), run_name="__main__")
