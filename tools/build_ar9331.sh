#!/bin/bash
TARGET="mips-unknown-linux-uclibc"
MODE="release"
BIN=target/$TARGET/$MODE/gw
TOOLCHAIN=~/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2
CC=mips-openwrt-linux-uclibc-gcc
AR=mips-openwrt-linux-uclibc-ar
STRIP=mips-openwrt-linux-uclibc-strip

export PATH=$PATH:$TOOLCHAIN/bin
export CC_mips_unknown_linux_uclibc=$CC
export AR_mips_unknown_linux_uclibc=$AR
export STAGING_DIR=$TOOLCHAIN

# 切换为 mips-unknown-linux-uclibc 编译的工具链
rustup default my_toolchain

cd ..
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN
