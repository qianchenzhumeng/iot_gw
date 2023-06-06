#!/bin/bash

TARGET="arm-unknown-linux-gnueabihf"
MODE="release"
BIN=target/$TARGET/$MODE/gw
# 注意，需要改为实际工具链所在的路径
TOOLCHAIN=~/wsl/tool/raspberrypi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64
CC=arm-linux-gnueabihf-gcc
# 注意，需要改为实际 libz.h 所在的路径
CFLAGS="-I/home/dell/wsl/source/libz-1.2.1100+2/libz"
STRIP=arm-linux-gnueabihf-strip

export PATH=$PATH:$TOOLCHAIN/bin
export CC_arm_unknown_linux_gnueabihf=$CC
export CFLAGS_arm_unknown_linux_gnueabihf=$CFLAGS

cd ..
if [ -f "$BIN" ];then
    rm $BIN
fi
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN
