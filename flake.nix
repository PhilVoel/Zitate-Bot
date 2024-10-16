{
	description = "Zitate-Bot shell flake";

	inputs = {
		nixpkgs.url = "github:nixos/nixpkgs/24.05";
		flake-utils.url = "github:numtide/flake-utils/v1.0.0";
	};

	outputs = {flake-utils, nixpkgs, self, ...}:
		flake-utils.lib.eachDefaultSystem (system:
			let pkgs = nixpkgs.legacyPackages."${system}";
			in {
				devShells.default = pkgs.mkShell {
					name = "Zitate-Bot shell flake";
					packages = with pkgs; [
						cargo
					];
				};
				packages.default = pkgs.rustPlatform.buildRustPackage {
					pname = "zitate_bot";
					version = "0.1.1";
					src = self;
					cargoLock.lockFile = ./Cargo.lock;
					buildType = "debug";
					doCheck = false;
				};
				packages.release = pkgs.rustPlatform.buildRustPackage {
					pname = "zitate_bot";
					version = "0.1.1";
					src = self;
					cargoLock.lockFile = ./Cargo.lock;
				};
			}
		);
}
