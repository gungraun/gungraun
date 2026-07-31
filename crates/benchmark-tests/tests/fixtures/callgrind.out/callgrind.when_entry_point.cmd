#!/bin/bash

cd $(dirname ${BASH_SOURCE[0]}) || exit 1

callgrind_annotate --threshold=100 --inclusive=yes callgrind.when_entry_point.out |\
    ../../../../scripts/callgrind_annotate_to_stacks.pl \
        --insert='0 (below main) [/usr/lib/libc.so.6] 3473' \
        --insert='0 (below main) [target/release/benchmark-tests-exit] 3473' \
        --add-missing-ob='target/release/benchmark-tests-exit' \
        --replace='/usr/src/debug/gcc/gcc/libgcc/unwind-dw2-btree.h:release_registered_frames [target/release/benchmark-tests-exit]==>>/usr/src/debug/gcc/gcc/libgcc/unwind-dw2-btree.h:release_registered_frames [/usr/lib/libgcc_s.so.1]' > callgrind.when_entry_point.exp_stacks
