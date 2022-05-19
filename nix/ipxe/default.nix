{ stdenv, lib, fetchgit, fetchFromGitHub, perl, cdrkit, xz, openssl, gnu-efi, mtools
, syslinux ? null
, embedScript ? null
, additionalTargets ? {}
}:

let
  targets = additionalTargets // lib.optionalAttrs stdenv.isx86_64 {
    "bin-x86_64-efi/ipxe.efi" = null;
    "bin-x86_64-efi/ipxe.efirom" = null;
    "bin-x86_64-efi/ipxe.usb" = "ipxe-efi.usb";
  } // lib.optionalAttrs stdenv.hostPlatform.isx86 {
    "bin/ipxe.dsk" = null;
    "bin/ipxe.usb" = null;
    "bin/ipxe.lkrn" = null;
    "bin/undionly.kpxe" = null;
  } // lib.optionalAttrs stdenv.isAarch32 {
    "bin-arm32-efi/ipxe.efi" = null;
    "bin-arm32-efi/ipxe.efirom" = null;
    "bin-arm32-efi/ipxe.usb" = "ipxe-efi.usb";
  } // lib.optionalAttrs stdenv.isAarch64 {
    "bin-arm64-efi/ipxe.efi" = null;
    "bin-arm64-efi/ipxe.efirom" = null;
    "bin-arm64-efi/ipxe.usb" = "ipxe-efi.usb";
  };
in

stdenv.mkDerivation rec {
  pname = "ipxe";
  version = "1.21.1";

  nativeBuildInputs = [ perl cdrkit xz openssl gnu-efi mtools ] ++ lib.optional stdenv.hostPlatform.isx86 syslinux;

  src = fetchgit {
    url = https://git.tu-berlin.de/luishebendanz/ipxe;
    branchName = "multibootv2";
    rev = "b992d767355eff5db41735ffe1dcc475790fad52";
    sha256 = "sha256-VRxH69KUPGHFaGTtAQguJ+DiqwPaNfmxXzXxE1B5shQ=";
  };
 
  # not possible due to assembler code
  hardeningDisable = [ "pic" "stackprotector" ];

  NIX_CFLAGS_COMPILE = "-Wno-error";

  makeFlags =
    [ "ECHO_E_BIN_ECHO=echo" "ECHO_E_BIN_ECHO_E=echo" # No /bin/echo here.
      "DEBUG=multiboot:3,image:3,multibootv2:3"
    ] ++ lib.optionals stdenv.hostPlatform.isx86 [
      "ISOLINUX_BIN_LIST=${syslinux}/share/syslinux/isolinux.bin"
      "LDLINUX_C32=${syslinux}/share/syslinux/ldlinux.c32"
    ] ++ lib.optional (embedScript != null) "EMBED=${embedScript}";


  enabledOptions = [
    "PING_CMD"
    "IMAGE_TRUST_CMD"
    "DOWNLOAD_PROTO_HTTP"
    "DOWNLOAD_PROTO_HTTPS"
  ];

  configurePhase = ''
    runHook preConfigure
    for opt in ${lib.escapeShellArgs enabledOptions}; do echo "#define $opt" >> src/config/general.h; done
    substituteInPlace src/Makefile.housekeeping --replace '/bin/echo' echo
    runHook postConfigure
  '';

  preBuild = "cd src";

  buildFlags = lib.attrNames targets;

  installPhase = ''
    runHook preInstall
    mkdir -p $out
    ${lib.concatStringsSep "\n" (lib.mapAttrsToList (from: to:
      if to == null
      then "cp -v ${from} $out"
      else "cp -v ${from} $out/${to}") targets)}
    # Some PXE constellations especially with dnsmasq are looking for the file with .0 ending
    # let's provide it as a symlink to be compatible in this case.
    ln -s undionly.kpxe $out/undionly.kpxe.0
    runHook postInstall
  '';

  enableParallelBuilding = true;

  meta = with lib;
    { description = "Network boot firmware";
      homepage = "https://ipxe.org/";
      license = licenses.gpl2Only;
      maintainers = with maintainers; [ ehmry ];
      platforms = platforms.linux;
    };
}
