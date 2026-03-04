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
          webkitgtk_4_1              # WebKit2GTK 4.1 — core webview engine for Tauri v1
          gtk3                       # GTK 3 — windowing, dialogs
          glib                       # GLib base library
          openssl                    # TLS for reqwest, octocrab
          libsoup_3                  # HTTP library used by WebKit internals
          dbus                       # D-Bus IPC — required for keyring sync-secret-service
          libsecret                  # GNOME Secret Service — keyring backend
          libappindicator-gtk3       # System tray support
          librsvg                    # SVG icon rendering
          gdk-pixbuf                 # Image loading for GTK
          pango                      # Text layout
          atk                        # GTK accessibility toolkit
          at-spi2-atk                # AT-SPI2 ATK bridge — accessibility IPC bridge
          at-spi2-core               # AT-SPI2 core — accessibility IPC daemon client
          cairo                      # 2D rendering
          glib-networking            # GLib TLS/networking modules (critical for WebKit HTTPS)
          gsettings-desktop-schemas  # GSettings schemas needed by WebKit
          xorg.libX11                # X11 base library (X11 sessions)
          xorg.libxcb                # XCB protocol library
          libxkbcommon               # Keyboard handling (Wayland)
        ];

        # ── Native build-time tools (not linked into binary) ──────────────────
        nativeBuildDeps = with pkgs; [
          pkg-config            # Finds C library headers and link flags
          wrapGAppsHook3        # Wraps binary with GNOME env vars (schemas, GIO modules, etc.)
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

          # Cargo workspace is rooted in src-tauri/, not the project root.
          # sourceRoot tells buildRustPackage where Cargo.toml lives within the unpacked src.
          sourceRoot = "${src.name}/src-tauri";

          # Use Cargo.lock for reproducible dependency fetching — no hash required.
          # allowBuiltinFetchGit handles any git-sourced transitive dependencies.
          cargoLock = {
            lockFile = ./src-tauri/Cargo.lock;
            allowBuiltinFetchGit = true;
          };

          # Build-time tools
          nativeBuildInputs = nativeBuildDeps;

          # Runtime libraries (linked into the binary)
          buildInputs = runtimeLibs;

          # Emit a diagnostic showing what is available before patching.
          # tauri.conf.json uses distDir = "../src" which resolves correctly from
          # src-tauri/ (sourceRoot) to the project root's src/ directory.
          prePatch = ''
            echo "==> Nix Tauri build: sourceRoot=$(pwd)"
            echo "==> Frontend files available at: $(ls ../src/ 2>/dev/null || echo 'NOT FOUND')"
          '';

          # Disable the test phase — Tauri integration tests require a display server
          # and a live D-Bus session; neither is available in the Nix sandbox.
          doCheck = false;

          # Post-install: copy frontend assets and set up font symlinks, then wrap binary.
          postInstall = ''
            # ── Frontend static files ─────────────────────────────────────────
            # Install HTML/CSS/JS assets where Tauri's resource resolver expects them.
            mkdir -p $out/share/github-export
            cp -r ../src/. $out/share/github-export/

            # ── Liberation Fonts for PDF export (genpdf) ─────────────────────
            mkdir -p $out/share/fonts/github-export
            for f in ${pkgs.liberation_ttf}/share/fonts/truetype/liberation/LiberationSans-*.ttf; do
              ln -sf "$f" "$out/share/fonts/github-export/$(basename "$f")"
            done

          '';

          # Use preFixup so wrapGAppsHook3's postFixup wrapper picks up our extra
          # args — this avoids double-wrapping the binary (wrapGAppsHook3 already
          # calls wrapProgram in its own postFixup hook; calling wrapProgram again
          # in postInstall would nest a second wrapper around it).
          preFixup = ''
            gappsWrapperArgs+=(
              "--set-default" "WEBKIT_DISABLE_COMPOSITING_MODE" "1"
              "--set" "LIBERATION_FONTS_DIR" "$out/share/fonts/github-export"
              "--prefix" "XDG_DATA_DIRS" ":" "${pkgs.gsettings-desktop-schemas}/share/gsettings-schemas/${pkgs.gsettings-desktop-schemas.name}"
              "--prefix" "XDG_DATA_DIRS" ":" "${pkgs.gtk3}/share/gsettings-schemas/${pkgs.gtk3.name}"
              "--prefix" "XDG_DATA_DIRS" ":" "$out/share"
              "--prefix" "LD_LIBRARY_PATH" ":" "${pkgs.lib.makeLibraryPath runtimeLibs}"
            )
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

        # ── Apps (runnable via `nix run`) ─────────────────────────────────────
        apps = {
          default = flake-utils.lib.mkApp {
            drv = github-export;
            name = "github-export";
          };
          github-export = flake-utils.lib.mkApp {
            drv = github-export;
            name = "github-export";
          };
        };

        # ── Development Shell ─────────────────────────────────────────────────
        # Enter with: nix develop
        # Then: npm install && npm run dev
        devShells.default = pkgs.mkShell {
          name = "github-export-dev";

          # Build-time and runtime dependencies for development
          buildInputs = runtimeLibs ++ nativeBuildDeps ++ (with pkgs; [
            # Rust toolchain
            rustc
            cargo
            rust-analyzer
            clippy
            rustfmt
            cargo-audit    # Security advisory checks (`cargo audit`)
            cargo-watch    # Auto-rebuild on file changes (`cargo watch -x run`)

            # Node.js for Tauri CLI (npm run dev / npm run build)
            nodejs_20
            nodePackages.npm

            # Font files for PDF export development and testing
            liberation_ttf

            # Common dev utilities
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

            # Liberation fonts for PDF export development
            export LIBERATION_FONTS_DIR="${pkgs.liberation_ttf}/share/fonts/truetype/liberation"

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
