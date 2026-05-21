{
	description = "Zitate-Bot shell flake";

	inputs = {
		nixpkgs.url = "nixpkgs/nixos-25.11";
		flake-utils.url = "github:numtide/flake-utils/v1.0.0";
		crate2nix.url = "github:nix-community/crate2nix/0.15.0";
	};

	outputs = {crate2nix, flake-utils, nixpkgs, ...}:
		flake-utils.lib.eachDefaultSystem (system:
			let
				pkgs = nixpkgs.legacyPackages."${system}";
				cargoNix = crate2nix.tools.${system}.appliedCargoNix {
					name = "zitate_bot";
					src = ./.;
				};
			in {
				devShells.default = pkgs.mkShell {
					name = "Zitate-Bot shell flake";
					packages = with pkgs; [
						cargo
						rustc
					];
				};

				packages.default = cargoNix.rootCrate.build;
			}
		);
}
