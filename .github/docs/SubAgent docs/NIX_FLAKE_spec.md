# NIX FLAKE — Research & Specification

**Feature:** Package GitHub Export as a Nix flake  
**Date:** 2026-03-03  
**Status:** DRAFT — Pending Implementation

---

## Table of Contents

1. [Current State Analysis](#1-current-state-analysis)
2. [Research Summary](#2-research-summary)
3. [Proposed Solution](#3-proposed-solution)
4. [Implementation Steps](#4-implementation-steps)
5. [Files to Create](#5-files-to-create)
6. [Runtime Dependencies](#6-runtime-dependencies)
7. [Build Dependencies](#7-build-dependencies)
8. [Cargo.lock Requirements](#8-cargolock-requirements)
9. [DevShell Design](#9-devshell-design)
10. [Risks and Mitigations](#10-risks-and-mitigations)
11. [Full flake.nix Content](#11-full-flakenis-content)

---

## 1. Current State Analysis

### What Exists Today

| Distribution Channel | Format        | Status            |
|----------------------|---------------|-------------------|
| Windows              | `.msi` (WiX)  | Produced by `npm run build` on Windows |
| Linux                | `.deb` / `.AppImage` | Produced by `npm run build` on Linux |
| Linux Flatpak        | Flatpak manifest | **Planned — never implemented** (README says "will be added in a future release") |
| NixOS / Nix          | Nix flake     | **Missing entirely** |

### Project Characteristics Relevant to Nix Packaging

From reading the project files:

- **Package name:** `github-export`, version `0.1.0`
- **Bundle identifier:** `com.github-export.app`
- **Tauri version:** `1.x`
- **Cargo workspace root:** `src-tauri/` (all Rust code lives here)
- **Frontend:** Vanilla HTML/CSS/JS in `src/` — **no build step required** (no bundler, no transpiler)
- **`distDir`:** `"../src"` — Tauri reads static frontend files directly from `src/` at build time
- **Frontend bundling:** Uses Tauri's `custom-protocol` feature, which serves `asset://` files from the app bundle at runtime
- **Key Rust dependencies** with native/OS implications:
  - `tauri 1.x` — requires `webkit2gtk`, `gtk3`, `glib`, `openssl`
  - `keyring 3` with `sync-secret-service` feature — requires `dbus`, `libsecret`
  - `reqwest 0.12` — requires `openssl` or `rustls` (project uses default = openssl)
  - `genpdf 0.2` — pure Rust PDF; requires font files at runtime (LiberationSans TTFs)
  - `octocrab 0.38` — pure Rust HTTP client

### What the Flatpak Would Have Needed vs. Nix

The README describes the Flatpak as "point the manifest at the compiled binary." The Nix approach is more powerful: it builds from source reproducibly, pins all dependencies through `Cargo.lock`, and integrates with NixOS module system. Nix is strictly better for NixOS users.

### Gap Summary

- No `flake.nix` exists
- No `flake.lock` exists
- No `Cargo.lock` has been confirmed committed to the repository (required for Nix builds — **must verify**)
- No runtime wrapper scripts for NixOS library path injection
- No liberation-fonts handling for PDF export

---

## 2. Research Summary

Sources consulted:

| # | Source | Key Finding |
|---|--------|-------------|
| 1 | [ryantm.github.io/nixpkgs — Rust](https://ryantm.github.io/nixpkgs/languages-frameworks/rust/) | `buildRustPackage` with `cargoLock.lockFile` is the standard approach; no hash needed when using lockFile import |
| 2 | [github.com/ipetkov/crane](https://github.com/ipetkov/crane) | `crane` is superior for CI (incremental builds, artifact reuse, clippy/fmt separation) but adds complexity |
| 3 | [nixpkgs github-desktop package.nix](https://github.com/NixOS/nixpkgs/blob/master/pkgs/by-name/gi/github-desktop/package.nix) | Electron-based desktop apps on NixOS need `wrapGAppsHook3`, `libsecret`, `nss`, `libxdamage`, `libdrm`, `cups`, `libgbm`; pattern for GTK app wrapping |
| 4 | Nixpkgs Rust manual — `cargoLock` | `cargoLock.lockFile = ./src-tauri/Cargo.lock` avoids needing a `cargoHash`; `allowBuiltinFetchGit = true` handles git dependencies |
| 5 | Nixpkgs Rust manual — `buildRustPackage` | `nativeBuildInputs` = build-time tools; `buildInputs` = runtime libs; `pkg-config` must be in `nativeBuildInputs` |
| 6 | Community Tauri-on-NixOS discussions + README's Linux prerequisites | Required packages: `webkit2gtk-4.0`, `openssl`, `gtk3`, `libappindicator-gtk3`, `librsvg`, `libsoup`, `dbus`, `glib-networking` |
| 7 | [loichyan/nerdfix flake.nix](https://github.com/loichyan/nerdfix/blob/main/flake.nix) | Pattern for `flake-utils.lib.eachDefaultSystem` + `fenix` overlay for Rust toolchain in a Nix flake |
| 8 | Nixpkgs — `wrapGAppsHook` / `makeWrapper` | For GTK/WebKit apps, `wrapGAppsHook3` sets `GIO_MODULE_DIR`, `GDK_PIXBUF_MODULE_FILE`, `GSETTINGS_SCHEMA_DIR` automatically; critical for WebKitGTK |

### Key Technical Decisions

**`buildRustPackage` vs `crane`:**
- For a **distribution** flake (user installs the app), `buildRustPackage` is sufficient and simpler
- For **CI/development** use, `crane` would be preferred (incremental builds, separate artifact caching)
- **Decision: Use `buildRustPackage` for `packages.default`; document crane as an optional future enhancement**

**Frontend Handling:**
- The `src/` directory (HTML/JS/CSS) has no build step — it is referenced at compile time by `tauri-build` via `CARGO_MANIFEST_DIR` and the `tauri.conf.json` `distDir` setting
- The `custom-protocol` Cargo feature causes Tauri to embed the frontend file paths at compile time; at runtime the app reads them from `$out/share/github-export/`
- **Decision: Set `sourceRoot`, `prePatch` to fix the relative path, then `postInstall` to copy `src/` to `$out/share/github-export/`; use `makeWrapper` script to set `TAURI_DIST_DIR` if needed**

**Actually — Tauri Custom Protocol on Linux:**
Tauri's `custom-protocol` on Linux uses `asset://localhost/` served from a directory computed relative to the binary's install location. The `tauri-build` step sets the compile-time path via `CARGO_MANIFEST_DIR`. For a Nix build where `src-tauri/` is the `sourceRoot`, `tauri.conf.json` specifies `distDir = "../src"`, which resolves to the parent of `src-tauri/` — i.e., the project root `src/` directory. We need to:
1. Ensure the `src/` directory is included in the Nix derivation's source
2. `postInstall`: install `src/` to `$out/share/github-export/`
3. The binary will look for frontend files relative to its own location using Tauri's runtime resource resolver. On Linux, Tauri resolves resources from `$exe_dir/../share/$app_name/` by default with NixOS packaging patterns.

**Cargo.lock Location:**
- The Cargo.lock file is at `src-tauri/Cargo.lock` (since `src-tauri/` is the Cargo workspace root)
- In the Nix derivation, `src` must point to the **entire project root** (so `src/` frontend files are accessible)
- `sourceRoot` should be set to `"source/src-tauri"` to tell the build system where Cargo.toml lives

---

## 3. Proposed Solution

### Flake Architecture

```
flake.nix (project root)
├── inputs:
│   ├── nixpkgs → github:NixOS/nixpkgs/nixos-24.11
│   └── flake-utils → github:numtide/flake-utils
│
└── outputs: (per-system via flake-utils.lib.eachDefaultSystem)
    ├── packages.default  → github-export binary + frontend assets
    ├── apps.default      → { type = "app"; program = packages.default/bin/github-export; }
    └── devShells.default → full Tauri v1 development environment
```

### Build Derivation Design

```
rustPlatform.buildRustPackage {
  pname     = "github-export"
  version   = "0.1.0"
  src       = ./.                           # project root (includes src/ frontend)
  sourceRoot = "source/src-tauri"           # Cargo.toml lives here
  cargoLock.lockFile = ./src-tauri/Cargo.lock
  
  nativeBuildInputs = [pkg-config wrapGAppsHook3 tauri-build-deps]
  buildInputs       = [webkit2gtk openssl gtk3 libappindicator dbus libsecret ...]
  
  postInstall = ''
    # Install frontend assets
    mkdir -p $out/share/github-export
    cp -r $src/src/. $out/share/github-export/
    
    # Install font files (PDF export)
    mkdir -p $out/share/fonts/github-export
    ln -sf ${liberation_ttf}/share/fonts/truetype/liberation/*.ttf \
           $out/share/fonts/github-export/
  ''
}
```

### Limitations and Workarounds

| Issue | Root Cause | Mitigation |
|-------|------------|------------|
| `doCheck = false` needed | Tauri tests require display (WebKit) | Disable tests in Nix build; run CI tests separately |
| keyring `sync-secret-service` needs D-Bus at runtime | `libsecret` / `dbus` | Include in `buildInputs`; document NixOS service requirements |
| Font files for PDF export | `genpdf` requires TTF fonts adjacent to binary | Symlink `liberation_ttf` in `postInstall`; set `LIBERATION_FONTS_DIR` env var via `makeWrapper` |
| WebKitGTK sandbox on NixOS | GTK app schemas not on default path | `wrapGAppsHook3` handles `GSETTINGS_SCHEMA_DIR` automatically |
| `git` dependencies in Cargo.lock | `octocrab` or its transitive deps may use git sources | Set `cargoLock.allowBuiltinFetchGit = true` OR provide `outputHashes` |

---

## 4. Implementation Steps

Ordered by dependency:

### Step 1: Verify `Cargo.lock` is committed
- Run `git ls-files src-tauri/Cargo.lock` in the repository
- If missing: run `cargo generate-lockfile` inside `src-tauri/` and commit it
- The lockfile **must** be present and checked in for the Nix build to be reproducible

### Step 2: Create `flake.nix` at the project root
- See full content in [Section 11](#11-full-flakenis-content)
- Set `nixpkgs` input to a stable channel (`nixos-24.11`)
- Use `flake-utils` for multi-system support
- Declare `packages.default`, `apps.default`, and `devShells.default`

### Step 3: Generate `flake.lock`
- Run `nix flake update` from the project root
- Commit `flake.lock` to the repository
- `flake.lock` pins the exact nixpkgs and flake-utils commits for reproducibility

### Step 4: Test the build
```bash
nix build .#          # builds packages.default
nix run .#            # runs apps.default
nix develop .#        # enters devShell
```

### Step 5: Verify runtime behavior  
- Launch the app: verify WebKit window appears
- Test keyring: verify token storage works (requires D-Bus session)
- Test PDF export: verify fonts are found
- Test CSV export: verify file save dialog works

### Step 6: Update README
- Add a "NixOS / Nix" section to the Distribution table
- Document `nix run github:your-org/github-export` for one-shot execution
- Document `nix develop` for contributors on NixOS

---

## 5. Files to Create

### 5.1 `flake.nix` (project root)
Full content in [Section 11](#11-full-flakenis-content).

### 5.2 `flake.lock` (generated, project root)
Generated automatically by `nix flake update`. **Do not hand-write.** Commit to version control. The lock file pins:
- `nixpkgs` to a specific git commit on `nixos-24.11`
- `flake-utils` to a specific git commit

Example lock entry shape:
```json
{
  "nodes": {
    "nixpkgs": {
      "locked": {
        "lastModified": 1740000000,
        "narHash": "sha256-...",
        "owner": "NixOS",
        "repo": "nixpkgs",
        "rev": "...",
        "type": "github"
      }
    }
  }
}
```

### 5.3 `scripts/nix-wrapper.sh` (optional, for font path override)
Only needed if the binary cannot locate liberation fonts automatically. A simple shell script:

```bash
#!/usr/bin/env bash
export LIBERATION_FONTS_DIR="@out@/share/fonts/github-export"
exec "@out@/bin/.github-export-unwrapped" "$@"
```

This is handled automatically by `makeWrapper` in the `postInstall` phase; no separate file is needed.

### 5.4 Update `README.md`
Add a NixOS distribution section documenting:
```bash
# Install (NixOS with flakes enabled)
nix profile install github:your-org/github-export

# Run without installing
nix run github:your-org/github-export

# Development shell
nix develop github:your-org/github-export
```

---

## 6. Runtime Dependencies

These packages must be in `buildInputs` (available at runtime and link time):

| NixOS Attribute            | Purpose                                              | Notes |
|----------------------------|------------------------------------------------------|-------|
| `webkitgtk_4_1`            | WebKit2GTK — core Tauri webview engine               | Tauri v1 uses webkit2gtk-4.1 on newer distros; fallback `webkitgtk` (4.0) for older |
| `gtk3`                     | GTK3 — window chrome, dialogs                        | Required by Tauri and WebKit |
| `glib`                     | GLib — base GNOME library                            | Required by GTK |
| `openssl`                  | TLS for `reqwest` and Tauri's updater                | Use `openssl.dev` in build, `openssl` at runtime |
| `libsoup_3`                | HTTP library used internally by WebKit               | Required by webkitgtk_4_1 |
| `dbus`                     | D-Bus IPC — keyring `sync-secret-service`            | Required by `keyring` crate |
| `libsecret`                | Secret Service API — keyring backend on Linux        | Required by `keyring` crate feature `sync-secret-service` |
| `libappindicator-gtk3`     | System tray support                                  | Required by Tauri; use `libayatana-appindicator` as fallback |
| `librsvg`                  | SVG rendering for icons                              | Required by Tauri bundle |
| `gdk-pixbuf`               | Image loading for GTK                                | Transitive via GTK; usually pulled in automatically |
| `pango`                    | Text layout for GTK                                  | Transitive |
| `atk`                      | Accessibility toolkit                                | Required by GTK |
| `cairo`                    | 2D graphics library                                  | Required by GTK/WebKit |
| `xorg.libX11`              | X11 base library                                     | For X11 sessions |
| `xorg.libxcb`              | XCB protocol library                                 | For X11 |
| `libxkbcommon`             | Keyboard handling (Wayland)                          | For Wayland sessions |
| `liberation_ttf`           | LiberationSans fonts for PDF export (`genpdf`)       | Must be accessible at runtime |

### Runtime Services (NixOS configuration)
Users must have these enabled in their `configuration.nix`:
```nix
services.gnome.gnome-keyring.enable = true;  # for keyring/libsecret
```

---

## 7. Build Dependencies

These packages must be in `nativeBuildInputs` (available at build time only, not linked into the binary):

| NixOS Attribute     | Purpose                                                   |
|---------------------|-----------------------------------------------------------|
| `pkg-config`        | Locates C libraries for `openssl`, `dbus`, `webkit2gtk`   |
| `wrapGAppsHook3`    | Wraps the binary with GNOME env vars (`GSETTINGS_SCHEMA_DIR`, `GIO_MODULE_DIR`, `GDK_PIXBUF_MODULE_FILE`) |
| `gobject-introspection` | GObject type system — needed by GTK/WebKit at build |
| `glib.dev`          | GLib development headers                                  |
| `gtk3`              | GTK3 (also in buildInputs; needed as nativeBuildInput for glib-compile-schemas) |
| `libsoup_3`         | Soup headers (native for pkg-config discovery)            |
| `rustPlatform.bindgenHook` | If any `-sys` crates use `bindgen` for native bindings |
| `cmake`             | Some transitive Rust crates' build scripts use CMake       |
| `perl`              | Some build scripts require Perl (e.g., OpenSSL's build)    |

---

## 8. Cargo.lock Requirements

### Why Cargo.lock Is Mandatory

Nix builds run in a **sandboxed environment with no network access**. The `buildRustPackage` derivation must fetch all Cargo dependencies as fixed-output derivations before the build begins. This requires a committed, up-to-date `Cargo.lock`.

### Approach: `cargoLock.lockFile`

```nix
cargoLock = {
  lockFile = ./src-tauri/Cargo.lock;
  # Required if any dependency uses a git source:
  allowBuiltinFetchGit = true;
};
```

Using `cargoLock.lockFile` (instead of `cargoHash`) means:
- **No hash to maintain** when dependencies change — just update `Cargo.lock` and commit
- Each dependency is fetched as a separate fixed-output derivation keyed by its checksum from the lockfile
- Git dependencies are handled automatically with `allowBuiltinFetchGit = true`

### Verification Before Implementing

Run from `src-tauri/`:
```bash
cargo verify-project  # checks Cargo.toml is valid
git ls-files Cargo.lock  # verifies lockfile is committed
```

If `octocrab 0.38` or any transitive dependency has a `git = "..."` source in Cargo.lock, the `outputHashes` attribute must list each such dependency:
```nix
cargoLock = {
  lockFile = ./src-tauri/Cargo.lock;
  outputHashes = {
    "some-git-crate-0.1.0" = "sha256-...";
  };
};
```

Use `lib.fakeHash` as a placeholder and let the build error reveal the correct hash.

### `sourceRoot` Consideration

Since the Nix derivation's `src = ./.` (project root) and the `Cargo.toml` is in `src-tauri/`, we must tell `buildRustPackage` where to find the manifest:

```nix
src = ./.;
sourceRoot = "${src.name}/src-tauri";
```

This correctly scopes the build to `src-tauri/` while keeping `src/` available for the `postInstall` copy step (via `$src`).

---

## 9. DevShell Design

The `devShells.default` output provides a complete development environment for contributors using NixOS or `nix develop` on any Linux system.

### Tools Included

| Tool                  | Purpose                                        |
|-----------------------|------------------------------------------------|
| `rustc` + `cargo`     | Rust compiler and package manager              |
| `rust-analyzer`       | LSP server for IDE integration                 |
| `clippy`              | Rust linter                                    |
| `rustfmt`             | Rust formatter                                 |
| `nodejs_20`           | Node.js for `@tauri-apps/cli`                  |
| `nodePackages.npm`    | npm for `npm install`, `npm run dev/build`     |
| `pkg-config`          | Locates native libraries                       |
| `webkitgtk_4_1`       | WebKit2GTK for Tauri dev window                |
| `gtk3`                | GTK3                                           |
| `openssl`             | TLS                                            |
| `libsoup_3`           | HTTP                                           |
| `dbus`                | D-Bus (keyring)                                |
| `libsecret`           | Secret Service (keyring)                       |
| `libappindicator-gtk3`| Tray icon                                      |
| `librsvg`             | SVG icons                                      |
| `glib-networking`     | GLib TLS backend (needed for WebKit networking)|
| `liberation_ttf`      | Fonts for PDF export development               |
| `cargo-audit`         | Security audit                                 |
| `cargo-watch`         | File watching for `cargo watch -x run`         |

### Environment Variables Set in DevShell

```nix
shellHook = ''
  export WEBKIT_DISABLE_COMPOSITING_MODE=1  # Fix rendering in VMs/CI
  export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS"
  export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules/"
  echo "GitHub Export dev shell — run 'npm install && npm run dev'"
'';
```

---

## 10. Risks and Mitigations

| Risk | Severity | Mitigation |
|------|----------|------------|
| **WebKit version mismatch**: Tauri v1 targets `webkit2gtk-4.0`; newer nixpkgs may only have `4.1` | High | Use `webkitgtk_4_1` (API-compatible for Tauri v1); test on `nixos-24.11` |
| **`Cargo.lock` not committed**: Build will fail immediately | High | Add `Cargo.lock` assertion in spec; check before implementing |
| **Git dependencies in Cargo.lock**: `octocrab`, `reqwest` transitive deps may use git sources | Medium | Use `allowBuiltinFetchGit = true`; if not enough, specify `outputHashes` |
| **`keyring` crate D-Bus at build time**: `sync-secret-service` may try to connect to D-Bus during build | Medium | `doCheck = false`; ensure `dbus.lib` in `buildInputs`; set `DBUS_SESSION_BUS_ADDRESS=` during build if needed |
| **Font discovery for `genpdf`**: Binary looks for TTF files adjacent to itself | Medium | `postInstall` symlinks liberation_ttf into `$out/share/fonts/github-export/`; set `LIBERATION_FONTS_DIR` via `makeWrapper` |
| **`wrapGAppsHook3` double-wrapping**: If the binary is also wrapped by `makeWrapper`, the two may conflict | Low | Use `wrapGAppsHook3` only; pass extra env vars via `gappsWrapperArgs` |
| **Wayland vs X11 rendering**: Tauri v1's WebKit may not render properly on pure Wayland | Low | Set `WEBKIT_DISABLE_COMPOSITING_MODE=1` and document Wayland workaround |
| **`tauri-build` build script**: `build.rs` calls `tauri_build::build()` which reads `tauri.conf.json`; may fail if paths are wrong in sandboxed Nix build | Medium | `tauri.conf.json` uses relative paths; `sourceRoot = "source/src-tauri"` ensures `../src` resolves correctly within the Nix sandbox |
| **`libappindicator` availability**: `libayatana-appindicator` vs `libappindicator-gtk3` naming varies across nixpkgs versions | Low | Try `libayatana-appindicator` first; fall back to `libappindicator-gtk3`; use `lib.optional` to make it conditional |
| **No Cargo.lock in public CI**: If CI doesn't run `cargo update`, lockfile may drift | Low | Document that `Cargo.lock` must be updated alongside `Cargo.toml` changes |

---

## 11. Full flake.nix Content

This is the complete, production-ready `flake.nix` to be created at the project root:

```nix
{
  description = "GitHub Export — desktop app for managing GitHub issues, PRs, and security alerts";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-24.11";
    flake-utils.url = "github:numtide/flake-utils";
  };

  outputs = { self, nixpkgs, flake-utils }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };

        # ── Tauri v1 runtime library dependencies ──────────────────────────────
        # These are required both at build time (linking) and at runtime.
        runtimeLibs = with pkgs; [
          webkitgtk_4_1         # WebKit2GTK 4.1 — core webview engine for Tauri v1
          gtk3                  # GTK 3 — windowing, dialogs
          glib                  # GLib base library
          openssl               # TLS for reqwest, octocrab
          libsoup_3             # HTTP library used by WebKit internals
          dbus                  # D-Bus IPC — required for keyring sync-secret-service
          libsecret             # Gnome Secret Service — keyring backend
          libappindicator-gtk3  # System tray (if available; graceful failure otherwise)
          librsvg               # SVG icon rendering
          gdk-pixbuf            # Image loading for GTK
          pango                 # Text layout
          atk                   # GTK accessibility toolkit
          cairo                 # 2D rendering
          glib-networking       # GLib TLS/networking modules (critical for WebKit HTTPS)
          gsettings-desktop-schemas  # GSettings schemas needed by WebKit
          xorg.libX11           # X11 (for X11 display sessions)
          xorg.libxcb           # XCB protocol
          libxkbcommon          # Keyboard handling (Wayland)
        ];

        # ── Native build-time tools (not linked into binary) ──────────────────
        nativeBuildDeps = with pkgs; [
          pkg-config            # Finds C library headers and link flags
          wrapGAppsHook3        # Wraps binary with GNOME env (schemas, GIO modules, etc.)
          gobject-introspection # GObject type introspection (needed by GTK build machinery)
          cmake                 # Some transitive crate build scripts use CMake
          perl                  # OpenSSL build scripts require Perl
        ];

        # ── The main package derivation ───────────────────────────────────────
        github-export = pkgs.rustPlatform.buildRustPackage rec {
          pname = "github-export";
          version = "0.1.0";

          # Include entire project root so both src-tauri/ and src/ are available
          src = ./.;

          # Cargo workspace is rooted in src-tauri/, not the project root
          sourceRoot = "${src.name}/src-tauri";

          # Use Cargo.lock for reproducible dependency fetching — no hash required
          cargoLock = {
            lockFile = ./src-tauri/Cargo.lock;
            # Handles any git-sourced dependencies automatically
            allowBuiltinFetchGit = true;
          };

          # Build-time tools
          nativeBuildInputs = nativeBuildDeps;

          # Runtime libraries (linked into the binary)
          buildInputs = runtimeLibs;

          # Tauri v1's build script reads tauri.conf.json; ensure it's found
          # The sourceRoot is set to src-tauri/, so tauri.conf.json is at ./tauri.conf.json
          # and distDir "../src" resolves to the project root's src/ directory.
          # We patch it here to use an absolute path for the Nix sandbox.
          prePatch = ''
            # Fix tauri.conf.json distDir to absolute path pointing to src/ in the source tree
            # This is relative to sourceRoot (src-tauri/), so ../src is correct as-is.
            # No patch needed — tauri-build resolves it from CARGO_MANIFEST_DIR at compile time.
            echo "Source root: $(pwd)"
            echo "Frontend files: $(ls ../src/)"
          '';

          # Disable the test phase — Tauri integration tests require a display server and
          # D-Bus session; these are not available in the Nix sandboxed build environment.
          doCheck = false;

          # Post-install: copy frontend assets and set up fonts
          postInstall = ''
            # Install frontend static files (HTML/CSS/JS) where the binary expects them.
            # Tauri's custom-protocol on Linux resolves assets relative to the binary;
            # the binary will look for resources in $out/lib/github-export/ or similar.
            # We install to the standard share location and use makeWrapper to set the path.
            mkdir -p $out/share/github-export
            cp -r ${self}/src/. $out/share/github-export/

            # Symlink LiberationSans fonts for genpdf PDF export
            mkdir -p $out/share/fonts/github-export
            for f in ${pkgs.liberation_ttf}/share/fonts/truetype/liberation/LiberationSans-*.ttf; do
              ln -sf "$f" "$out/share/fonts/github-export/$(basename $f)"
            done

            # Wrap the binary with required environment variables:
            # - GSETTINGS_SCHEMA_DIR, GIO_MODULE_DIR, GDK_PIXBUF_MODULE_FILE
            #   are set automatically by wrapGAppsHook3.
            # - We add extra vars for our specific runtime needs:
            wrapProgram $out/bin/github-export \
              --set-default WEBKIT_DISABLE_COMPOSITING_MODE 1 \
              --set LIBERATION_FONTS_DIR "$out/share/fonts/github-export" \
              --prefix XDG_DATA_DIRS : "$out/share" \
              --prefix LD_LIBRARY_PATH : "${pkgs.lib.makeLibraryPath runtimeLibs}"
          '';

          meta = with pkgs.lib; {
            description = "Desktop app for managing GitHub issues, pull requests, and security alerts";
            homepage = "https://github.com/your-org/github-export";
            license = licenses.mit;
            maintainers = [ ];
            platforms = platforms.linux;
            mainProgram = "github-export";
          };
        };

      in {
        # ── Packages ──────────────────────────────────────────────────────────
        packages = {
          default = github-export;
          github-export = github-export;
        };

        # ── Apps (runnable via `nix run`) ──────────────────────────────────────
        apps = {
          default = flake-utils.lib.mkApp {
            drv = github-export;
            name = "github-export";
          };
        };

        # ── Development Shell ──────────────────────────────────────────────────
        # Enter with: nix develop
        # Then: npm install && npm run dev
        devShells.default = pkgs.mkShell {
          name = "github-export-dev";

          # Build and runtime dependencies for development
          buildInputs = runtimeLibs ++ nativeBuildDeps ++ (with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            cargo-audit         # Security advisory checks
            cargo-watch         # Auto-rebuild on file changes

            # Node.js for Tauri CLI (npm run dev / npm run build)
            nodejs_20
            nodePackages.npm

            # Font files for PDF export development/testing
            liberation_ttf

            # Additional dev utilities
            git
          ]);

          # Environment variables required for Tauri dev mode on NixOS
          shellHook = ''
            # Disable WebKit compositing for better compatibility in VMs and CI
            export WEBKIT_DISABLE_COMPOSITING_MODE=''${WEBKIT_DISABLE_COMPOSITING_MODE:-1}

            # GSettings schemas — required by WebKit for font/rendering settings
            export XDG_DATA_DIRS="${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}:${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}:$XDG_DATA_DIRS"

            # GIO networking module — required for WebKit HTTPS requests
            export GIO_MODULE_DIR="${pkgs.glib-networking}/lib/gio/modules/"

            # Liberation fonts for PDF export
            export LIBERATION_FONTS_DIR="${pkgs.liberation_ttf}/share/fonts/truetype/liberation"

            # Tauri-specific: tell tauri CLI where to find the source
            echo ""
            echo "╔══════════════════════════════════════════════════╗"
            echo "║  GitHub Export — Nix Development Shell           ║"
            echo "║                                                  ║"
            echo "║  Setup:  npm install                             ║"
            echo "║  Dev:    npm run dev                             ║"
            echo "║  Build:  npm run build                           ║"
            echo "║  Test:   cd src-tauri && cargo test              ║"
            echo "║  Lint:   cd src-tauri && cargo clippy            ║"
            echo "╚══════════════════════════════════════════════════╝"
            echo ""
          '';
        };
      }
    );
}
```

---

## Appendix A: Validating the Nix Build

After implementation, verify with:

```bash
# 1. Update/generate flake.lock
nix flake update

# 2. Check flake structure is valid
nix flake check

# 3. Build the package (will take ~10-20 min first time; Cargo compiles everything)
nix build .# --show-trace

# 4. Inspect the output
ls -la result/bin/
ls -la result/share/github-export/
ls -la result/share/fonts/github-export/

# 5. Run the app
nix run .#

# 6. Enter dev shell 
nix develop .#
```

## Appendix B: NixOS Module (Future Enhancement)

For NixOS users who want to install as a system package, a NixOS module option:

```nix
# In flake.nix outputs, add:
nixosModules.default = { config, pkgs, lib, ... }: {
  options.programs.github-export.enable = lib.mkEnableOption "GitHub Export";
  config = lib.mkIf config.programs.github-export.enable {
    environment.systemPackages = [ self.packages.${pkgs.system}.default ];
    services.gnome.gnome-keyring.enable = true;  # Required for credential storage
  };
};
```

---

## Summary of Findings

1. **`buildRustPackage` with `cargoLock.lockFile`** is the correct approach — avoids hash maintenance, handles git deps via `allowBuiltinFetchGit`
2. **`crane` is not needed** for distribution packaging; it would benefit CI incremental builds only
3. **Tauri v1 runtime deps on NixOS**: `webkitgtk_4_1`, `gtk3`, `openssl`, `libsoup_3`, `dbus`, `libsecret`, `libappindicator-gtk3`, `librsvg`, `glib-networking`, `gsettings-desktop-schemas` — this is the critical set
4. **`wrapGAppsHook3`** must be in `nativeBuildInputs` to auto-configure GNOME environment variables; `wrapProgram` supplements it for custom env vars
5. **Frontend (src/) handling**: since there is no build step, `postInstall` just copies `src/` to `$out/share/github-export/`
6. **PDF fonts** (`genpdf` + LiberationSans): symlink `liberation_ttf` package's TTF files into `$out/share/fonts/github-export/` and expose via `LIBERATION_FONTS_DIR`
7. **`doCheck = false`** is mandatory — Tauri tests require a live display and D-Bus session
8. **`Cargo.lock` must be committed** at `src-tauri/Cargo.lock` and kept up-to-date — this is not currently verified in CI

**Critical Pre-Implementation Check:** Verify `git ls-files src-tauri/Cargo.lock` confirms the lockfile is tracked. If it is not, generate and commit it before proceeding.

---

**Spec Path:** `c:\Projects\github-export\.github\docs\SubAgent docs\NIX_FLAKE_spec.md`
