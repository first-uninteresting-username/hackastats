# SPDX-FileCopyrightText: 2026 first-uninteresting-username
#
# SPDX-License-Identifier: GPL-2.0-or-later
{
  description = "HackaStats GNOME Shell extension";

  inputs.nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";

  outputs = {
    self,
    nixpkgs,
  }: let
    systems = ["x86_64-linux" "aarch64-linux"];
    forAllSystems = nixpkgs.lib.genAttrs systems;
  in {
    packages = forAllSystems (system: let
      pkgs = nixpkgs.legacyPackages.${system};
      hackastats = pkgs.callPackage ./package.nix {
        version = "unstable-${self.shortRev or "dirty"}";
      };
    in {
      inherit hackastats;
      default = hackastats;
    });

    checks = forAllSystems (system: {
      inherit (self.packages.${system}) hackastats;
    });

    formatter = forAllSystems (system: nixpkgs.legacyPackages.${system}.alejandra);
  };
}
