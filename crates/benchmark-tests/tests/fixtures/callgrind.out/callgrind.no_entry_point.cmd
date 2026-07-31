#!/bin/bash

cd $(dirname ${BASH_SOURCE[0]}) || exit 1

callgrind_annotate --threshold=100 --inclusive=yes callgrind.no_entry_point.out |\
    ../../../../scripts/callgrind_annotate_to_stacks.pl \
        --sentinel=0x000000000001b530 \
        --insert='0 (below main) [/usr/lib/libc.so.6] 103210' \
        --modify='(below main) [target/release/benchmark-tests-exit] 103222' \
        --modify='0x0000000000005040 [/usr/lib/ld-linux-x86-64.so.2] 806' \
        --insert='0 0x0000000000005040 [/usr/lib/libgcc_s.so.1] 5' \
        --add-missing-ob='target/release/benchmark-tests-exit' \
        --replace='/usr/src/debug/gcc/gcc/libgcc/../gcc/common/config/i386/cpuinfo.h:__cpu_indicator_init@GCC_4.8.0 [target/release/benchmark-tests-exit]==>>/usr/src/debug/gcc/gcc/libgcc/../gcc/common/config/i386/cpuinfo.h:__cpu_indicator_init@GCC_4.8.0 [/usr/lib/libgcc_s.so.1]' \
        --replace='/usr/src/debug/gcc/gcc-build/gcc/include/cpuid.h:__cpu_indicator_init@GCC_4.8.0 [target/release/benchmark-tests-exit]==>>/usr/src/debug/gcc/gcc-build/gcc/include/cpuid.h:__cpu_indicator_init@GCC_4.8.0 [/usr/lib/libgcc_s.so.1]' \
        --replace='/usr/src/debug/gcc/gcc/libgcc/unwind-dw2-btree.h:release_registered_frames [target/release/benchmark-tests-exit]==>>/usr/src/debug/gcc/gcc/libgcc/unwind-dw2-btree.h:release_registered_frames [/usr/lib/libgcc_s.so.1]' \
    > callgrind.no_entry_point.exp_stacks
