{
  description = "A Nix-flake-based Rust development environment";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = { self, nixpkgs, ...}:
  let
    system = "x86_64-linux";
  in {
    devShells."${system}".default =
    let
      pkgs = import nixpkgs { inherit system; };
    in pkgs.mkShell
    {
      strictDepts = true;
      nativeBuildInputs = with pkgs; [
        cargo
        rustc
      ];

      shellHook = ''
        exec zsh
      '';
    };
  };
}
