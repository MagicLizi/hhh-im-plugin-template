---
name: release
description: 给这个 HHH IM 插件发一个新版本：改版本号与更新说明、合进 main，再用 GitHub Actions 编全部系统、建草稿、核对后发布到 GitHub（或在自己电脑上用打包工具发布）。用户说“发版本”“发布 x.y.z”“出个包”时用。
---

# 发版本

规矩以 hhh-im-sdk 的 `docs/release.md` 为准；这里是步骤。要出哪些系统写在 `.github/hhh-plugin.json` 的 `targets`，清单 `os` 要包含对应系统。

1. 定版本号：比仓库里最新的正式版本大（`主.次.修订`）。改清单 `hhh-im-provider.json` 的 `version`，`Cargo.toml` 的 `version` 一起改。
2. 写更新说明：在 `notes.md` 最上面加一节 `### <版本>`（Markdown，给用户看：改进、修复，一条一行）。整份超过 2000 字时，发布只取最新的几节。
3. 本地核对：按 `AGENTS.md`「改完必须」跑一遍；在 HHH 里「管理 → 从文件更新」装本机打的包试一遍。
4. 开 PR 合进 main：PR 上的 `hhh-plugin` 检查要全部通过。不用自己打标签，发布时标签自动打在这次的提交上。
5. 建草稿：GitHub → Actions → `hhh-plugin` → Run workflow，「发布请求编号」随便填（如版本号）。它编全部系统、合成版本索引、建草稿 Release（HHH 不读草稿）。版本没往上加、已经发过、没写这一版说明、系统比上一个正式版本少时会停下，照提示改，不要绕过；确实不再出某个系统写进 `.github/hhh-plugin.json` 的 `dropTargets`。
6. 发布：到 Releases 页核对草稿里每个系统的 `.hhhim` 与 `hhhim-release.json`，点 Publish release。
7. 核对：在 HHH 里打开设置 → IM，卡片写“有新版本 x.y.z”，点更新能装上。

agent 只做 1–4；第 5、6 步由维护者决定什么时候做，用户明确要求发布时才代为触发。

不用 CI 时：令牌放环境变量 `HHH_IM_PUBLISH_TOKEN`（GitHub 写 Contents 权限；GitLab 带 `api` 的个人访问令牌），不写进任何文件、不贴进对话；Windows 跑 `powershell -File scripts/pack.ps1 -Publish`，macOS / Linux 跑 `scripts/pack.sh --publish`，要出几个系统就在几台电脑上各跑一次；iPhone / iPad 的包在任一台上跑 `scripts/pack-ios.sh --publish`。同一版本的索引会合并。

## 私有插件与分享码

- GitHub 的清单 `source` 指向单独的发布仓库，只放安装包与版本索引。发布令牌只留在发布者电脑；分享码由支持这项功能的 HHH 引导生成，细粒度令牌只选发布仓库、Contents 只读。
- GitLab 使用包含最新索引功能的打包工具发布；发布成功后包仓库 `packages/generic/<插件编号>/latest/hhhim-release.json` 与版本目录的索引应完全一致。给旧版本补索引用 `hhh-im-pack dist/stage --publish --latest-only`，暂存清单版本必须已发布；不重传安装包、不改版本。GitLab 上用不了这里的 GitHub Actions，按上面「不用 CI 时」发布。
