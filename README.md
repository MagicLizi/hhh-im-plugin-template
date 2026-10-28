# HHH IM 插件模板

给 HHH 写一个 IM 插件的起点：清单、标志、能跑的插件骨架、打包脚本、给 agent 的规范，都在这里。

HHH 的 IM 层只做统一的界面和一个统一的插座（HHH-IM-Protocol）。插件是一个子进程：和自己的平台说话、把平台的格式翻成统一格式、
声明自己能做什么和要用户填什么；聊天记录、凭据、配置都交给 HHH。

## 五分钟起一个插件

1. 用这个模板建仓库（GitHub 上点 “Use this template”），克隆下来。
2. 装工具（自检与打包）：

   ```sh
   cargo install --git https://github.com/MagicLizi/hhh-im-sdk --tag v0.16.0 hhh-im-protocol --features transport,package
   ```

3. 改清单 `hhh-im-provider.json`：`id`（发布后不能改）、`name`、`publisher`、`source`（换成你的仓库）。
4. 打包并自检：

   ```sh
   powershell -File scripts/pack.ps1     # Windows
   scripts/pack.sh                       # macOS / Linux
   hhh-im-check dist/stage/hhh-im-provider.json
   ```

5. 在 HHH 里试：设置 → IM → 添加 IM → 文件夹，选 `dist/stage`。会多一个“示例 IM”，里面一个示例群，发什么它回“收到「…」”。

## iPhone / iPad 的包

iOS 不能运行下载来的原生程序，HHH 的 iOS 版在 App 里跑插件的 WASM 组件（`wasm32-wasip2`）。同一份代码多编一个包，插件代码不用改：

```sh
rustup target add wasm32-wasip2      # 第一次
scripts/pack-ios.sh                  # macOS / Linux
powershell -File scripts/pack-ios.ps1   # Windows
```

出来的是 `dist/` 里的 `demo-im-<版本>-wasm32-wasip2.hhhim`，和桌面包写进同一份版本索引。用到 tokio 的网络、文件的插件编 WASM 包时要带
`RUSTFLAGS="--cfg tokio_unstable"`；依赖里有 C 代码（如 `ring`）的要装 [wasi-sdk](https://github.com/WebAssembly/wasi-sdk) 并设 `CC_wasm32_wasip2`。

然后照 `src/main.rs` 的注释和 `AGENTS.md` 换成你的平台。

## 在 HHH 里怎么试

| 方式 | 什么时候用 |
|---|---|
| 添加 IM → 文件夹 | 开发时：选 `dist/stage` 或你的插件文件夹 |
| 添加 IM → 安装包 | 选 `dist/` 里的 `.hhhim`，和用户装的一样 |
| 添加 IM → 仓库 | 发布后：填你的仓库地址，HHH 读最新版本、下载这台电脑的包 |
| 添加 IM → 分享码 | 在支持分享码的 HHH 中，粘贴拥有者提供的分享码，安装和更新私有插件 |
| 卡片「管理 → 从文件更新」 | 已经装过，换成新打的包或文件夹 |

## 自动检查与发版本

仓库带一套统一工作流：`.github/workflows/hhh-plugin.yml` 与 `.github/hhh-ci/`（不要在你的仓库里改，要更新就从本模板复制新版）。要出哪些系统写在 `.github/hhh-plugin.json` 的 `targets`，清单 `os` 要写了对应系统。

- **自动检查**：提 PR、推 main 时，把每个系统都编一遍，跑测试与 `hhh-im-check`，并试着合一次包。
- **发版本**：
  1. 改清单与 `Cargo.toml` 的 `version`，在 `notes.md` 最上面写一节 `### <版本>`（给用户看的更新说明），合进 main。
  2. GitHub → Actions → **hhh-plugin** → Run workflow，「发布请求编号」随便填（比如版本号）→ Run。它在 main 上编全部系统、合成一份版本索引，建一个**草稿** Release（HHH 不读草稿，用户看不到）。版本没往上加、这一版已经发过、没写这一版的说明时，编译前就停下。
  3. 到仓库的 Releases 页看草稿：每个系统的 `.hhhim` 与 `hhhim-release.json` 都在，点 **Publish release**。用户打开 HHH 的设置 → IM 时会看到“有新版本”，点一下就更新。

不用 CI 也可以在自己电脑上发：令牌放环境变量 `HHH_IM_PUBLISH_TOKEN`，跑 `scripts/pack.ps1 -Publish`（或 `scripts/pack.sh --publish`）；iPhone / iPad 的包再跑一次 `scripts/pack-ios.sh --publish`（或 `pack-ios.ps1 -Publish`），同一版本的索引会合并。

详细规矩见 [hhh-im-sdk 的发版本规范](https://github.com/MagicLizi/hhh-im-sdk/blob/main/docs/release.md)。

### 分享私有插件

拥有者在支持分享码的 HHH 中点「管理 → 分享…」生成分享码，朋友粘贴安装后照常更新。删除分享码后，朋友下次检查显示「分享已被取消」，已装的插件仍可使用。

- GitHub：单独建一个只放安装包与版本索引的发布仓库，清单 `source` 指向它；分享用的细粒度令牌只选这一个仓库、Contents 只读。HHH 会引导生成，不能把发布令牌发给朋友。
- GitLab：新版打包工具发布时同时更新包仓库的最新版本索引，HHH 创建只读包仓库的部署令牌。旧工具发布的版本可用 `hhh-im-pack dist/stage --publish --latest-only` 只补索引，`dist/stage` 的清单版本必须与已发布版本相同。

本页的 `--latest-only` 需要包含该选项的新版打包工具（hhh-im-sdk v0.14.0 起都有）。

## 给 agent 的规范

用任何 agent 打开这个仓库都按同一套规矩写插件：

- `AGENTS.md`：规则正本（先读协议、清单怎么填、插件只做什么、改完必须自检并在 HHH 里试、怎么发版本）。
- `CLAUDE.md`：只有一行，指到 `AGENTS.md`。
- skill「开发插件」与「发版本」：`.agents/skills/` 与 `.claude/skills/`（内容相同）。

## 本仓库的版本

这个模板本身也按规范发布示例插件“示例 IM”的版本，可以在 HHH 里用「添加 IM → 仓库」填 `https://github.com/MagicLizi/hhh-im-plugin-template` 试装与更新。

## 许可证

[MIT](LICENSE)。用这个模板起的插件，许可证由你自己定（改掉 `LICENSE` 与 `Cargo.toml` 的 `license`）。

## 图片的预览与原件

示例消息演示预览与原件分开、共用同一份图片、只有预览三种情况；资源分段用运行时 chunk_of_bytes。

新宿主经 `resources/describe` 选择实际版本，读取时带 `variant`；省略 `variant` 的旧调用仍按原来的方式读取；二进制安装仍需通过宿主的最低 SDK 检查。分段缓存包含实际版本，不把预览当成原件。本版固定使用 IM SDK v0.16.0，宿主需支持该版 SDK。

## 合并转发记录

示例群包含三种转发记录：60 条的完整记录（文字、图片、可下载的 `记录说明.md` 和嵌套记录）、部分可读记录，以及平台未提供正文的记录。点击预览后由宿主的同一个居中阅读器展示。`src/forwarded.rs` 演示外层消息内唯一资源键及按消息和种类验证下载归属；内部编号不作为当前会话消息上报。新样例需要支持 `forwarded` 内容块的宿主，普通示例与新样例统一固定使用公开 IM SDK `v0.16.0`。
