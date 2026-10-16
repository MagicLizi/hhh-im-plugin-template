#!/bin/sh
# 打 iPhone / iPad 的包：编成 WASM 组件（wasm32-wasip2）、把它与清单放进 dist/stage-ios、用 hhh-im-pack 打成 .hhhim 并写进版本索引
# （与桌面包同一份，同一版本会合并）。HHH 的 iOS 版在 App 里跑这个组件，插件代码不用改。
# 用法：scripts/pack-ios.sh [--publish]
#   --publish：打好后发到清单 source 写的仓库；令牌放环境变量 HHH_IM_PUBLISH_TOKEN（或 GITHUB_TOKEN / GITLAB_TOKEN），不写进任何文件。
# 第一次先装目标：rustup target add wasm32-wasip2。用到 tokio 的网络、文件的插件要带 RUSTFLAGS="--cfg tokio_unstable"。
# hhh-im-pack 没装时：cargo install --git https://github.com/MagicLizi/hhh-im-sdk --tag v0.6.0 hhh-im-protocol --features transport,package
set -eu
cd "$(dirname "$0")/.."
cargo build --release --target wasm32-wasip2
rm -rf dist/stage-ios
mkdir -p dist/stage-ios
cp hhh-im-provider.json logo.svg dist/stage-ios/
cp target/wasm32-wasip2/release/demo-im.wasm dist/stage-ios/
hhh-im-pack dist/stage-ios --target wasm32-wasip2 --out dist --notes notes.md "$@"
