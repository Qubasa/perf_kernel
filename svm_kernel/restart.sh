#!/usr/bin/env bash
set -e


export CARGO_MANIFEST_DIR=$PWD
cargo build
ls src/*.rs external/bootloader/src/*.rs | entr -r -n sh -c "
set -e
sh -c 'bootimage runner --grub target/x86_64-os/debug/svm_kernel' &
sleep 5
echo '==Dumping core=='

pid=\$(pgrep svm_kernel)
name=\$(date -d 'today' +'%H_%M_%S')

echo \"Name: \$name Pid: \$pid\"
sudo ../vmsh/target/debug/kernel_inspector coredump \$pid target/\${name}.dump
python ../vmsh/tests/coredump_analyze.py target/\${name}.dump > target/dump.analysis &
sleep 5
rm target/\${name}.dump
echo Done
" 


