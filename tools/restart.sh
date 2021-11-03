#!/usr/bin/env bash
set -e


export CARGO_MANIFEST_DIR=$PWD/kernel

if [ ! -d "$CARGO_MANIFEST_DIR" ]; then
    echo "Couldn't find kernel project under path: $CARGO_MANIFEST_DIR"
    exit 1
fi


find "$CARGO_MANIFEST_DIR" -iname "*.rs" | entr -r -n sh -c "
set -e
cd $CARGO_MANIFEST_DIR
sh -c 'cargo run' &

sleep 10
pid=\$(pgrep perf_kernel)
name=\$(date -d 'today' +'%H_%M_%S')

echo \"Name: \$name Pid: \$pid\"
#sudo vmsh/target/debug/kernel_inspector coredump \$pid target/\${name}.dump
#python vmsh/tests/coredump_analyze.py target/\${name}.dump > target/dump.analysis &
#sleep 5
#rm target/\${name}.dump
#echo Done
" 


