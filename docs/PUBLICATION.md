# Public-source checks and GitHub publication

The shared source policy is `scripts/release_policy.py`. Only reviewed source/config examples/documentation enter a new package. Personal session handoffs, runtime settings, tokens, pairing bundles, logs, databases, caches and arbitrary workspace files do not.

`.gitignore` also excludes files by default and explicitly allows reviewed source extensions and named documents. New document/asset types require a deliberate policy update. Local SSH keys/known-host files, generated installation metadata/launchers and `AGENTS.md` stay private, including when placed under a source directory. Ignore rules do not remove already-tracked files; the index/history scanner is a separate required check.

## What goes to GitHub

| Include at the repository root | Purpose |
| --- | --- |
| `apps/`, `crates/`, `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml` | Application source and reproducible dependency/toolchain selection |
| `install.sh`, the three `.command` launchers, `OrangeDeck.desktop`, `scripts/` | Guided installation, launchers and local release checks |
| `README.md`, `README.ko.md`, `README.en.md`, `docs/`, `LICENSE` | Korean-first and English setup, features, controls and publication information |
| `config/*.example.toml`, `config/*.plist.example` | Templates for each user's own environment |
| `.github/`, `.gitignore`, `.gitattributes` | CI checks and publication exclusions; keep these hidden files |
| Reviewed `download-page/` source and empty `config.toml` template | Optional private update tool, with no machine settings |

The export contains only the allow-listed members within these directories. Do not upload `starter.md`, handoff/progress notes, `config/local/`, `config.local.toml`, tokens/pairing files, Codex authentication, logs, personal screenshots, databases, build outputs or the historical `dist/` directory. Local contributor instructions are excluded from packaged/exported source.

The image exceptions are the twelve reviewed Korean/English demo screenshots and two static device-role SVG diagrams documented in [SCREENSHOTS.md](SCREENSHOTS.md). Their exact paths and SHA256 hashes are pinned in `scripts/release_policy.py`, and the index/history scanner and ZIP/TAR validator use the same content check. Replacing an image or adding private metadata fails verification until it has been reviewed and its hash explicitly approved. Diagrams also pass the text privacy scan; their reviewed SVGs contain no script, external resource or embedded image. Git export attributes preserve these assets in GitHub source downloads; other images remain excluded.

<a id="development"></a>

## Development checks and demo

The user guide targets macOS Connector and a CachyOS Handheld ROG Ally remote. Implementation paths for other systems do not establish successful installation on those systems. Steam Deck/SteamOS remain untested.

```sh
cargo test --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all --check
python3 scripts/check_architecture.py
python3 -m unittest discover -s scripts -p 'test_*.py'
python3 download-page/test_manage.py
cargo build --release --workspace --locked
./scripts/run-demo.zsh
# English UI and example conversations:
./scripts/run-demo.zsh en
```

Demo uses SIMULATED MAC / LOCAL MOCK data. The recorded Codex CLI schema baseline is 0.153.2. Demo and build checks do not verify real Mac installation, notifications or approvals.

## Prepare a separate upload candidate

After verifying changes and assigning a new workspace version:

```sh
python3 scripts/check_public.py
python3 download-page/manage.py export
```

The existing packager validates **both** source archives and writes:

- `dist/github/OrangeDeck-vVERSION/`: the exact public source tree. Its contents belong at the GitHub repository root.
- `dist/github/OrangeDeck-vVERSION.sha256`: every source file's path and SHA-256, for review.
- `dist/OrangeDeck-Mac-vVERSION.zip` and `dist/OrangeDeck-source-vVERSION.tar.gz`: checked source release assets, if you choose to attach them to a release.

This command starts no server and performs no Git initialization, commit or upload. Existing version archives and changed exports are never overwritten. Old archives have not been retroactively cleaned. Publishing the source tree does not require attaching any ZIP.

Use the exported tree in a separate checkout. For an existing repository, inspect its remote history/assets before copying in updated source; a clean export does not clean that history. Review the actual staged file list and diff, run the checks below **before each push**, and commit only the intended changes. Keep your original private working folder outside that Git checkout.

## Before publishing

```sh
python3 scripts/check_public.py
python3 scripts/check_public.py --tracked --history
```

The first command checks candidate public files. The second checks the Git index (including files already tracked despite `.gitignore`), every path in reachable commit trees, reachable blob contents and commit/tag metadata. It requires full history and refuses shallow clones. Identical content under multiple historical filenames is checked at every path. Reports print object IDs, line numbers and finding types; matching source is withheld, and sensitive filenames are redacted too. File reads are bounded and reject non-regular files. These are heuristic checks: inspect unrecognized data and use an independent secret scanner when reviewing an existing public repository.

