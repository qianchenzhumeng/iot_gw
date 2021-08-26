#!/bin/bash
TARGET="mipsel-unknown-linux-musl"
MODE="release"
BIN=target/$TARGET/$MODE/gw
TOOLCHAIN=~/wsl/OpenWRT/openwrt-sdk-ramips-mt76x8_gcc-7.5.0_musl.Linux-x86_64/staging_dir/toolchain-mipsel_24kc_gcc-7.5.0_musl
CC=mipsel-openwrt-linux-gcc
STRIP=mipsel-openwrt-linux-strip

export PATH=$PATH:$TOOLCHAIN/bin
export CC_mipsel_unknown_linux_musl=$CC
export STAGING_DIR=$TOOLCHAIN

cd ..
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN
