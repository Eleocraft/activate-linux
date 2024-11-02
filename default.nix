{ lib, rustPlatform, libxkbcommon, pkg-config }:
rustPlatform.buildRustPackage {
  pname = "activate-linux";
  version = "0.1.0";
  src = ./.;
  cargoHash = "sha256-E9jYdvjXlOi+PNvpYaLHDfdQsouBY2Pxj0WDdBEvHjc=";
  buildInputs = [ libxkbcommon ];
  nativeBuildInputs = [ pkg-config ];

  meta = with lib; {
    description = ''Windows' "Active Windows" watermark for Linux '';
    homepage = "https://github.com/Perigord-Kleisli/activate-linux";
    mainProgram = "activate-linux";
    platforms = platforms.linux;
    license = licenses.mit;
  };
}