The scanner flags ordinary email addresses in author/commit/tag data; example domains and GitHub no-reply addresses are exempt. Review author names and other personal identifiers yourself, and choose your own GitHub no-reply address before committing. Check all branches/tags, releases, old ZIPs, screenshots, issues, pull requests and Actions logs. Local source scanning cannot establish what a remote repository has already published. GitHub auto-generated ZIPs depend on the tracked tree/export attributes; `.gitignore` alone is insufficient.

GitHub Actions is a second check **after upload**, so it cannot prevent that first exposure. Run the local checks first; review GitHub secret scanning/push protection and private vulnerability reporting settings when configuring the repository. The checked-in workflows use read-only repository permissions, disable persisted checkout credentials and pin downloaded tools/actions. No workflow publishes a release automatically.

Do not upload the whole working folder or the whole historical `dist/` directory. Build a **new version** with `python3 download-page/manage.py prepare` to obtain checked source ZIP/TAR files. Previously issued archives are immutable local records and are not retroactively sanitized. Never attach a private pairing file or diagnostic log to an issue.

## If something was already exposed

1. Revoke/rotate an exposed secret at its issuer. For an OrangeDeck token, stop the Connector, regenerate credentials with reviewed project settings and re-pair trusted clients. Merely deleting a GitHub file does not invalidate a credential.
2. Review the exact affected paths/commits/assets before changing remote history. A corrective commit removes a file only from the current tree. History rewriting, force pushes, removing release assets or changing repository visibility need a deliberate owner decision.
3. Coordinate cleanup of other clones, forks, cached views and published artifacts. Git history cannot prove that nobody copied an exposed value.

References: [GitHub: removing sensitive data](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/removing-sensitive-data-from-a-repository), [GitHub: commit email settings](https://docs.github.com/en/account-and-profile/how-tos/email-preferences/setting-your-commit-email-address).

## 한국어

**작업 폴더 전체를 GitHub에 올리지 않습니다.** `python3 download-page/manage.py export`가 만드는 `dist/github/OrangeDeck-v버전/` 안의 검사된 파일만 공개 후보로 사용합니다. 그 안의 README·소스·설치기·예제 설정·문서·검사 워크플로가 저장소 첫 화면에 바로 오도록 배치합니다. `.github`, `.gitignore`, `.gitattributes` 같은 숨김 파일도 포함합니다. 옆의 `.sha256` 파일은 전체 파일 목록과 각 내용의 해시입니다.

실제 설정, 토큰과 페어링, Codex 인증, 대화·사용 기록, 캡처, `starter.md`와 개인 인수인계, 빌드 결과, 과거 압축은 제외합니다. GitHub Releases에 첨부할 경우에도 이번에 검사한 버전의 ZIP/TAR만 선택하며, 첨부 자체는 필수가 아닙니다. 기존 압축을 새 검사 통과본으로 간주하지 않습니다.

공개 후보 생성은 서버를 켜거나 GitHub에 올리는 명령이 아닙니다. 원본 작업 폴더는 개인용으로 유지하고 별도 Git 체크아웃에서 올릴 변경을 검토합니다. **푸시 전에** `python3 scripts/check_public.py --tracked --history`와 독립 비밀값 검사를 실행합니다. GitHub Actions는 이미 업로드된 다음 실행되므로 최초 유출 방지 수단으로만 믿으면 안 됩니다. 커밋 작성자 이메일은 본인의 GitHub no-reply 주소를 사용하고 표시 이름도 확인하세요.

개인 환경은 `config/local/`, `download-page/config.local.toml` 또는 사용자 설정 폴더에 두세요. 공개 README·예제 설정에는 실제 주소·계정 경로·사용 기록을 넣지 않습니다. 검사기는 일치한 비밀값 원문을 출력하지 않습니다.

이미 공개된 파일은 최신 커밋에서 삭제하거나 `.gitignore`에 넣는 것만으로 과거 기록에서 사라지지 않습니다. 먼저 실제 비밀값을 폐기·교체하고, 저장소의 브랜치·태그·릴리스·로그 등을 확인한 다음 소유자가 검토한 방식으로 기록을 정리해야 합니다. 검사 결과가 없다는 사실은 모든 개인정보·비밀값이 없다는 보장이 아닙니다.
