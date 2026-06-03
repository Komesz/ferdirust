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

    cefBinary = pkgs.stdenv.mkDerivation {
      pname = "cef-binary";
      version = "145.0.28";
      src = cefSrc;
      dontConfigure = true;
      dontBuild = true;
      dontStrip = true;
      installPhase = ''
        runHook preInstall
        mkdir -p $out
        cp -r . $out/
        runHook postInstall
      '';
    };

    runtimeLibs = with pkgs; [
      gtk3 glib atk cairo pango gdk-pixbuf
      nss nspr dbus expat libdrm libgbm mesa
      alsa-lib libpulseaudio fontconfig freetype
      libxkbcommon vulkan-loader libGL
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

      # Override cef-dll-sys's network download by providing CEF up front.
      # If cef-dll-sys insists on downloading anyway, we'll need __noChroot
      # or a postPatch to short-circuit its build script.
      DEP_CEF_CEF_DIR = "${cefBinary}";
      CEF_DIR        = "${cefBinary}";

      # The build.rs sets rustc-link-arg=-Wl,-rpath,$ORIGIN so the binary
      # looks for libcef.so next to itself. We lay everything out under
      # $out/lib/ferdirust/ and put a wrapper in $out/bin/.
      postInstall = ''
        install -d $out/lib/ferdirust $out/share/applications $out/share/pixmaps

        # Move the binary into the bundle dir
        mv $out/bin/ferdirust $out/lib/ferdirust/ferdirust

        # Copy CEF runtime files alongside
        cp -r ${cefBinary}/Release/. $out/lib/ferdirust/ 2>/dev/null || true
        cp -r ${cefBinary}/Resources/. $out/lib/ferdirust/ 2>/dev/null || true
        # Some CEF distributions flatten; fall back to top-level
        for f in libcef.so libEGL.so libGLESv2.so libvk_swiftshader.so libvulkan.so.1 \
                 v8_context_snapshot.bin icudtl.dat \
                 chrome_100_percent.pak chrome_200_percent.pak resources.pak; do
          if [ -f ${cefBinary}/$f ] && [ ! -e $out/lib/ferdirust/$f ]; then
            cp ${cefBinary}/$f $out/lib/ferdirust/
          fi
        done
        if [ -d ${cefBinary}/locales ] && [ ! -d $out/lib/ferdirust/locales ]; then
          cp -r ${cefBinary}/locales $out/lib/ferdirust/
        fi

        # chrome-sandbox needs setuid in real installs, but Nix store is
        # read-only — leave it as-is; users can copy + chmod if they need it.

        # Bundled resources
        cp ${./resources/icon.svg} $out/lib/ferdirust/icon.svg
        cp -r ${./resources/scripts} $out/lib/ferdirust/scripts
        cp $out/lib/ferdirust/icon.svg $out/share/pixmaps/ferdirust.svg

        # Wrapper entry point
        makeWrapper $out/lib/ferdirust/ferdirust $out/bin/ferdirust \
          --prefix LD_LIBRARY_PATH : "$out/lib/ferdirust"

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
