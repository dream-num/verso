# Verso

[English](README.md) | [简体中文](README.zh-CN.md)

[![CI](https://github.com/dream-num/verso/actions/workflows/ci.yml/badge.svg)](https://github.com/dream-num/verso/actions/workflows/ci.yml)
[![npm version](https://img.shields.io/npm/v/@univerkit/verso.svg)](https://www.npmjs.com/package/@univerkit/verso)
[![license](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Verso 是一个面向 JavaScript workspace 的轻量发布工具，适合多个包共用同一个版本号的仓库。它会更新 package manifest，生成 Angular 风格的 conventional changelog，创建 release commit 和 tag，并通过 `git push --follow-tags` 推送。

## 安装

Verso 需要 Node.js 22.18.0 或更高版本。

```sh
pnpm add -D @univerkit/verso
```

在项目的 `package.json` 里增加 release script：

```json
{
  "scripts": {
    "release": "verso"
  }
}
```

## 配置

单包项目可以不创建 `verso.toml`。默认 `verso.toml` 不存在且根目录有 package manifest 时，Verso 会使用内置默认配置发布根 package。

只有需要自定义发布行为时才需要创建 `verso.toml`。单包项目的配置可以很轻量：

```toml
[version]
root_package = "package.json"
```

workspace 发布再配置 package glob：

```toml
[workspaces]
patterns = [
  "apps/*",
  "examples/**",
  "bundle/*",
  "packages/*",
  "!packages/**/fixtures",
  "packages-experimental/*",
  "presets/packages/*",
]
```

如果省略 `workspaces.patterns`，Verso 会先读取包管理器的 workspace 元数据，再回退到单包模式。它支持 `pnpm-workspace.yaml` 的 `packages`、根 package manifest 里的 `workspaces: ["packages/*"]`，以及 `workspaces: { "packages": ["packages/*"] }`。

package 发现支持 `package.json`、`package.json5`、`package.yaml` 和 `package.yml`。同一个目录存在多个 manifest 时，按这个顺序选择。

默认配置如下：

```toml
[version]
root_package = "package.json"
require_consistent_versions = true
cargo_manifest_paths = []

[workspaces]
include_root = true
ignore = []
use_gitignore = true

[changelog]
infile = "CHANGELOG.md"
preset = "angular"

[git]
require_clean_worktree = true
commit_message = "chore(release): release v${version}"
tag_name = "v${version}"
push = "follow-tags"

[github_release]
enabled = false
```

hooks 是可选配置，默认全部关闭：

```toml
[hooks]
before_version = "pnpm test"
after_version = "pnpm build"
before_commit = "pnpm lint"
after_push = "node scripts/notify-release.mts"
```

`changelog.preset` 当前只支持 `angular`。`git.push` 当前只支持 `follow-tags`。Verso 目前不会根据项目配置创建 GitHub Release，因此 `github_release.enabled = true` 会被拒绝。Verso 自身的二进制产物由本仓库的 GitHub Actions release workflow 附加到 GitHub Release。

### 配置项

未传 `--config` 且默认 `verso.toml` 不存在时，如果根目录存在 package manifest，Verso 会回退到内置默认配置。显式传入的 `--config <PATH>` 必须存在。

| 配置项 | 必填 | 默认值 | 说明 |
| --- | --- | --- | --- |
| `workspaces.patterns` | 否 | `[]` | 相对于配置文件目录的 package workspace glob。使用正斜杠，支持 `*`、`**`、`?`、字符类、brace，以及 `!` 排除模式。省略时会读取 `pnpm-workspace.yaml` 或根 package manifest 的 `workspaces`；如果都不存在，则使用单包模式。 |
| `workspaces.include_root` | 否 | `true` | 是否包含 `version.root_package` 指向的根 package。 |
| `workspaces.ignore` | 否 | `[]` | workspace 发现时额外忽略的模式。`fixtures` 这类普通路径片段会按目录名匹配。 |
| `workspaces.use_gitignore` | 否 | `true` | workspace 发现时是否读取根目录和子目录里的 `.gitignore`。 |
| `version.root_package` | 否 | `package.json` | 用于读取当前版本并参与更新的根 package manifest。路径必须在配置文件目录内。省略且 `package.json` 不存在时，Verso 会依次查找 `package.json5`、`package.yaml`、`package.yml`。 |
| `version.require_consistent_versions` | 否 | `true` | 发现 package 或配置的 Cargo manifest 版本不一致时是否失败。 |
| `version.cargo_manifest_paths` | 否 | `[]` | 需要同步更新 `[package].version` 的 Cargo manifest 路径。存在最近的 `Cargo.lock` 时会一起更新。 |
| `changelog.infile` | 否 | `CHANGELOG.md` | 发布时写入的 changelog 文件。路径必须在配置文件目录内。 |
| `changelog.preset` | 否 | `angular` | 目前只支持 `angular`。 |
| `git.require_clean_worktree` | 否 | `true` | 修改文件前要求工作区干净。 |
| `git.commit_message` | 否 | `chore(release): release v${version}` | release commit message。`${version}` 会替换为目标版本。 |
| `git.tag_name` | 否 | `v${version}` | release tag 模板。必须包含 `${version}`，并渲染为合法 Git tag。 |
| `git.push` | 否 | `follow-tags` | 目前只支持 `follow-tags`。 |
| `github_release.enabled` | 否 | `false` | 当前版本不支持设为 `true`。 |
| `hooks.before_version` | 否 | 无 | 更新 release 文件前执行的 shell 命令。 |
| `hooks.after_version` | 否 | 无 | 更新 release 文件后执行的 shell 命令。 |
| `hooks.before_commit` | 否 | 无 | 暂存并提交前执行的 shell 命令。 |
| `hooks.after_commit` | 否 | 无 | release commit 创建后执行的 shell 命令。 |
| `hooks.before_tag` | 否 | 无 | 创建 release tag 前执行的 shell 命令。 |
| `hooks.after_tag` | 否 | 无 | release tag 创建后执行的 shell 命令。 |
| `hooks.before_push` | 否 | 无 | 执行 `git push --follow-tags` 前执行的 shell 命令。 |
| `hooks.after_push` | 否 | 无 | push 成功后执行的 shell 命令。 |

## CLI

```sh
pnpm release
pnpm release -- --dry-run
pnpm release -- --version 0.26.0
pnpm release -- --version 0.26.0 --yes
pnpm release -- --dry-run --json
pnpm release -- --config path/to/verso.toml
pnpm release -- doctor
pnpm release -- init
pnpm release -- -V
pnpm release -- --help
```

| 参数 | 默认值 | 说明 |
| --- | --- | --- |
| `--dry-run` | `false` | 预览发布过程，不写文件，也不执行会修改状态的 git 命令。 |
| `--json` | `false` | 以 JSON 打印 dry-run 输出。必须和 `--dry-run` 一起使用。 |
| `--version <SEMVER>` | 无 | 使用指定目标版本，跳过版本选择。 |
| `--config <PATH>` | `verso.toml` | 读取其他配置文件。 |
| `--yes` | `false` | 跳过发布确认。它不会替你选择版本。 |
| `-V, --tool-version` | 无 | 打印 Verso CLI 版本。 |
| `--help` | 无 | 打印帮助信息。 |

子命令：

| 命令 | 说明 |
| --- | --- |
| `verso init` | 创建初始 `verso.toml`。会自动探测 `packages/*`，也可以用 `--single`、`--workspace`、`--force` 控制行为。 |
| `verso doctor` | 校验配置解析、package 发现、版本一致性、changelog 路径和 Cargo manifest 版本。可用 `--json` 输出结构化结果。 |

不传 `--version` 时，Verso 会打开交互式菜单选择 patch、minor、major、alpha、beta、rc 或自定义 semver。选择 prerelease channel 后，会继续选择 base version，也支持输入自定义 base version。`--version` 可以传精确版本，包括 `0.26.0-alpha.0`、`0.26.0-beta.1`、`0.26.0-rc.2` 这类 prerelease。

`--yes` 会跳过发布确认，包括目标版本不大于当前版本时的确认。它不会替你选择目标版本；没有 `--version` 时仍然会进入交互式版本选择。`-V` 和 `--tool-version` 会在读取发布配置前直接输出 CLI 版本，适合排查安装问题。

当 stdin 或 stdout 不是终端时，Verso 会保留纯文本 fallback，脚本测试和管道输入仍然可以用名称选择，比如先输入 `beta`，再输入 `minor`。

## 发布时会发生什么

Verso 会读取配置，发现匹配的 package manifest，在启用一致性检查时确认版本一致，然后解析目标版本。实际发布时，它会在更新 release 文件、提交、打 tag、推送前分别请求确认。这些确认默认是 yes：直接回车会继续，输入 `n` 会在下一步开始前停止；传入 `--yes` 时这些确认会被跳过。更新 release 文件会修改 package 文件、配置的 Cargo manifest 以及对应最近的 `Cargo.lock`，并把 `CHANGELOG.md` 追加到顶部。

Dry run 不会写文件，也不会执行会修改状态的 git 命令。它会打印当前版本、目标版本、警告、changelog 路径、计划执行的 git 命令、计划执行的 hooks，以及将被更新的版本文件树。Dry run 只列出 hooks，不会执行 hooks。`--dry-run --json` 会输出结构化 release plan，方便脚本和 CI 读取。

workspace 发现始终跳过 `.git` 和 `node_modules`。默认也会读取根目录和子目录里的 `.gitignore`，被忽略的目录不会继续扫描，即使里面有 package manifest 也不会被当作发布包。如果项目确实要发布被 `.gitignore` 忽略的目录，可以设置 `workspaces.use_gitignore = false`。Verso 只更新 package manifest 的版本，不会改写 workspace 内部依赖范围，也不会执行包管理器的 publish 命令。

如果本地发布命令执行失败，Verso 会尽力回滚自己修改过的文件、取消暂存 release 路径，并在安全时清理本地 release 状态。如果你在发布确认里输入 `n`，Verso 会停止流程，但不会回滚已经完成的步骤。如果最后 push 失败，本地 release commit 和 tag 会保留，你可以修复远端问题后执行 `git push --follow-tags`。远端 push 成功后的回滚需要手动处理。

## 分发方式

`@univerkit/verso` 是 TypeScript npm wrapper。真正的 Rust 二进制通过 optional platform package 安装。

| 平台 | CPU | 包名 | 二进制 |
| --- | --- | --- | --- |
| macOS | arm64 | `@univerkit/verso-darwin-arm64` | `bin/verso` |
| macOS | x64 | `@univerkit/verso-darwin-x64` | `bin/verso` |
| Linux | arm64 | `@univerkit/verso-linux-arm64` | `bin/verso` |
| Linux | x64 | `@univerkit/verso-linux-x64` | `bin/verso` |
| Windows | x64 | `@univerkit/verso-win32-x64` | `bin/verso.exe` |

Release workflow 会构建这些二进制，用 `--help` 做 smoke test，并检查二进制输出的 `--tool-version` 是否和 release tag 匹配。随后 workflow 会为原生二进制生成 GitHub Artifact Attestation，把二进制复制进对应的平台 npm 包，上传 `verso-binaries` workflow artifact，发布 GitHub Release assets，再发布平台包，最后发布 `@univerkit/verso` 主包。

GitHub Release 会附带 `verso-binaries.tar.gz` 和独立的 `SHA256SUMS.txt`。解压后可以这样校验：

```sh
shasum -a 256 -c SHA256SUMS.txt
```

联网时也可以校验二进制来源：

```sh
gh attestation verify ./verso-linux-x64/verso \
  --repo dream-num/verso \
  --signer-workflow dream-num/verso/.github/workflows/release.yml
```

## 发布流程

发布分为两个 GitHub Actions workflow。

`Prepare Release` 手动触发，需要输入目标版本。它会安装依赖，执行 `pnpm run check`，然后运行 `pnpm release -- --version <version> --yes`。这一步会修改版本、更新 `CHANGELOG.md`、创建 release commit、创建 tag，并推送回仓库。

`Release` 在 `v*` tag push 后自动触发，也可以手动输入 tag 触发。它会先检查 `NPM_TOKEN` 是否存在，然后构建平台二进制、生成校验文件和 GitHub Release assets，再按顺序发布平台 npm 包和主包。发布脚本会跳过 npm 上已经存在的同版本包，因此同一个 tag 的 workflow 重新运行时，可以从未发布的包继续。

稳定版本使用 `latest` dist-tag。`alpha`、`beta`、`rc` prerelease 会分别使用对应 dist-tag，避免覆盖稳定安装通道。稳定 GitHub Release 会标记为 latest；`alpha`、`beta`、`rc` GitHub Release 会标记为 prerelease，且不会提升为 latest。

发布前需要配置仓库 secret：

- `GH_TOKEN`：拥有 repository contents read/write 权限的 GitHub PAT，用于 `Prepare Release` 推送 release commit 和 tag。
- `NPM_TOKEN`：拥有 `@univerkit` scope 发布权限的 npm token。

当前发布流程仍使用 token 发布。npm trusted publishing 是后续迁移方向，不是当前默认路径。不要在 tokenless publish 真正验证通过前删除 `NPM_TOKEN`。

## 本地开发

```sh
pnpm install
pnpm run check
```

本地开发需要 Node.js 22.18.0 或更高版本。CI 当前使用 Node.js 24，`.nvmrc` 记录了推荐的本地 Node 主版本。

Rust 需要 1.85 或更高版本。本仓库通过 `rust-toolchain.toml` 使用 stable toolchain，并启用 rustfmt 和 clippy。

`pnpm run check` 会检查 release helper scripts 的 TypeScript 类型、Rust 格式、clippy、Rust 测试、npm wrapper 类型和 wrapper 测试。Rust 检查和 release build 都会以 `--locked` 使用 `Cargo.lock`。
