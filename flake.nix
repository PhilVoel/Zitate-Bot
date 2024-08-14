{
	description = "Zitate-Bot shell flake";
	inputs.nixpkgs.url = "github:nixos/nixpkgs/24.05";

	outputs = {nixpkgs, ...}:
	let
		system = "x86_64-linux";
		pkgs = nixpkgs.legacyPackages."${system}".pkgs;
	in {
		devShells."${system}".default = pkgs.mkShell {
			name = "Zitate-Bot shell flake";
			buildInputs = with pkgs; [
				bacon
				cargo
				clang
				mold
				rustc
				sccache
			];
			SHELL_FLAKE_PATH="\\/home\\/philipp\\/Programming\\/Zitate-Bot";
			SHELL_FLAKE_PATH_NO_SPACES="file:\\/\\/\\/home\\/philipp\\/Programming\\/Zitate-Bot";
		};
	};
}
