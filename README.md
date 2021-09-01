### 1. 功能描述

**已支持以下功能**：

- 数据管理模块(使用 sqlite3)
- 支持与服务器通过MQTT协议通信，保持长连接
- 支持解析JSON格式的终端数据
- 支持从文件读取终端数据
- 离线时缓存来自终端的数据
- 在线时发布离线时缓存的数据
- 使用同一个TCP连接发送数据
- 从文件读取配置信息，例如数据库路径、服务器地址、id等信息
- 支持数据模板，根据数据模板重新格式化来自终端的数据、添加自定义属性、预定义属性(例如添加时间戳)等，从而生成新的JSON数据
- 支持从串口读取终端的数据
- 支持缓存来自终端的任意类型数据(最长 256 字节)
- 日志
- 支持MQTTS(TLS v1.2，v1.3)

**计划开发的功能**：

- 提供配置信息的WEB界面

### 2. 使用示例

在本地启动 MQTT broker，监听 1883 端口，例如使用 mosquitto：

```bash
mosquitto -v -c mosquitto.conf
```

或者修改配置文件 `gw.toml`，指定可用的 MQTT broker：

```toml
[server]
address = "127.0.0.1:1883"
```

#### (1) 从文件读取数据（默认）

修改配置文件（默认是 gw.toml），指定文件，并且将数据接口类型设置为 `text_file`（默认为此配置）：

```toml
[data_if]
#if_name = "/dev/ttyS14"
#if_type = "serial_port"
if_name = "./data_if.txt"
if_type = "text_file"
```

运行网关程序：

```bash
cargo run -- -c gw.toml
```

在文件中写入数据（需要有换行 `'\n'`）：

```bash
echo "{\"id\":1,\"name\":\"SN-001\",\"temperature\": 27.45,\"humidity\": 25.36,\"voltage\": 3.88,\"status\": 0}" > data_if.txt
```

**说明**：网关程序每隔 1 秒读清一次文件，读清后需要手动写入数据。

#### (2) 从串口读取数据

修改配置文件（默认是 gw.toml），指定串口，并且将数据接口类型设置为 `serial_port`：

```toml
[data_if]
if_name = "/dev/ttyS14"
if_type = "serial_port"
#if_name = "./data_if.txt"
#if_type = "text_file"
```

网关程序：

```bash
cargo run -- -c gw.toml
```

