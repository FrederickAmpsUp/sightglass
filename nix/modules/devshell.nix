{ inputs, ... }:
{
  perSystem = { config, self', pkgs, lib, ... }: {
    devShells.default = pkgs.mkShell {
      name = "sightglass-shell";
      inputsFrom = [
        self'.devShells.rust
        config.pre-commit.devShell # See ./nix/modules/pre-commit.nix
      ];
      packages = with pkgs; [
        just
        nixd # Nix language server
        bacon
        
        (rust-bin.stable.latest.default.override {
          targets = [ "wasm32-unknown-unknown" ];
        })

        pkg-config
        wayland
        libxkbcommon
        vulkan-loader
        vulkan-headers
      ];

      LD_LIBRARY_PATH = pkgs.lib.makeLibraryPath (with pkgs; [
        wayland
        libxkbcommon
        vulkan-loader
      ]);

      shellHook = ''
        export PATH="$HOME/.cargo/bin:$PATH"
      '';
    };
  };
}
