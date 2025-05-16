# Copyright 2025 TII (SSRC) and the Ghaf contributors
# SPDX-License-Identifier: Apache-2.0
{ stdenv, lib }:

stdenv.mkDerivation {
  pname = "ghaf-wallpapers";

  version = "0.1.0";

  src = ./wallpapers;

  installPhase = ''
    mkdir -p $out/share/backgrounds/ghaf
    for file in $src/*; do
      install -D -m 0644 "$file" "$out/share/backgrounds/ghaf/$(basename "$file")"
    done
  '';

  meta = with lib; {
    description = "Wallpaper backgrounds for Ghaf";

    license = {
      fullName = "Unsplash License";
      url = "https://unsplash.com/license";
      free = true;
    };

    platforms = platforms.linux;
  };
}
