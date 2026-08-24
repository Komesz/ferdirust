{
  description = "Ferdirust — Rust + CEF multi-messenger (Telegram / Slack / Messenger / Proton Mail)";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-25.11";

  outputs = { self, nixpkgs }:
  let
    system = "x86_64-linux";
    pkgs = nixpkgs.legacyPackages.${system};

    # CEF binary distribution. The exact version is pinned to what
    # `cef-dll-sys` downloads during cargo build. To bump:
    #   1. cargo build --release locally
    #   2. ls target/release/build/cef-dll-sys-*/out/  → note the tarball name
    #   3. update url + hash here
    cefSrc = pkgs.fetchurl {
      url = "https://cef-builds.spotifycdn.com/cef_binary_145.0.28+g51162e8+chromium-145.0.7632.160_linux64_minimal.tar.bz2";
      hash = "sha256-Blx/dEL08pF6lcP96AUsuH4eL3zpYoZMgyWxLlLsOfQ=";
    };

    # Lay out the prebuilt CEF in the exact dir + sentinel-file structure that
    # cef-dll-sys's build.rs (https://github.com/tauri-apps/cef-rs/blob/dev/sys/build.rs)
    # expects via `CEF_PATH`. The build script then skips its own download.
    #
    # Required layout:
    #   $CEF_PATH/<cef_version>/<os_arch>/        ← extracted tarball contents
    #   $CEF_PATH/<cef_version>/<os_arch>/archive.json
    #
    # `<cef_version>` here = "145.0.28" (the build-metadata part of the
    # cef-dll-sys crate version 145.6.1+145.0.28, per `unwrap_cef_version`).
    cefVersion = "145.0.28";
    cefArchiveName = "cef_binary_145.0.28+g51162e8+chromium-145.0.7632.160_linux64_minimal";

    cefBinary = pkgs.stdenv.mkDerivation {
      pname = "cef-binary";
      version = cefVersion;
      src = cefSrc;
      dontConfigure = true;
      dontBuild = true;
      dontStrip = true;
      # cef-dll-sys 145.6.1 expects FLAT layout: archive.json at top level
      # plus all CEF files at top level. (The versioned-subdir layout is
      # only in cef-rs main branch / future releases.)
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        # Tarball extracts to a single dir named cefArchiveName/ — flatten it.
        if [ -d ${cefArchiveName} ]; then
          cp -r ${cefArchiveName}/. $out/
        else
          cp -r . $out/
        fi
        # cef-dll-sys emits `-L$CEF_PATH -lcef`, so libcef.so must be at $out
        # top level. The minimal tarball ships it in Release/ — link up.
        if [ -d $out/Release ]; then
          for f in $out/Release/*; do
            name=$(basename "$f")
            [ -e "$out/$name" ] || ln -s "Release/$name" "$out/$name"
          done
        fi
        # Sentinel file cef-dll-sys's check_archive_json validates.
        cat > $out/archive.json <<EOF
        {
          "type": "minimal",
          "name": "${cefArchiveName}",
          "sha1": "0000000000000000000000000000000000000000"
        }
        EOF
        runHook postInstall
      '';
    };

    runtimeLibs = with pkgs; [
      gtk3 glib atk at-spi2-atk at-spi2-core cairo pango gdk-pixbuf
      nss nspr dbus expat libdrm libgbm mesa
      alsa-lib libpulseaudio pipewire fontconfig freetype
      libxkbcommon vulkan-loader libGL
      cups
      systemd        # provides libudev
      xorg.libX11 xorg.libXcomposite xorg.libXcursor xorg.libXdamage
      xorg.libXext xorg.libXfixes xorg.libXi xorg.libXrandr xorg.libXrender
      xorg.libXScrnSaver xorg.libXtst xorg.libxcb xorg.libxkbfile
    ];
  in {
    packages.${system}.default = pkgs.rustPlatform.buildRustPackage {
      pname = "ferdirust";
      version = "0.1.0";
      src = ./.;
      cargoLock.lockFile = ./Cargo.lock;

      nativeBuildInputs = with pkgs; [
        pkg-config
        makeWrapper
        autoPatchelfHook
      ];

      buildInputs = runtimeLibs;

      # Pre-fetched CEF — cef-dll-sys (https://github.com/tauri-apps/cef-rs)
      # reads CEF_PATH and skips its network download when the expected
      # archive.json sentinel is at the top level (see cefBinary above).
      CEF_PATH = "${cefBinary}";

      # libcef.so transitively references many system libs. cef-dll-sys only
      # emits `-lcef`, so list the rest here. Add to this list when a new
      # `undefined reference to` error appears for a known lib symbol.
      NIX_LDFLAGS = builtins.concatStringsSep " " [
        "-lglib-2.0" "-lgio-2.0" "-lgobject-2.0"
        "-lgtk-3" "-lgdk-3"
        "-lpango-1.0" "-lpangocairo-1.0"
        "-lcairo"
        "-lgdk_pixbuf-2.0"
        "-latk-1.0" "-latk-bridge-2.0" "-latspi"
        "-lnss3" "-lnssutil3" "-lsmime3" "-lssl3"
        "-lnspr4" "-lplc4" "-lplds4"
        "-lexpat"
        "-lfontconfig" "-lfreetype"
        "-lxkbcommon"
        "-lvulkan"
        "-lpulse"
        "-lasound"
        "-ldbus-1"
        "-lcups"
        "-lgbm" "-ldrm"
        "-ludev"
        "-lX11" "-lXcomposite" "-lXcursor" "-lXdamage" "-lXext"
        "-lXfixes" "-lXi" "-lXrandr" "-lXrender" "-lXss" "-lXtst" "-lxcb"
      ];

      # The build.rs sets rustc-link-arg=-Wl,-rpath,$ORIGIN so the binary
      # looks for libcef.so next to itself. We lay everything out under
      # $out/lib/ferdirust/ and put a wrapper in $out/bin/.
      postInstall = ''
        install -d $out/lib/ferdirust $out/share/applications $out/share/pixmaps

        # Move the binary into the bundle dir
        mv $out/bin/ferdirust $out/lib/ferdirust/ferdirust

        # CEF runtime files: copy from the pre-fetched cefBinary layout
        # (${cefBinary}/ is flat — see cefBinary derivation above).
        cefDir=${cefBinary}
        cp -r $cefDir/Release/. $out/lib/ferdirust/ 2>/dev/null || true
        cp -r $cefDir/Resources/. $out/lib/ferdirust/ 2>/dev/null || true
        # Fall back: copy flattened top-level files for distributions that
        # don't use Release/Resources subdirs.
        for f in libcef.so libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1 \
                 v8_context_snapshot.bin icudtl.dat \
                 chrome_100_percent.pak chrome_200_percent.pak resources.pak \
                 vk_swiftshader_icd.json chrome-sandbox; do
          if [ -f $cefDir/$f ] && [ ! -e $out/lib/ferdirust/$f ]; then
            cp $cefDir/$f $out/lib/ferdirust/
          fi
        done
        if [ -d $cefDir/locales ] && [ ! -d $out/lib/ferdirust/locales ]; then
          cp -r $cefDir/locales $out/lib/ferdirust/
        fi

        # chrome-sandbox needs setuid in real installs, but Nix store is
        # read-only — leave it as-is; users can copy + chmod if they need it.

        # Bundled resources
        cp ${./resources/icon.svg} $out/lib/ferdirust/icon.svg
        cp -r ${./resources/scripts} $out/lib/ferdirust/scripts
        cp $out/lib/ferdirust/icon.svg $out/share/pixmaps/ferdirust.svg

        # Wrapper entry point
        makeWrapper $out/lib/ferdirust/ferdirust $out/bin/ferdirust \
          --prefix LD_LIBRARY_PATH : "$out/lib/ferdirust:${pkgs.lib.makeLibraryPath runtimeLibs}"

        # .desktop file
        cat > $out/share/applications/ferdirust.desktop <<EOF
        [Desktop Entry]
        Name=Ferdirust
        Comment=Multi-messenger (Telegram / Slack / Messenger / Proton Mail)
        Exec=$out/bin/ferdirust
        Icon=ferdirust
        Terminal=false
        Type=Application
        Categories=Network;InstantMessaging;
        StartupWMClass=ferdirust
        EOF
      '';

      meta = with pkgs.lib; {
        description = "Multi-messenger built on CEF";
        homepage = "https://github.com/Komesz/ferdirust";
        platforms = [ "x86_64-linux" ];
        mainProgram = "ferdirust";
      };
    };

    # Dev shell: `nix develop` gives a shell with all build deps + CEF on PATH
    # so you can `cargo build` and `./bundle.sh` normally.
    devShells.${system}.default = pkgs.mkShell {
      buildInputs = runtimeLibs ++ [ pkgs.rustc pkgs.cargo pkgs.pkg-config ];
      shellHook = ''
        export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath runtimeLibs}:$LD_LIBRARY_PATH"
      '';
    };
  };
}