使用外部设备按 [MIN](https://github.com/min-protocol/min) 协议向串口发送数据（ arduino/min 目录内有 Arduino UNO 示例）：

```
{"id":1,"name":"SN-001","temperature": 27.45,"humidity": 25.36,"voltage": 3.88,"status": 0}
```

#### (3) 使用 TLS

在本地启动 MQTT broker，例如使用 mosquitto：

```bash
mosquitto -c mosquitto.conf
```

配置文件内容（按实际情况修改 ca 文件路径）：

```shell
# mosquitto.conf
log_type error
log_type warning
log_type notice
log_type information
log_type debug

allow_anonymous true

# non-SSL listeners
listener 1883

# server authentication - no client authentication
listener 18885

# 指定 ca 文件路径
cafile ca/ca.crt
certfile ca/server.crt
keyfile ca/server.key
require_certificate false
tls_version tlsv1.2
```

修改 Cargo.toml 文件，开启 `ssl` 特性：

```toml
[features]
default = ["build_bindgen", "bundled", "ssl"]
#default = ["build_bindgen", "bundled"]
```

修改配置文件（默认是 gw.toml），使用 ssl 协议，并指定 ca 文件：

```toml
[server]
#address = "127.0.0.1:1883"
address = "ssl://127.0.0.1:18885"

[tls]
cafile = "ca/ca.crt"
# pem 文件生成方式：cat client.crt client.key ca.crt > client.pem
key_store = "ca/client.pem"
```

编译运行网关程序：

```bash
cargo run -- -c gw.toml
```

### 3. 数据模板引擎功能说明

 模板支持的功能：

1. 可以使用原消息中字符串类型的属性值作为属性名
2. 可以使用原消息中的属性值
3. 可以新增自定义的属性名/属性值
4. 可以使用有特定内容的模板，比如时间戳

对原始数据的要求：

1. 格式为 JSON

1. 不能有同名的属性
2. 打算用做模板属性名的字符串属性值需要符合 JSON 属性名命名规范

 模板示例（以 `gw.toml` 中的为例）：

```bash
# 原始数据示例
"{\"id\":1,\"name\":\"SN-001\",\"temperature\": 27.45,\"humidity\": 25.36,\"voltage\": 3.88,\"status\": 0}"
# 模板
"{<{name}>: [{\"ts\": <#TS#>,\"values\": {\"temperature\": <{temperature}>, \"humidity\": <{humidity}>,\"voltage\": <{voltage}>,\"status\": <{status}>}}]}"
```

以上设置的转换效果为：

```bash
# 原始数据
{"id":1,"name":"SN-001","temperature": 27.45,"humidity": 25.36,"voltage": 3.88,"status": 0}
# 输出数据
{"SN-001": [{"ts": 1596965153255,"values": {"temperature": 27.45, "humidity": 25.36,"voltage": 3.88,"status": 0}}]}
```

模板注解:

1. `<{name}>` ：取原消息属性 `name` 对应的属性值。例如，需要使用消息 `"temperature": 27.45` 中 `temperature` 的属性值 `27.45` 作为输出数据中的属性值，需要在模板中填写 `<{temperature}>`
2. `<#NAME#>` ：使用模板引擎可以提供的值。例如<#TS#>表示自 EPOCH 以来的秒数；
3. 符合 JSON 属性名命名规范的字符串类型的属性值可以作为模板中的属性名。需要将模板填成 "<{属性名}>" 的形式. 例如, 需要使用消息 `{"name": "SN-001"}`中 `name` 的属性值 `SN-001` 作为输出数据中的属性名, 需要在模板中填写 `<{name}>`。

### 4. 已支持的平台

- x86_64-unknown-linux-gnu
  
  - 需要将子目录 `termios-rs`、`serial-rs`、`ioctl-rs` 切换到 `master` 分支
  
- mips-unknown-linux-uclibc
  - 需要为该目标平台编译 rust：[Cross Compile Rust For OpenWRT](https://qianchenzhumeng.github.io/posts/cross-compile-rust-for-openwrt/)
  - 需要编译 openssl
  - 需要将子目录 `termios-rs`、`serial-rs`、`ioctl-rs` 切换到 `openwrt_cc` 分支
  - 编译命令: 
  
  ```bash
  #编译 libsqlite3-sys 需要指定交叉编译工具链
  export STAGING_DIR=/mnt/f/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2
  export CC_mips_unknown_linux_uclibc=mips-openwrt-linux-uclibc-gcc
  cargo build --target=mips-unknown-linux-uclibc --release
  ```

- arm-unknown-linux-gnueabihf（树莓派）
  - 需要将子目录 `termios-rs`、`serial-rs`、`ioctl-rs` 切换到 `pi` 分支
- mipsel-unknown-linux-musl
  - 尚未对串口进行适配

默认启用了 rusqlite 的 bundled 特性，libsqlite3-sys 会使用 cc crate 编译 sqlite3，交叉编译时要在环境变量中指定 cc crate使用的编译器(cc crate 的文档中有说明)，否则会调用系统默认的 cc，导致编译过程中出现文件格式无法识别的情况。

为其他平台进行交叉编译时，需要为其单独处理 `termios-rs`、`serial-rs`、`ioctl-rs`、`paho-mqtt-sys`，这些库对应的 github 仓库中有相应的说明。

目录 tools 内有部分平台的编译脚本。

### 5. 交叉编译问题解答

#### (1) ssl 相关

默认启用了 `paho-mqtt-sys/vendored-ssl` 特性，编译过程中会自动编译自带的 `paho-mqtt-sys` 自带的 openssl 版本，编译成功后，会将其静态链接至最终的可执行文件。这部分是关闭该特性时手动编译 openssl 可能会遇到的问题。`paho-mqtt-sys` 自带的 openssl 版本内没有 mips-unknown-linux-uclibc 的编译配置，需要修改库文件，增加编译配置，或者关闭 `paho-mqtt-sys/vendored-ssl` 特性，手动编译、动态链接。如果是动态链接，需要确保最终的执行环境上有要链接的 openssl 库。

> /mnt/f/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2/bin/../lib/gcc/mips-openwrt-linux-uclibc/4.8.3/../../../../mips-openwrt-linux-uclibc/bin/ld: cannot find -lssl
> /mnt/f/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2/bin/../lib/gcc/mips-openwrt-linux-uclibc/4.8.3/../../../../mips-openwrt-linux-uclibc/bin/ld: cannot find -lcrypto

交叉编译 openssl：

```bash
# 设置 STAGING_DIR 环境变量（交叉编译工具链路径）
export STAGING_DIR=/mnt/f/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2
# 下载、解压源码
wget https://www.openssl.org/source/openssl-1.0.2l.tar.gz
tar zxf openssl-1.0.2l.tar.gz 
cd openssl-1.0.2l
```

```bash
# --prefix 为安装目录
./Configure linux-mips32 no-asm shared --cross-compile-prefix=mips-openwrt-linux-uclibc- --prefix=~/wsl/source/openssl
make
make install
```

将头文件以及共享库复制到交叉编译工具链的相关目录下：

```bash
cp ~/wsl/source/openssl/include/openssl $STAGING_DIR/include -R
cp ~/wsl/source/openssl/lib/*.so* $STAGING_DIR/lib
```

如果是 OpenWrt，可以通过 make menuconfig 启用 libopenssl（Libraries -> SSL -> libopenssl），然后将头文件以及共享库复制到交叉编译工具链的相关目录下：

```bash
cd ~/openwrt-19.07.2/build_dir/target-mipsel_24kc_musl/openssl-1.1.1d
cp *.so $STAGING_DIR/lib
cp include/openssl/ $STAGING_DIR/include -R
```

> Could not find directory of OpenSSL installation, and this `-sys` crate cannot
>   proceed without this knowledge. If OpenSSL is installed and this crate had
>   trouble finding it,  you can set the `OPENSSL_DIR` environment variable for the
>   compilation process.
>
>   Make sure you also have the development packages of openssl installed.
>   For example, `libssl-dev` on Ubuntu or `openssl-devel` on Fedora.

该信息前面会有一段环境变量相关的内容，根据提示设置相关的变量，例如：

```bash
# 设置环境变量
export MIPSEL_UNKNOWN_LINUX_MUSL_OPENSSL_DIR=$STAGING_DIR
export MIPSEL_UNKNOWN_LINUX_MUSL_OPENSSL_LIB_DIR=$STAGING_DIR/lib
export MIPSEL_UNKNOWN_LINUX_MUSL_OPENSSL_INCLUDE_DIR=$STAGING_DIR/include
# 编译
cargo build --target=mipsel-unknown-linux-musl --release -vv
```

#### (2) 找不到 libanl

> /mnt/f/wsl/OpenWRT/OpenWrt-SDK-ar71xx-for-linux-x86_64-gcc-4.8-linaro_uClibc-0.9.33.2/staging_dir/toolchain-mips_34kc_gcc-4.8-linaro_uClibc-0.9.33.2/bin/../lib/gcc/mips-openwrt-linux-uclibc/4.8.3/../../../../mips-openwrt-linux-uclibc/bin/ld: cannot find -lanl

按照 [src/CMakeLists.txt: fix build on uclibc or musl](https://github.com/eclipse/paho.mqtt.c/commit/517e8659ab566b15cc409490a432e8935b164de8) 修改 `.cargo/registry/src/crates.rustcc.com-a21e0f92747beca3/paho-mqtt-sys-0.3.0/paho.mqtt.c/src/CMakeLists.txt`

修改后仍然可能因找到了主机的 libanl 报错，如果还报错，按如下方式修改：

```cmake
        #SET(LIBS_SYSTEM c dl pthread anl rt)
		SET(LIBS_SYSTEM c dl pthread rt)
```

#### (3) 找不到 libclang

> thread 'main' panicked at 'Unable to find libclang: "couldn\'t find any valid shared libraries matching: [\'libclang.so\', \'libclang-*.so\', \'libclang.so.*\']

```bash
sudo apt-get install clang libclang-dev
```

#### (4) 找不到 bindings
> thread 'main' panicked at 'No generated bindings exist for the version/target: bindings/bindings_paho_mqtt_c_1.3.2-mips-unknown-linux-uclibc.rs', paho-mqtt-sys/build.rs:102:13

```bash
cargo install bindgen
sudo apt install libc6-dev-i386
cd ~/.cargo/registry/src/crates.rustcc.com-a21e0f92747beca3/paho-mqtt-sys-0.3.0
TARGET=mips-unknown-linux-uclibc bindgen wrapper.h -o bindings/bindings_paho_mqtt_c_1.3.2-mips-unknown-linux-uclibc.rs -- -Ipaho.mqtt.c/src --verbose
```

#### (5) 找不到 C 库头文件

>   debug:clang version: clang version 10.0.0-4ubuntu1
>   debug:bindgen include path: -I/mnt/f/wsl/project/iot_gw/target/mipsel-unknown-linux-musl/release/build/paho-mqtt-sys-0e7cd946c58a7093/out/include
>
>   --- stderr
>   fatal: not a git repository (or any of the parent directories): .git
>   /usr/include/stdio.h:33:10: fatal error: 'stddef.h' file not found
>   /usr/include/stdio.h:33:10: fatal error: 'stddef.h' file not found, err: true
>   thread 'main' panicked at 'Unable to generate bindings: ()', /home/dell/.cargo/registry/src/crates.rustcc.com-a21e0f92747beca3/paho-mqtt-sys-0.3.0/build.rs:139:14
>   note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace

安装 clang

```bash
sudo apt-get install clang
```

指定交叉编译工具链，例如：

```bash
export CC_mipsel_unknown_linux_musl=mipsel-openwrt-linux-gcc
export CXX_mipsel_unknown_linux_musl=mipsel-openwrt-linux-g++
```

#### (6) 找不到 MQTTAsync.h

> [paho-mqtt-sys 0.3.0] debug:clang version: clang version 10.0.0-4ubuntu1
> [paho-mqtt-sys 0.3.0] debug:bindgen include path: -I/mnt/f/wsl/project/iot_gw/target/mipsel-unknown-linux-musl/release/b
> uild/paho-mqtt-sys-0e7cd946c58a7093/out/include
> [paho-mqtt-sys 0.3.0] wrapper.h:1:10: fatal error: 'MQTTAsync.h' file not found
> [paho-mqtt-sys 0.3.0] wrapper.h:1:10: fatal error: 'MQTTAsync.h' file not found, err: true
> [paho-mqtt-sys 0.3.0] thread 'main' panicked at 'Unable to generate bindings: ()', /home/dell/.cargo/registry/src/crates
> .rustcc.com-a21e0f92747beca3/paho-mqtt-sys-0.3.0/build.rs:139:14
> [paho-mqtt-sys 0.3.0] note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
> error: failed to run custom build command for `paho-mqtt-sys v0.3.0`

为 mipsel-unknown-linux-musl 编译时遇到了该问题，解决办法是修改 paho-mqtt-sys 的版本：

```toml
[dependencies.paho-mqtt-sys]
default-features = false
#version = "0.3"
version = "0.5"
```

### 6. 网关运行问题解答

#### (1) 数据接口类型未知

> thread 'main' panicked at 'Init data interface failed: DataIfUnknownType'

Cargo.toml 中有关数据接口的特性和网关配置文件内的不一致。

#### (2) 连接错误

> Error connecting to the broker: NULL Parameter: NULL Parameter

使用 ssl 连接 broker，但是没有在 `Cargo.toml` 中启用 `ssl` 特性。

