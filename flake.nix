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
			shellHook = ''
				export NIX_LDFLAGS="$(echo $NIX_LDFLAGS | sed "s/\/home\/philipp\/Programming\/Zitate-Bot/file:\/\/\/home\/philipp\/Programming\/Zitate-Bot/g")"
			'';
			PROMPT="%F{cyan}%n%F{blue}%F{cyan}%m%F{blue}:%F{magenta}%~ ";
		};
	};
}
