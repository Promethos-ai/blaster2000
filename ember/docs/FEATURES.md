# Ember — Features & Functions

Comprehensive documentation of Ember's features, including the Android Loader, control mechanisms, push channel, and more.

## Changelog

### v0.1.30
- **Splash screen** — On server connect, shows Promethos logo + "EMBER ASSISTANT", then fades to normal layout. Fade triggers on first push or first ask response.

### v0.1.29
- **Error sound** — On error, app plays a gentle tone instead of speaking error text.
- **Chat styles server-side** — `.\push-to-ember.ps1 "style"` pushes chat-style.css; server force-reloads styles on startup.
- **Plain chat** — Conversation elements: no rounded corners, plain solid text.
- **Rich area** — DRIVING MODE label; QR asset on left.

### v0.1.28
- **Rich area QR** — QR code (promqr.png) bundled as asset; shown on left of placeholder when empty. Scan to download APK.
- **Chat bubbles** — Rounded corners on each question/answer (`width: fit-content`, `overflow: hidden`).
- **Tag stripping** — Stricter `<|...|>` handling; flush boundary avoids splitting partial tags.
- **Build** — `build-android.ps1` copies promqr.png to assets when present.

### v0.1.27
- **Chat styling** — Pill-shaped bubbles (`border-radius: 9999px`), smaller text (13px), white on black background.
- **Rich area images** — WebView base URL and mixed content allow external images (e.g. QR from GitHub releases).
- **Push QR URL** — Default QR image points to v0.1.26 release.

### v0.1.26
- **Stay connected** — QUIC idle timeout disabled (was 30s default). Long-poll `__fetch_push__` no longer drops; connection stays alive.
- **Push QR** — `.\push-to-ember.ps1 "qr"` clears screen and shows QR code in rich area (scan to download APK).
- **Rich area outline** — Border stroke increased to 3dp for visibility.
- **install-grpc-service** — NSSM fallback: uses PATH (winget) or GitHub mirror when nssm.cc is down.

### v0.1.25
- **Chat scroll** — Scrolls from bottom up; smooth scroll to newest messages.
- **Message fade** — Older messages fade as they scroll up (gradient overlay at top).
- **QUIC idle timeout** — Server idle timeout increased to 90s so long-poll connections don't drop.

### v0.1.24
- **Long-poll push** — `__fetch_push__` now long-polls: server holds the connection until a push arrives (or 60s timeout). Screen clear and other pushes are delivered immediately instead of waiting up to 5 seconds.
- **Rich media area** — Bordered with accent blue stroke; shows Promethos logo placeholder when empty (replaces chicken).
- **Instructions file** — `--instructions-file` content is read on every request; edit `instructions.txt` and changes apply without restart.
- **Marquee push** — `.\push-to-ember.ps1 "marquee"` fetches weather (Open-Meteo) and gas prices (NREL) for last shared location, pushes rich HTML, then refresh.
- **start-servers.ps1** — Uses `--instructions-file instructions.txt` by default.

---

## 1. Ember Loader (Android app updater)

The **Ember Loader** is a separate Android app that checks for new Ember APK releases and installs them when available.

### How it works

1. **Fetches GitHub releases** — Queries `https://api.github.com/repos/Promethos-ai/blaster2000/releases`
2. **Finds Ember APK** — Looks for the first asset whose name starts with `ember-` and ends with `.apk`
3. **Compares versions** — Shows installed vs. latest; enables "Install" if a newer version exists
4. **Downloads & installs** — On tap, downloads the APK and launches the system installer

### When a new APK is available

- Open the **Ember Loader** app
- It fetches releases on launch
- If a newer version exists than what's installed, the **Install** button appears
- Tap **Install** → APK downloads (progress shown) → system prompts to install
- After install, the new Ember app is ready

### Loader UI

| State | Display |
|-------|---------|
| Checking | Progress spinner, "Checking..." |
| Up to date | "Up to date", no Install button |
| Update available | Current vs latest version, **Install** button |
| Downloading | Progress percentage |
| Installing | "Installing..." → system install dialog |
| Errors | Toast + status text (fetch/download/install failed) |

### Building the Loader

```powershell
# Windows
cd android
.\gradlew.bat :loader:assembleRelease
# APK at android/loader/build/outputs/apk/release/loader-release.apk
```

```bash
# Linux/macOS
cd android
./gradlew :loader:assembleRelease
```

### Releasing so the Loader finds it

For the Loader to detect a new Ember APK:

1. **Build** the main app: `.\build-android.ps1`
2. **Sign** the APK: `.\sign-apk.ps1`
3. **Create release** with an asset named `ember-{version}.apk` (e.g. `ember-0.1.25.apk`)

```powershell
# Copy signed APK to expected name, then release (includes promqr.png if present)
Copy-Item android\app\build\outputs\apk\release\app-release.apk ember-0.1.27.apk
.\release-android.ps1 -Version "v0.1.27" -ApkPath "ember-0.1.27.apk"
```

**QR code download:** Place `promqr.png` in the ember folder. The release script uploads it to every release so users can scan the QR code to download the APK.

### Loader configuration

| Config | Value |
|--------|-------|
| GitHub repo | `Promethos-ai/blaster2000` |
| Ember package | `com.ember.android` |
| APK name pattern | `ember-*.apk` |

---

## 2. Control pipe & app reset

### Control messages

Any `__word__` pattern is a control message. The server handles them in-process; they never reach the LLM.

