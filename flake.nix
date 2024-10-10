{
	description = "Zitate-Bot shell flake";

	inputs = {
		nixpkgs.url = "github:nixos/nixpkgs/24.05";
		flake-utils.url = "github:numtide/flake-utils/v1.0.0";
	};

	outputs = {flake-utils, nixpkgs, ...}:
		flake-utils.lib.eachDefaultSystem (system:
			let pkgs = nixpkgs.legacyPackages."${system}";
			in {
				devShells.default = pkgs.mkShell {
					name = "Zitate-Bot shell flake";
					packages = with pkgs; [
						bacon
						cargo
						clang
						mold
						rustc
						sccache
					];
				};
			}
		);
}
