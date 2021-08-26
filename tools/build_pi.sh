#!/bin/bash

TARGET="arm-unknown-linux-gnueabihf"
MODE="release"
BIN=target/$TARGET/$MODE/gw
TOOLCHAIN=~/wsl/tool/raspberrypi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64
CC=arm-linux-gnueabihf-gcc
STRIP=arm-linux-gnueabihf-strip

export PATH=$PATH:$TOOLCHAIN/bin
export CC_arm_unknown_linux_gnueabihf=$CC

cd ..
cargo build --$MODE --target=$TARGET -vv
$STRIP $BIN
echo ""
ls -lh $BIN