| Message | Action |
|---------|--------|
| `__get_style__` | Return CSS from server |
| `__fetch_push__` | Long-poll: pop from proactive_queue, return payload (blocks until push or 60s) |
| `__check_in__` | Proactive check-in (synthetic prompt or queued msg) |
| `__app_clear__` | App reset (see below) |
| `__whatever__` | Any other `__word__` → return empty |

### App reset (`app clear` / `__app_clear__`)

When the app receives `"app clear"` or `"__app_clear__"` (from push channel or AI output):

- Clears chat history
- Clears rich content area
- Clears prompt input
- Resets CSS to default
- Hides error area

**Send via push script:**
```powershell
.\push-to-ember.ps1 "app clear"
```

**User intent:** When the user says "reset my screen", "clear the app", "screen to default", etc., the server routes to the control pipeline (no LLM). It sends `stream_control_payload` so the app executes immediately.

**AI output:** The AI can output `__app_clear__` or `<ember_push>app clear</ember_push>`; the server strips it; the app receives it immediately via long-poll. If the AI says "app clear" as text, the server has a failsafe and the app also detects it locally.

---

## 3. Push channel

The server exposes a TCP push channel (default port **4434**) so external processes can push messages to the app.

### Push script

```powershell
# Plain message (appends to chat)
.\push-to-ember.ps1 "Hello from the server!"

# App reset
.\push-to-ember.ps1 "app clear"

# Marquee: weather + gas prices for last shared location (Open-Meteo + NREL)
.\push-to-ember.ps1 "marquee"

# Reload ChatWebView styles from server (server/chat-style.css) — edit CSS, push, app updates at will
.\push-to-ember.ps1 "style"

# Refresh DOM (re-render chat + rich WebViews; fixes layout glitches)
.\push-to-ember.ps1 "refresh"

# Structured payload (JSON)
.\push-to-ember.ps1 -Payload '{"chat":[{"text":"Hi","isUser":true}],"rich":"<div>Dashboard</div>"}'
.\push-to-ember.ps1 -PayloadFile payload.json
```

### Fallback: push-queue.txt

If the TCP push channel is unavailable, the script writes to `push-queue.txt`. The server polls this file every second and queues any lines found.

### Structured payload fields

| Field | Type | Description |
|-------|------|-------------|
| `chat` | `[{text, isUser}, ...]` | Replace entire chat history |
| `chatCss` | string | CSS for chat area |
| `rich` | string | HTML for rich content area (top); empty = clear |
| `richStyle` | string | CSS to inject into rich area |
| `layout` | `{rich_height, theme}` | Layout hints |
| `input` | string | Prefill the prompt input |
| `message` | string | Append as AI message (fallback) |
| `refresh` | boolean | Re-render chat + rich WebViews (fixes layout glitches) |

---

## 4. Android app features

| Feature | Description |
|---------|-------------|
| **Streaming chat** | Token-by-token AI responses |
| **Check-in** | Proactive greeting when user taps "Check in" |
| **Control supervisor** | Long-polls `__fetch_push__` when in foreground; pushes delivered immediately |
| **TTS** | Optional text-to-speech for AI responses |
| **Rich content** | WebView for HTML (weather, cards, etc.); bordered area with Promethos logo when empty |
| **Chat scroll** | Scrolls from bottom up; older messages fade as they scroll up |
| **Voice input** | Microphone for speech-to-text |
| **Location** | Share location for context; server fetches local environment (address, nearby parks/water/shops) from coordinates via OSM. Location is remembered for follow-up questions until app clear. |
| **Error area** | Fixed area for errors (no scroll) |
| **Display adaptation** | Responsive layouts for different screen sizes |

---

## 5. Server features

| Feature | Description |
|---------|-------------|
| **QUIC** | Listens on UDP 4433 |
| **Push channel** | TCP 4434 for external push |
| **File push** | Polls `push-queue.txt` every 1s |
| **Inference params** | Reads `inference_params.json` on every request |
| **Instructions file** | `--instructions-file` for dynamic behavior (read on every request, no restart) |
| **Web search** | Brave Search via `--web-search` |
| **Connection log** | `ember-connections.log` |

---

## 6. AI output tags

The AI can use these tags in its output (server strips them):

| Tag | Purpose |
|-----|---------|
| `<ember_rich>HTML</ember_rich>` | Display-only content (weather, cards) |
| `<ember_style>CSS</ember_style>` | Dynamic CSS for rich area |
| `<ember_layout>JSON</ember_layout>` | Layout hints |
| `<ember_speak>text</ember_speak>` | TTS (spoken to user) |
| `<ember_push>payload</ember_push>` | Queue control payload for app |
| `__command__` | e.g. `__app_clear__` — queued for app |

---

## 7. Build & release scripts

| Script | Purpose |
|--------|---------|
| `build-android.ps1` | Build main APK (Rust + Gradle) |
| `build-android.sh` | Same as above (Linux/macOS) |
| `sign-apk.ps1` | Sign APK for distribution |
| `release-android.ps1` | Create GitHub release, upload APK |
| `push-to-ember.ps1` | Push message to app via server |
| `push-loader.ps1` | Build loader, push to repo, upload to release |
| `start-servers.ps1` | Start grpc_server, then ember-server (with instructions file) |

---

## 8. See also

- [ARCHITECTURE.md](ARCHITECTURE.md) — System architecture
- [CONTROL-PIPE.md](CONTROL-PIPE.md) — Control pipe protocol
- [DEVELOPMENT.md](DEVELOPMENT.md) — Build & troubleshooting
- [README.md](../README.md) — Quick start
