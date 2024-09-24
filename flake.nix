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
			packages = with pkgs; [
				bacon
				cargo
				clang
				mold
				rustc
				sccache
			];
		};
	};
}
