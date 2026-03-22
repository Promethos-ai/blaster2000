# Ember Development Guide

Engineering notes for building and troubleshooting the Ember project.

---

## Before recompiling and relaunching

**Always** kill existing processes and remove file locks before rebuilding or restarting. This avoids "Blocking waiting for file lock", "user-mapped section open", and transport errors from stale connections.

### 1. Kill existing processes

Stop all servers and build tools that may hold locks:

```powershell
# PowerShell
Get-Process -Name "cargo","rustc","ember-server","grpc_server","pinggy","pinggy-win" -ErrorAction SilentlyContinue | Stop-Process -Force
```

```powershell
# Alternative (taskkill)
taskkill /F /IM cargo.exe 2>$null
taskkill /F /IM rustc.exe 2>$null
taskkill /F /IM ember-server.exe 2>$null
taskkill /F /IM grpc_server.exe 2>$null
taskkill /F /IM pinggy.exe 2>$null
taskkill /F /IM pinggy-win.exe 2>$null
```

Wait a few seconds, then proceed.

### 2. Remove file locks

Cargo creates `.cargo-lock` files under `target/` to prevent concurrent builds. Remove them:

```powershell
# PowerShell (ember + Feb17)
Get-ChildItem -Path "d:\rust" -Directory -Filter "target" -Recurse -Depth 5 -ErrorAction SilentlyContinue | ForEach-Object {
    Get-ChildItem -Path $_.FullName -Filter ".cargo-lock" -Recurse -ErrorAction SilentlyContinue -Force | Remove-Item -Force
}
```

```powershell
# Single project
Get-ChildItem -Path "target" -Recurse -Force -Filter ".cargo-lock" -ErrorAction SilentlyContinue | Remove-Item -Force
```

```bash
# Bash
find target -name ".cargo-lock" -delete
```

### 3. Then rebuild and relaunch

After steps 1 and 2, run your build/start scripts. For a full clean rebuild:

```powershell
.\clean-and-start.ps1
```

---

## Removing file locks (troubleshooting)

When Cargo or Gradle hangs with "Blocking waiting for file lock on artifact directory", remove the locks and stop any processes holding them (see above).

### Gradle locks (Android)

If Gradle builds hang:

```powershell
# PowerShell
Get-ChildItem -Path "android\.gradle" -Recurse -Force -Filter "*.lock" -ErrorAction SilentlyContinue | Remove-Item -Force
```

```bash
# Bash
find android/.gradle -name "*.lock" -delete
```

### Git index lock

If Git operations fail with "index.lock":

```powershell
Remove-Item ".git\index.lock" -Force -ErrorAction SilentlyContinue
```

```bash
rm -f .git/index.lock
```

### What NOT to remove

- **`Cargo.lock`** (project root) — Dependency lock file; keep it for reproducible builds.

---

## Build Commands

Before any build or run, [kill processes and remove file locks](#before-recompiling-and-relaunching).

| Task | Command |
|------|---------|
| Build server + client | `cargo build -p ember-server -p ember-client` |
| Build Android APK | `.\build-android.ps1` (Windows) or `./build-android.sh` (Linux/macOS) |
| Build Flutter APK | `.\build-flutter.ps1` (requires Flutter SDK) |
| Sign APK | `.\sign-apk.ps1` |
| Run server | `cargo run -p ember-server` |
| Run server (TCP inference) | `cargo run -p ember-server -- --inference http://127.0.0.1:50051` |
| Run server (custom params file) | `cargo run -p ember-server -- --params-file my_config.json` |
| Test client | `cargo run -p ember-client -- 127.0.0.1:4433 "What is 2+2?"` |

---

## Inference parameters (fine-tuning)

The ember-server reads `inference_params.json` on **every request**. Edit the file between messages to tune output without restarting.

**Location:** `ember/inference_params.json` (or `--params-file PATH`)

**Fields:** `n_predict`, `temp`, `top_k`, `top_p`, `penalty_last_n`, `penalty_repeat`, `mirostat_tau`, `mirostat_eta`

See [README.md](../README.md#fine-tuning-inference-parameters) for the full parameter table.

---

## See Also

- [README.md](../README.md) — Quick start and architecture
- [PORT-FORWARDING-AND-TUNNEL-SETUP.md](PORT-FORWARDING-AND-TUNNEL-SETUP.md) — Exposing ember-server
- **Pinggy** (primary): Run `pinggy.bat` for remote access. Android uses the URL shown.
- [PGROK-EAGLEONE-README.md](PGROK-EAGLEONE-README.md) — Alternative: self-hosted tunnel on eagleoneonline.ca
