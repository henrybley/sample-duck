{
  description = "Rust eframe/egui development environment with Fenix";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, nixpkgs, flake-utils, fenix }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = nixpkgs.legacyPackages.${system};
        
        # Options: stable, beta, latest (nightly), or specific versions
        toolchain = fenix.packages.${system}.stable.toolchain;
        
      in
      {
        devShells.default = pkgs.mkShell {
          nativeBuildInputs = with pkgs; [
            pkg-config
            toolchain  # Use Fenix toolchain instead of nixpkgs rustc/cargo
          ];
          
          buildInputs = with pkgs; [
            # XKB and input handling
            xkeyboard_config
            libxkbcommon
            
            # Graphics and windowing
            libGL
            libGLU
            wayland
            wayland-protocols
            xorg.libX11
            xorg.libXcursor
            xorg.libXrandr
            xorg.libXi
            xorg.libXxf86vm
            
            # Font rendering
            fontconfig
            freetype
            
            # Audio (if needed)
            alsa-lib
          ];

          shellHook = ''
            export LD_LIBRARY_PATH="${pkgs.lib.makeLibraryPath [
              pkgs.libGL
              pkgs.libGLU
              pkgs.libxkbcommon
              pkgs.wayland
              pkgs.xorg.libX11
              pkgs.xorg.libXcursor
              pkgs.xorg.libXrandr
              pkgs.xorg.libXi
              pkgs.fontconfig
              pkgs.freetype
              pkgs.alsa-lib
            ]}:$LD_LIBRARY_PATH"
            
            export XKB_CONFIG_ROOT="${pkgs.xkeyboard_config}/share/X11/xkb"
            
            # Display Rust version info
            echo "Rust toolchain information:"
            rustc --version
            cargo --version
          '';
        };
        
        # Optional: Add a formatter for `nix fmt`
        formatter = pkgs.nixpkgs-fmt;
      });
}
