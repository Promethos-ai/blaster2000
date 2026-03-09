# Ember Development Guide

Engineering notes for building and troubleshooting the Ember project.

---

## Removing File Locks

When Cargo or Gradle hangs with "Blocking waiting for file lock on artifact directory", remove the locks and stop any processes holding them.

### 1. Stop Cargo processes (Windows)

```powershell
taskkill /F /IM cargo.exe 2>$null
taskkill /F /IM rustc.exe 2>$null
```

### 2. Remove Cargo build locks

Cargo creates `.cargo-lock` files under `target/` to prevent concurrent builds. Remove them:

```powershell
# PowerShell
Get-ChildItem -Path "target" -Recurse -Force -Filter ".cargo-lock" -ErrorAction SilentlyContinue | Remove-Item -Force
```

```bash
# Bash
find target -name ".cargo-lock" -delete
```

### 3. Remove Gradle locks (Android)

If Gradle builds hang:

```powershell
# PowerShell
Get-ChildItem -Path "android\.gradle" -Recurse -Force -Filter "*.lock" -ErrorAction SilentlyContinue | Remove-Item -Force
```

```bash
# Bash
find android/.gradle -name "*.lock" -delete
```

### 4. Remove Git index lock

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

| Task | Command |
|------|---------|
| Build server + client | `cargo build -p ember-server -p ember-client` |
| Build Android APK | `.\build-android.ps1` (Windows) or `./build-android.sh` (Linux/macOS) |
| Sign APK | `.\sign-apk.ps1` |
| Run server | `cargo run -p ember-server` |
| Test client | `cargo run -p ember-client -- 127.0.0.1:4433 "What is 2+2?"` |

---

## See Also

- [README.md](../README.md) — Quick start and architecture
- [PORT-FORWARDING-AND-TUNNEL-SETUP.md](PORT-FORWARDING-AND-TUNNEL-SETUP.md) — Exposing ember-server
- [PGROK-EAGLEONE-README.md](PGROK-EAGLEONE-README.md) — Tunnel setup for eagleoneonline.ca
