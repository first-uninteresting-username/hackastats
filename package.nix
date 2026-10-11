# SPDX-FileCopyrightText: 2026 first-uninteresting-username
#
# SPDX-License-Identifier: GPL-2.0-or-later
{
  lib,
  stdenvNoCC,
  glib,
  version ? "unstable",
}: let
  extensionUuid = "hackastats@first-uninteresting-username.github.io";
in
  stdenvNoCC.mkDerivation {
    pname = "gnome-shell-extension-hackastats";
    inherit version;
    src = ./.;

    nativeBuildInputs = [glib];
    dontBuild = true;

    installPhase = ''
      runHook preInstall
      extensionDir="$out/share/gnome-shell/extensions/${extensionUuid}"
      mkdir -p "$extensionDir"
      cp -r ${extensionUuid}/. "$extensionDir/"
      glib-compile-schemas --strict "$extensionDir/schemas"
      runHook postInstall
    '';

    passthru = {inherit extensionUuid;};

    meta = {
      description = "Display daily Hackatime coding stats in the GNOME panel";
      homepage = "https://github.com/first-uninteresting-username/hackastats";
      license = lib.licenses.gpl2Plus;
      platforms = lib.platforms.linux;
    };
  }
