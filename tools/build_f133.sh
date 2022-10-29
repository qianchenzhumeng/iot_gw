#!/bin/bash

TARGET="riscv64gc-unknown-linux-gnu"
MODE="release"
BIN=target/$TARGET/$MODE/gw
CC="riscv64-unknown-linux-gnu-gcc"
AR="riscv64-unknown-linux-gnu-ar"
# 注意，需要改为实际工具链所在的路径
TOOLCHAIN=~/Tina-Linux/out/f133-mq_r/staging_dir/toolchain
# 注意，需要改为实际 libz.h 所在的路径
CFLAGS="-I/home/dell/Tina-Linux/out/f133-mq_r/staging_dir/target/usr/include"
STRIP=riscv64-unknown-linux-gnu-strip

export CC_riscv64gc_unknown_linux_gnu=$CC
export AR_riscv64gc_unknown_linux_gnu=$AR
export CFLAGS_riscv64gc_unknown_linux_gnu=$CFLAGS
export PATH=$PATH:$TOOLCHAIN/bin

cd ..
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN