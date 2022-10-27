#!/bin/bash

# 注意，将 HOME 替换为实际的路径
HOME="/home/dell"
TARGET="riscv64gc-unknown-linux-gnu"
MODE="release"
BIN=target/$TARGET/$MODE/gw
CC="riscv64-unknown-linux-gnu-gcc"
AR="riscv64-unknown-linux-gnu-ar"
CFLAGS="-I$HOME/Tina-Linux/out/f133-mq_r/staging_dir/target/usr/include -I$HOME/Tina-Linux/out/f133-mq_r/compile_dir/target/openssl-1.1.0i/include"
TOOLCHAIN=~/Tina-Linux/out/f133-mq_r/staging_dir/toolchain
STRIP=riscv64-unknown-linux-gnu-strip

export PKG_CONFIG_LIBDIR=~/Tina-Linux/out/f133-mq_r/staging_dir/target/usr/lib/pkgconfig
export PKG_CONFIG_ALLOW_CROSS=1
export CC_riscv64gc_unknown_linux_gnu=$CC
export AR_riscv64gc_unknown_linux_gnu=$AR
export CFLAGS_riscv64gc_unknown_linux_gnu=$CFLAGS
export PATH=$PATH:$TOOLCHAIN/bin
export OPENSSL_INCLUDE_DIR=~/Tina-Linux/out/f133-mq_r/compile_dir/target/openssl-1.1.0i/include
export RISCV64GC_UNKNOWN_LINUX_GNU_OPENSSL_LIB_DIR=~/Tina-Linux/out/f133-mq_r/compile_dir/target/openssl-1.1.0i
cd ..
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN