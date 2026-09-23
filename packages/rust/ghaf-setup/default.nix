# SPDX-FileCopyrightText: 2026 TII (SSRC) and the Ghaf contributors
# SPDX-License-Identifier: Apache-2.0
{
  lib,
  pkgs,
  crane,
}:
let
  craneLib = crane.mkLib pkgs;

  commonArgs = {
    src = ./.;
    strictDeps = true;

    pname = "ghaf-setup";
    version = "0.1.0";

    nativeBuildInputs = with pkgs; [
      pkg-config
      libcosmicAppHook
    ];

    CARGO_BUILD_INCREMENTAL = "false";
    RUST_BACKTRACE = "1";

    # Pin the tree hash of every git dependency in Cargo.lock so crane uses
    # substitutable fetchgit derivations instead of builtins.fetchGit. Seeded
    # from ghaf-usb-passthrough-applet, which pins the same libcosmic
    # revision; adjusted for any hash mismatch crane reports.
    outputHashes = {
      "git+https://github.com/iced-rs/cryoglyph.git?rev=e429a025df36ab8145708acb309080ae3deec17a#e429a025df36ab8145708acb309080ae3deec17a" =
        "sha256-10JUHl1ktbqLaReuiU3HPa4r2KvsoryyJoF3BFoge3U=";
      "git+https://github.com/jackpot51/rust-atomicwrites#043ab4859d53ffd3d55334685303d8df39c9f768" =
        "sha256-QZSuGPrJXh+svMeFWqAXoqZQxLq/WfIiamqvjJNVhxA=";
      "git+https://github.com/pop-os/cosmic-panel#f416dbbe72d8600d395d24bf9f96f6ba1299d13b" =
        "sha256-3nIlsngBTH7WZGVGjSeVvJUY3rWvORnfby0TfgiRWLg=";
      "git+https://github.com/pop-os/cosmic-protocols?rev=32283d7#32283d76a8d0342da74c4cc022a533c52dcf378f" =
        "sha256-LUAmB+3+doRZOJbVURaIInaQuV/LXCKfoWHA28ihAMo=";
      "git+https://github.com/pop-os/dbus-settings-bindings#eed01dd3609e90e3c8cd043656734c500956c793" =
        "sha256-LYIR+qK+hCBVV+bfVWz2jvH5fGvfNTcryKqfe5n8Gog=";
      "git+https://github.com/pop-os/freedesktop-icons#ab4c57b8e416c6af9297cb04d101889896fd9a92" =
        "sha256-tPriTi5L0mFMHjo5xpF5cmKGHqlX3WUO7EZAgVdBpS4=";
      "git+https://github.com/pop-os/libcosmic?rev=1f6dc991eaa5115a09be2505cc8a323e5b2b0bff#1f6dc991eaa5115a09be2505cc8a323e5b2b0bff" =
        "sha256-sPOXJiukAdOvRJ5cet0ec+0LwGeoLBtkmZLSzBzKcnA=";
      "git+https://github.com/pop-os/smithay-clipboard?tag=sctk-0.20#859b02c88f45c554049a67c6ddeec1692ce0e20b" =
        "sha256-GojAFRbhJcP0Rpr+v9WOivgW9x38PZdeBWTbMhkDB3A=";
      "git+https://github.com/pop-os/softbuffer?tag=cosmic-4.0#c2b2c19ddb38ff17495643699f97cb1f2064a1be" =
        "sha256-9Ret/nfieBFl4yJ9TddyWsSuS7sI4QAza/TZrxYMb+I=";
      "git+https://github.com/pop-os/window_clipboard.git?tag=sctk-0.20#f68595ee0e62fbd6589f4709b5aaa5c3c7ea5f6c" =
        "sha256-WO3JFbE+6ESRAfkxrnEFeZyGuhUHLOKOVHcGQyHwoK0=";
      "git+https://github.com/pop-os/winit.git?tag=cosmic-0.14#71ce08c043814514a8fd92d9d0599f115ae854e8" =
        "sha256-8r9O5RgVa8vxkPPYvr2aQiRdZ4isg7Jdnk8O5gQIr9k=";
      "git+https://github.com/wash2/accesskit?tag=cosmic-0.14#f0599eed5f18111228266fe3f28991cc48b5964f" =
        "sha256-pP9CyiV1zIONQ7vbl5MkMtilemSPrHaZ0c/SyR+lb0k=";
    };
  };

  cargoArtifacts = craneLib.buildDepsOnly commonArgs;

  cargoTest = craneLib.cargoTest (commonArgs // { inherit cargoArtifacts; });

  cargoClippy = craneLib.cargoClippy (
    commonArgs
    // {
      inherit cargoArtifacts;
      cargoClippyExtraArgs = "--all-targets -- --deny warnings";
    }
  );
in
craneLib.buildPackage (
  commonArgs
  // {
    inherit cargoArtifacts;

    # Both binaries come from one workspace build: dependencies are compiled
    # once, and the ISO and the gui-vm each install only the binary they need.
    cargoExtraArgs = "--workspace --bins";

    # The programs ghaf-setup-core runs by name; a systemd unit's PATH has none of them.
    preFixup = ''
      libcosmicAppWrapperArgs+=(--prefix PATH : ${
        lib.makeBinPath (
          with pkgs;
          [
            bmaptool
            coreutils
            efibootmgr
            efitools
            lvm2
            parted
            systemd
            util-linux
            zstd
          ]
        )
      })
    '';

    passthru.tests = {
      inherit cargoTest cargoClippy;
    };

    meta = {
      description = "COSMIC setup wizards for Ghaf installation and first boot";
      longDescription = ''
        libcosmic wizards for installing Ghaf to a disk and for creating the
        desktop login account on first boot. The logic lives in a UI-free core
        crate that invokes system tools as subprocesses.
      '';
      homepage = "https://github.com/tiiuae/ghafpkgs";
      license = lib.licenses.asl20;
      platforms = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      mainProgram = "ghaf-installer-gui";
    };
  }
)
