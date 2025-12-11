{
	description = "Zitate-Bot shell flake";

	inputs = {
		nixpkgs.url = "nixpkgs/nixos-25.11";
		flake-utils.url = "github:numtide/flake-utils/v1.0.0";
	};

	outputs = {flake-utils, nixpkgs, self, ...}:
		flake-utils.lib.eachDefaultSystem (system:
			let pkgs = nixpkgs.legacyPackages."${system}";
			in rec {
				devShells.default = pkgs.mkShell {
					name = "Zitate-Bot shell flake";
					packages = with pkgs; [
						cargo
						rustc
					];
				};
				packages.default = packages.release;
				packages.debug = pkgs.rustPlatform.buildRustPackage {
					pname = "zitate_bot";
					version = "0.2.0";
					src = self;
					cargoLock.lockFile = ./Cargo.lock;
					buildType = "debug";
					doCheck = false;
				};
				packages.release = pkgs.rustPlatform.buildRustPackage {
					pname = "zitate_bot";
					version = "0.2.0";
					src = self;
					cargoLock.lockFile = ./Cargo.lock;
				};
			}
		);
}
