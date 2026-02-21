{
  inputs = {
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nix-github-actions = {
      url = "github:nix-community/nix-github-actions";
      inputs.nixpkgs.follows = "nixpkgs";
    };
    nixpkgs.url = "nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    fenix,
    naersk,
    nixpkgs,
    nix-github-actions,
    ...
  }: let
    desktopTargets = ["x86_64-unknown-linux-gnu" "aarch64-unknown-linux-gnu"];
    androidTargets = ["x86_64-linux-android" "aarch64-linux-android"];
    androidApi = 23;
    buildSystem = "x86_64-linux";
  in {
    githubActions = nix-github-actions.lib.mkGithubMatrix {checks = self.packages;};
    packages.${buildSystem} = let
      pkgs = (import nixpkgs) {
        system = buildSystem;
        config.allowUnfree = true;
      };
      lib = pkgs.lib;
      toolchain = with fenix.packages.${buildSystem};
        combine ([
            minimal.cargo
            minimal.rustc
          ]
          ++ (map (target: fenix.packages.${buildSystem}.targets.${target}.latest.rust-std) (androidTargets ++ desktopTargets)));
      arch = builtins.head (builtins.split "-" buildSystem);
    in let
      naersk' = naersk.lib.${buildSystem}.override {
        cargo = toolchain;
        rustc = toolchain;
      };
    in let
      allTargets =
        (map (target: {
            inherit target;
            linker = "${pkgs.androidenv.androidPkgs.ndk-bundle}/libexec/android-sdk/ndk-bundle/toolchains/llvm/prebuilt/linux-${arch}/bin/${target}${builtins.toString androidApi}-clang";
          })
          androidTargets)
        ++ (map (target: let
          in let
            aarch64 = let
              inherit (pkgs.pkgsCross.aarch64-multiplatform.stdenv) cc;
            in "${cc}/bin/${cc.targetPrefix}cc";
          in {
            inherit target;
            linker =
              {
                "aarch64-unknown-linux-gnu" = aarch64;
                "x86_64-unknown-linux-gnu" = "${pkgs.stdenv.cc}/bin/cc";
              }.${
                target
              };
          })
          desktopTargets);

      packages = builtins.listToAttrs (map (targetInfo: {
          name = targetInfo.target;
          value = naersk'.buildPackage {
            src = ./.;
            copyLibs = true;
            copyBins = false;
            CARGO_BUILD_TARGET = targetInfo.target;
            "CARGO_TARGET_${builtins.replaceStrings ["-"] ["_"] (lib.toUpper targetInfo.target)}_LINKER" = targetInfo.linker;
          };
        })
        allTargets);
    in
      packages;
  };
}
