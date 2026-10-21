---
name: release
description: 给这个 HHH IM 插件发一个新版本：改版本号、打包、核对、发布到 GitHub / GitLab。用户说“发版本”“发布 x.y.z”“出个包”时用。
---

# 发版本

规矩以 hhh-im-sdk 的 `docs/release.md` 为准；这里是步骤。

1. 定版本号：比仓库里最新的正式版本大（`主.次.修订`）。改清单 `hhh-im-provider.json` 的 `version`，`Cargo.toml` 的 `version` 一起改。
2. 写更新说明 `notes.md`（Markdown，给用户看，2000 字以内：改进、修复，一条一行）。
3. 本地核对：`scripts/pack`（Windows：`powershell -File scripts/pack.ps1`），`hhh-im-check dist/stage/hhh-im-provider.json` 全部通过；
   在 HHH 里「管理 → 从文件更新」装 `dist/` 里的 `.hhhim` 试一遍。
4. 提交，打标签 `v<版本>`（与清单完全一致），推提交与标签。
5. 发布：令牌放环境变量 `HHH_IM_PUBLISH_TOKEN`（GitHub 写 Contents 权限；GitLab 带 `api` 的个人访问令牌），不写进任何文件、不贴进对话：
   - Windows：`powershell -File scripts/pack.ps1 -Publish`
   - macOS / Linux：`scripts/pack.sh --publish`
   要出几个系统的包就在几台电脑上各跑一次；iPhone / iPad 的包在任一台上跑 `scripts/pack-ios.sh --publish`（Windows：
   `powershell -File scripts/pack-ios.ps1 -Publish`）。同一版本的索引会合并。
6. 核对：Release 上有每个目标的 `.hhhim` 与 `hhhim-release.json`；在 HHH 里打开设置 → IM，卡片写“有新版本 x.y.z”，点更新能装上。

标签与清单版本对不上、版本没往上加时打包工具会拒绝，照它说的改，不要绕过。

## 私有插件与分享码

- GitHub 的清单 `source` 指向单独的发布仓库，只放安装包与版本索引。发布令牌只留在发布者电脑；分享码由支持这项功能的 HHH 引导生成，细粒度令牌只选发布仓库、Contents 只读。
- GitLab 使用包含最新索引功能的打包工具发布；发布成功后包仓库 `packages/generic/<插件编号>/latest/hhhim-release.json` 与版本目录的索引应完全一致。给旧版本补索引用 `hhh-im-pack dist/stage --publish --latest-only`，暂存清单版本必须已发布；不重传安装包、不改版本。
- 先核对正在运行的打包工具确实支持 `--latest-only`，不把模板锁定的旧 SDK 版本当成已具备新功能。分享者在 HHH「管理 → 分享…」建码，朋友粘贴安装和更新；删除码后朋友下次检查显示已取消，已装插件照常能用。
