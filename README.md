# GitHub Export

A cross-platform desktop application for managing **GitHub issues**, **pull requests**, and **security alerts** — built with [Rust](https://www.rust-lang.org/) and [Tauri](https://tauri.app/).

> Designed to complement [GitHub Desktop](https://desktop.github.com/) by covering the issue/PR/security workflow that GitHub Desktop does not handle.

---

## Features

| Feature | Status |
|---|---|
| Authenticate via Personal Access Token (PAT) | ✅ |
| Persistent credential storage (OS keyring) | ✅ |
| List repositories for the authenticated user | ✅ |
| View issues with state, labels, assignees | ✅ |
| View pull requests with branch info, draft flag | ✅ |
| View Dependabot security alerts | ✅ |
| Filter by state (open / closed / all) | ✅ |
| Sort by created, updated, comments | ✅ |
| Free-text search across titles | ✅ |
| Export to **CSV** | ✅ |
| Export to **PDF** | ✅ |
| Dark-themed UI inspired by GitHub | ✅ |

---

## Screenshots

_Coming soon — run `npm run dev` to see the app in action._

---

## Tech Stack

| Layer | Technology |
|---|---|
| GUI framework | **Tauri 1.x** (Rust backend + native WebView) |
| GitHub API | **octocrab** — typed Rust client for the GitHub REST API |
| CSV export | **csv** crate |
| PDF export | **genpdf** crate |
| Credential storage | **keyring** crate (Windows Credential Manager, macOS Keychain, Linux Secret Service) |
| Serialization | **serde** + **serde_json** |
| Async runtime | **tokio** |
| Frontend | Vanilla HTML / CSS / JS (no framework, no bundler) |

---

## Prerequisites

1. **Rust** (1.70+): <https://rustup.rs/>
2. **Node.js** (18+): <https://nodejs.org/>
3. **Tauri CLI prerequisites** (platform-specific native deps):
   - **Windows**: MSVC Build Tools (Visual Studio 2022) + WebView2 (pre-installed on Windows 10/11)
   - **Ubuntu/Debian**: `sudo apt install libwebkit2gtk-4.0-dev build-essential curl wget file libssl-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
   - **Fedora**: `sudo dnf install webkit2gtk4.0-devel openssl-devel gtk3-devel libappindicator-gtk3-devel librsvg2-devel`
   - **Arch**: `sudo pacman -S webkit2gtk base-devel openssl gtk3 libappindicator-gtk3 librsvg2`
4. **GitHub PAT** with `repo` + `security_events` scopes: <https://github.com/settings/tokens>

For **PDF export**, place the Liberation Sans font files next to the compiled binary:
- `LiberationSans-Regular.ttf`
- `LiberationSans-Bold.ttf`
- `LiberationSans-Italic.ttf`
- `LiberationSans-BoldItalic.ttf`

On most Linux distros these are available via `fonts-liberation` or `liberation-sans-fonts`.

---

## Getting Started

```bash
# 1. Clone the repo
git clone https://github.com/your-org/github-export.git
cd github-export

# 2. Install the Tauri CLI
npm install

# 3. Run in development mode (hot-reload UI, Rust recompiles on save)
npm run dev

# 4. Build a production release
npm run build
```

The production artifacts land in `src-tauri/target/release/bundle/`:
- **Windows**: `.msi` installer
- **Linux**: `.deb`, `.AppImage`

---

## Project Structure

```
github-export/
├── src/                          # Frontend (loaded into WebView)
│   ├── index.html
│   ├── main.js
│   └── styles.css
├── src-tauri/                    # Rust backend
│   ├── Cargo.toml
│   ├── build.rs
│   ├── tauri.conf.json
│   └── src/
│       ├── main.rs               # Tauri commands & app entry
│       ├── models/
│       │   └── mod.rs            # Data types (Issue, PR, Alert, …)
│       ├── github/
│       │   ├── mod.rs
│       │   ├── auth.rs           # PAT authentication, keyring ops
│       │   ├── issues.rs         # Fetch repos & issues
│       │   ├── pulls.rs          # Fetch pull requests
│       │   └── security.rs       # Fetch Dependabot alerts
│       └── export/
│           ├── mod.rs
│           ├── csv_export.rs     # CSV writer
│           └── pdf_export.rs     # PDF report generator
├── package.json
├── README.md
└── LICENSE
```

---

## Distribution

### Windows (.msi)

The Tauri build automatically produces an `.msi` installer when building on Windows:

```bash
npm run build
# Output: src-tauri/target/release/bundle/msi/GitHub Export_0.1.0_x64_en-US.msi
```

### Linux (Flatpak)

Tauri produces `.deb` and `.AppImage` out of the box. To create a **Flatpak**:

1. Install Flatpak builder: `sudo apt install flatpak-builder`
2. Create a `com.github_export.app.yml` manifest (see [Flatpak docs](https://docs.flatpak.org/en/latest/first-build.html))
3. Point the manifest at the compiled binary in `src-tauri/target/release/`

> A sample Flatpak manifest will be added in a future release.

---

## Library Recommendations

| Purpose | Crate | Notes |
|---|---|---|
| GitHub REST API | `octocrab` | Typed, async, actively maintained |
| GitHub GraphQL | `graphql_client` + `reqwest` | For tighter queries (optional) |
| CSV export | `csv` | De-facto standard |
| PDF export | `genpdf` | Pure Rust, no C deps |
| PDF (advanced) | `printpdf` | Lower-level, more control |
| Cross-platform GUI | **Tauri** | WebView + Rust back-end |
| Alt. pure Rust GUI | `iced` | Elm-architecture, GPU-accelerated |
| Alt. pure Rust GUI | `egui` / `eframe` | Immediate-mode, great for tools |
| Secure credential storage | `keyring` | OS-native (Keychain / Credential Manager / Secret Service) |
| Date/time | `chrono` | Serde-compatible timestamps |
| Error handling | `anyhow` + `thiserror` | Ergonomic error types |

---

## Contributing

1. Fork the repo and create a feature branch.
2. `npm run dev` to iterate.
3. Open a PR with a clear description.

---

## License

[MIT](LICENSE)