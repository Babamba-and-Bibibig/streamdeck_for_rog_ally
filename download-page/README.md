# Optional private download page / 개인 다운로드 페이지

Most users should download source from GitHub and run `sh install.sh`. This Python 3.11+ standard-library tool is for maintainers who also want a private update page on their own tailnet. Reuse this tool; do not create a second packager/server.

## Configure once

Copy `config.toml` to **`config.local.toml`**, enter the serving device's Tailscale IPv4 as `bind`, choose the port, and list only your allowed client device addresses in `allow` (including the serving device). The local file is excluded from Git and public source packages. The checked-in template deliberately has no device address and cannot start a server until configured.

On your Mac, use **Safari → the address printed by `status` → Mac 업데이트 다운로드**. Then start the Connector, run `Enable Codex Notifications.command`, and review/trust OrangeDeck in Codex `/hooks`.

## Commands

From the project folder, after verified application changes and a new workspace version:

```sh
python3 download-page/manage.py update
python3 download-page/manage.py status
python3 download-page/manage.py start
python3 download-page/manage.py stop
```

`prepare` builds verified source ZIP/TAR files and updates the local catalog without starting a server. `update` also starts/reuses the managed server. Existing versions and hashes are preserved. Use a new version for new code. Stop/start/status validate the manager's instance record rather than guessing a PID. `scripts/package-source.zsh` delegates to the same packager.

For GitHub preparation, run **`python3 download-page/manage.py export`**. It uses the same checked ZIP/TAR, creates `dist/github/OrangeDeck-vVERSION/` plus a neighboring `.sha256` file listing every source file, and prints the upload candidate location. It requires no server configuration, starts no server and does not initialize Git or upload anything. Existing exports are verified and reused only if unchanged; local edits are never overwritten. Keep hidden files such as `.github/`, `.gitignore` and `.gitattributes` when transferring the source tree. See [the publication checklist](../docs/PUBLICATION.md).

Both archive formats are checked for identical file contents and permissions, including when reused. Entries are regular source files with mode 0644/0755; private paths, symlinks, special files, duplicate/ambiguous paths and unexpected metadata are rejected. Review limits are 8 MiB per file, 64 MiB total and 4096 files.

Public source files are selected by `scripts/release_policy.py` and scanned for common secret/private-data patterns before packaging. Configs, private handoffs, logs, caches and arbitrary workspace files are excluded. Archives contain source, not prebuilt Mac binaries. The catalog update does not verify Mac execution or hook trust.

`dist/download-page.json`, `dist/SHA256SUMS` and `dist/.download-page/` are local operational state. Do not publish the entire dist directory: historical archives may contain older private documentation. The HTTP server serves only the fixed page, published ZIPs, current checksum and minimal status; no upload or arbitrary file endpoints exist.

```sh
python3 download-page/test_manage.py -v
```

Tests use temporary folders and loopback. This optional server has no automatic login service.
