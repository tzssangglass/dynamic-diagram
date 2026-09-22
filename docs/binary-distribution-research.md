# Rust CLI 二进制 + npm/pi 扩展的发布与分发:业内通行做法调研

调研日期:2026-09-21。所有结论均基于一手来源(官方文档、GitHub Releases API、npm registry JSON、官方 README),关键引用保留英文原文并附 URL。

---

## 1. Rust CLI 项目的通行发布做法

### 1.1 旗舰 CLI 的 GitHub Releases 产物(实测定况)

通过 GitHub Releases API 抓取了 ripgrep、fd、bat、uv、ruff 最新 release 的完整资产清单:

**ripgrep 15.2.0**([api.github.com/repos/BurntSushi/ripgrep/releases/latest](https://api.github.com/repos/BurntSushi/ripgrep/releases/latest)):
- 命名:`ripgrep-15.2.0-<target-triple>.tar.gz`(Unix)/ `.zip`(Windows),tag 不带 `v` 前缀
- 覆盖:x86_64/aarch64/armv7/i686/s390x × linux-gnu/linux-musl/darwin/windows-msvc/gnu
- 每个资产配一个同名 `.sha256` 文件;另有 `ripgrep_15.2.0-1_amd64.deb`(Debian 包)
- 无 GPG/cosign 签名

**fd v10.5.0**([api.github.com/repos/sharkdp/fd/releases/latest](https://api.github.com/repos/sharkdp/fd/releases/latest)):
- 命名:`fd-v10.5.0-<target-triple>.tar.gz`(tag 带 `v` 前缀,文件名也带)
- 同样 per-target 压缩包 + 多架构 `.deb`(amd64/arm64/armhf/i686/musl 变体)
- 无签名

**bat v0.26.1**([api.github.com/repos/sharkdp/bat/releases/latest](https://api.github.com/repos/sharkdp/bat/releases/latest)):
- 同 fd 的命名与结构;per-target tar.gz/zip + 全套 `.deb`;无签名

**uv 0.12.17** 与 **ruff 0.16.8**([uv releases/latest](https://api.github.com/repos/astral-sh/uv/releases/latest)、[ruff releases/latest](https://api.github.com/repos/astral-sh/ruff/releases/latest)):
- 命名:`uv-<target-triple>.tar.gz`,tag 不带 `v` 前缀
- 每个资产配 `.sha256`;另有一个汇总的 `sha256.sum`
- 有 `dist-manifest.json`(cargo-dist 的标志性产物)和 `source.tar.gz`
- 有 `uv-installer.sh` 和 `uv-installer.ps1`(curl | sh / irm | iex 一键安装脚本)
- 无 GPG/cosign 签名

**结论**:通行做法 = per-target-triple 压缩包(tar.gz 用于 Unix、zip 用于 Windows)+ SHA-256 校验和(每资产一个或汇总一个)。`.deb` 包常见但非必须。**签名(GPG/cosign)在抽样五个项目中均未使用**,不是事实标准要求。GitHub 自 2025-06 起为 release 资产自动提供 SHA-256 digest([GitHub changelog](https://github.blog/changelog/2025-06-03-releases-now-expose-digests-for-release-assets/))。

### 1.2 cargo-dist:当前事实标准

[cargo-dist 官方 book](https://axodotdev.github.io/cargo-dist/book/introduction.html)(uv、ruff、axolotlsay 等均使用,从资产中的 `dist-manifest.json` 可直接确认 uv/ruff 在用)。

支持的 installers([官方文档](https://axodotdev.github.io/cargo-dist/book/installers/index.html),原文引用):

> "Currently supported installers include:
> - **shell**: a shell script that fetches and installs executables (for `curl | sh`)
> - **powershell**: a powershell script that fetches and installs executables (for `irm | iex`)
> - **npm**: an npm project that fetches and runs executables (for `npx`)
> - **homebrew**: a Homebrew formula that fetches and installs executables
> - **msi**: a Windows msi that bundles and installs executables"

安装器分两类(原文):"**Fetching**(shell, powershell, npm, homebrew): thin wrappers which detect the user's current platform and download and unpack the appropriate archive from a server";"**Bundling**(msi): contain the actual binaries they will install on the user's system"。

npm installer 的工作方式([官方文档](https://axodotdev.github.io/cargo-dist/book/installers/npm.html),原文引用):

> "The npm package will fetch your prebuilt archives and install your binaries to node_modules, exposing them as commands ('bins') of the package."
> "The npm package has a few module dependencies which are used to fetch your binary after the package itself is installed."

即:cargo-dist 生成的 npm 包是 **install 时从 GitHub Releases 下载二进制**的 fetching 包(类似下文 puppeteer 模式),而不是 esbuild 式的平台子包。

产物:per-target tar.gz/zip 归档、每个资产的 `.sha256`、`sha256.sum` 汇总、统一 `dist-manifest.json`(供下游工具/oranda 消费)、一键安装脚本、自动发布到 Homebrew tap / npm。

### 1.3 三个 CI 工具的角色定位

- **release-plz**([README](https://github.com/release-plz/release-plz),原文):"Release-plz helps you release your Rust packages by automating: CHANGELOG generation (with git-cliff). Creation of GitHub/Gitea/GitLab releases. Publishing to a cargo registry (crates.io by default). Version bumps in Cargo.toml." 通过 Release PR 工作流,合并后自动打 tag 并 `cargo publish`。**角色:版本/发布编排,不编译二进制。**
- **taiki-e/upload-rust-binary-action**([README](https://github.com/taiki-e/upload-rust-binary-action),原文):"GitHub Action for building and uploading Rust binary to GitHub Releases."默认用 cross 交叉编译,归档命名 `$bin-$target.$extension`,支持 `checksum` 输入(sha256 等),并明确"this action is basically intended to be used in combination with an action like create-gh-release-action"。**角色:构建+上传二进制,不创建 release、不管版本。**
- **softprops/action-gh-release**([README](https://github.com/softprops/action-gh-release),原文):"A common case for GitHub releases is to upload your binary after its been validated and packaged." **角色:语言无关的通用 release 创建/资产上传器。**

典型组合:release-plz 管版本与 crates.io;upload-rust-binary-action 或 action-gh-release 管二进制资产。而 cargo-dist 是"全家桶",把上述全部职能打包(版本之外)——ripgrep/fd/bat 这一代用自定义 matrix CI,uv/ruff 这一代用 cargo-dist。

### 1.4 crates.io 发布的 Cargo.toml 硬性要求

[Cargo 官方 publishing 文档](https://doc.rust-lang.org/cargo/reference/publishing.html)(原文):

> "Before publishing, make sure you have filled out the following fields:
> - `license` or `license-file`
> - `description`
> - `homepage`
> - `repository`
> - `readme`
>
> It would also be a good idea to include some `keywords` and `categories`, though they are not required."

即 `description` 和 `license`/`license-file` 是必须填写的(缺了 crates.io 服务端会拒绝);`homepage`/`repository`/`readme` 官方要求填写,`keywords`/`categories` 明确"not required"。`documentation` 字段可选(docs.rs 自动托管)。参考:[manifest 格式文档](https://doc.rust-lang.org/cargo/reference/manifest.html)。

---

## 2. npm 包分发平台原生二进制的通行模式

### 2.1 模式 A:optionalDependencies + 平台子包(esbuild / swc / biome)

**npm 官方文档**对 `os`/`cpu` 字段的定义([docs.npmjs.com package-json](https://docs.npmjs.com/cli/v11/configuring-npm/package-json),原文):

> os: "You can specify which operating systems your module will run on: `"os": ["darwin", "linux"]` … The host operating system is determined by `process.platform`"
> cpu: "If your code only runs on certain cpu architectures, you can specify which ones. `"cpu": ["x64", "ia32"]` … The host architecture is determined by `process.arch`"

对 optionalDependencies 的行为(原文):"build failures do not cause installation to fail";"Running `npm install --omit=optional` will prevent these dependencies from being installed"。平台子包正是靠 "os/cpu 不匹配 → 作为 optional 依赖被静默跳过" 这一组合工作。

**esbuild**(实测 npm registry,[registry.npmjs.org/esbuild/latest](https://registry.npmjs.org/esbuild/latest),v0.28.2):
- 26 个 `@esbuild/<os>-<cpu>` 平台子包全部列在 `optionalDependencies`,版本号与主包严格钉死(如 `"@esbuild/linux-x64":"0.28.2"`)
- 同时仍保留 `"scripts":{"postinstall":"node install.js"}`——但它不再下载二进制。esbuild 官方文档([esbuild.github.io/getting-started](https://esbuild.github.io/getting-started/),原文):

> "the `esbuild` package depends on a separate optional package for each platform-specific binary executable. … The `esbuild` binary for the current platform should be automatically selected and installed from esbuild's optional dependencies by npm."
> "The install script then runs which a) checks that the esbuild binary is the correct version … and b) optimizes the `esbuild` command in `node_modules/.bin`"
> 仅当用户 `--no-optional` 时:"The install script then runs and notices the missing `esbuild` binary and attempts to download it manually from the npm registry. This is less robust than the default installation flow because many people have complex npm configuration that esbuild's install script can't necessarily replicate."

**@biomejs/biome**(实测 npm registry,[registry.npmjs.org/@biomejs/biome/latest](https://registry.npmjs.org/@biomejs/biome/latest),v2.5.14):主包 `optionalDependencies` 列出 8 个 `@biomejs/cli-*` 子包(linux-x64、linux-arm64、darwin-x64、darwin-arm64、win32-x64、win32-arm64、linux-x64-musl、linux-arm64-musl);子包(如 [@biomejs/cli-linux-x64](https://registry.npmjs.org/@biomejs/cli-linux-x64/latest))内含 `"os":["linux"],"cpu":["x64"],"libc":["glibc"]`。Biome 与 dynamic-diagram 的情况最像:子包里是**独立可执行文件**,由 JS wrapper 以子进程方式调用。

**@swc/core**(实测 npm registry,[registry.npmjs.org/@swc/core/latest](https://registry.npmjs.org/@swc/core/latest),v1.16.2):12 个 `@swc/core-<os>-<arch>-<abi>` optionalDependencies,子包内是 napi-rs 编译的 `.node` 原生模块(与 dynamic-diagram 场景不同,仅作模式佐证)。

**权衡**:离线友好(npm registry 是唯一依赖,企业代理/私服原生支持);版本锁定(lockfile 钉死子包);主包体积小。代价:每发一版要发布 N+1 个 npm 包(需要 CI 自动化);包管理器用 `--omit=optional` 时会坏(需运行时兜底报错);npm 之外(yarn/pnpm 旧版、某些私服)兼容性偶有问题。

### 2.2 模式 B:postinstall / 首次运行时从 GitHub Releases 下载并缓存

**puppeteer**([官方安装文档 pptr.dev/guides/installation](https://pptr.dev/guides/installation)、[配置文档](https://pptr.dev/guides/configuration)):
- postinstall 自动下载钉死版本的 Chrome for Testing;v19 起缓存到用户级目录 `~/.cache/puppeteer`(Windows 为 `%LOCALAPPDATA%`)
- 环境变量:`PUPPETEER_SKIP_DOWNLOAD`(跳过下载)、`PUPPETEER_CACHE_DIR`(自定义缓存位置)、`PUPPETEER_DOWNLOAD_BASE_URL`(镜像/代理)
- 被包管理器拦截 postinstall 时可手动 `npx puppeteer browsers install`
- `puppeteer-core` = 不带下载的纯库版本

**cargo-dist 的 npm installer**(见 1.2)本质也是这个模式:npm 包 install 后从 GitHub Releases fetch 归档、解包到 node_modules。esbuild 早期 install.js 同样是此模式,后来才迁移到模式 A(官方文档称其"less robust")。

**权衡**:不需要发布平台子包,npm 侧只发一个包;二进制天然可以复用 GitHub Releases 已有产物(与 Rust 侧发布流程解耦)。代价:install 时要联网,离线/CI 缓存复杂;包管理器默认拦截 postinstall 的趋势(pnpm、Bun)使其可靠性下降;版本一致性要靠脚本自己保证。缓解手段:下载校验 sha256、失败时给出手动安装指引、首次运行时惰性下载(比 postinstall 更不易被拦)。

### 2.3 模式 C:要求用户自行安装(PATH / 环境变量)

现状(pi-diagram 现行做法)。零 npm 侧复杂度,但用户体验最差,且版本兼容性完全失控——对"npm install 后即用"的编码助手扩展场景,业内(上述所有参照)没有这么做的先例。

---

## 3. cargo-binstall:从 GitHub Releases 直接装预编译二进制

[cargo-binstall SUPPORT.md](https://github.com/cargo-bins/cargo-binstall/blob/main/SUPPORT.md)(原文):

- 未配置 `[package.metadata.binstall]` 时,binstall 用一组默认模板猜测 release 资产 URL,文件名候选包括:
  - `{ name }-{ target }-{ version }{ archive-suffix }`
  - `{ name }-{ target }-v{ version }{ archive-suffix }`
  - `{ name }-{ version }-{ target }{ archive-suffix }`
  - `{ name }-v{ version }-{ target }{ archive-suffix }`
  - (以及 `_` 分隔变体和 `{ name }-{ target }` 无版本变体)
  GitHub 路径候选:`{ repo }/releases/download/{ version }/` 和 `{ repo }/releases/download/v{ version }/`(文件名 × 路径两两组合尝试)。
- 支持的 `pkg-fmt`([binstalk-types PkgFmt 枚举,docs.rs](https://docs.rs/binstalk-types/latest/binstalk_types/cargo_toml_binstall/enum.PkgFmt.html)):`Tar`、`Tgz`(默认)、`Txz`、`Tbz2`、`Tzstd`、`Zip`、`Bin`(裸二进制)。
- 可在 `Cargo.toml` 用 `[package.metadata.binstall]` 显式配置 `pkg-url`/`bin-dir`/`pkg-fmt`,并支持 `[package.metadata.binstall.overrides.<target-triple>]` 按平台覆盖(如 Windows 用 zip、其余用 tgz)。
- 模板变量:`name`、`version`、`repo`、`bin`、`target`、`archive-suffix`、`binary-ext`、`target-family`、`target-arch`、`target-libc` 等。
- 解析回退链:crates.io 元数据 → GitHub/GitLab 等 releases → cargo-quickinstall → `cargo install` 源码编译兜底;可选签名校验(`--only-signed`)。

**对 dynamic-diagram 的意义**:只要发布到 crates.io 且 release 资产命名符合上述任一默认模板(如 `dynamic-diagram-v0.1.0-x86_64-unknown-linux-musl.tar.gz` 或 `dynamic-diagram-x86_64-apple-darwin.tar.gz`),`cargo binstall dynamic-diagram` 即可零配置工作;命名不符时在 Cargo.toml 加几行 metadata 即可。

---

## 4. 汇总结论

### 业内最接近的参照

「npm 包 + 独立原生二进制」的两大参照系:

1. **esbuild / @biomejs/biome / @swc/core 模式**(optionalDependencies 平台子包)——npm 生态内最主流、最健壮,离线友好。其中 **Biome 与 dynamic-diagram 同构度最高**:Rust 写的独立可执行文件、JS wrapper 子进程调用。
2. **puppeteer / cargo-dist npm installer 模式**(install 时从 GitHub Releases 下载+缓存)——npm 侧零平台包负担,直接复用 Rust 侧的 release 产物。

### GitHub Releases 是否必要

**必要**。两条链路都以它为二进制托管底座:模式 B 直接消费它;模式 A 虽从 npm registry 提供,但发布流水线本身(构建 per-target 产物)与 ripgrep/fd/bat/uv/ruff 的通行做法完全一致,GitHub Releases 同时还是 cargo-binstall、Homebrew、 mise/aqua 等用户侧工具的取数源。命名建议采用 binstall 默认模板兼容的形式,校验和跟随 uv/ruff 惯例(每资产 `.sha256` + 汇总 `sha256.sum`)。

### 推荐分发链路取舍

| 方案 | 优点 | 缺点 | 适用性 |
|---|---|---|---|
| optionalDependencies 平台子包 | 离线/代理友好;lockfile 锁版本;npm 生态主流 | 每版发 N+1 个 npm 包;需 CI 自动化;`--omit=optional` 会坏 | 最健壮,但 pi 扩展 + 外部 Rust repo 的组合下需自行搭子包发布流水线 |
| postinstall/首运行时下载 + 缓存 | npm 侧一个包;直接复用 GitHub Releases;Rust/npm 发布解耦 | 需联网;postinstall 被拦截趋势;需自己做 sha256 校验与版本对齐 | 与 cargo-dist npm installer、puppeteer 同路线;改动最小 |
| 用户自装(PATH/ENV) | 零实现 | 体验差、版本失控;业内无先例 | 仅作兜底 |

---

## 5. 对 dynamic-diagram / pi-diagram 的建议清单

1. **dynamic-diagram 必须发 GitHub Releases 预编译二进制**,产物集对齐通行最小集:per-target `tar.gz`(Unix)/`zip`(Windows)+ 每资产 `.sha256`。最低目标集:`x86_64-unknown-linux-musl`(静态)、`aarch64-apple-darwin`、`x86_64-apple-darwin`、`x86_64-pc-windows-msvc`(可选 linux-gnu/aarch64-linux-musl)。`.deb`、签名(GPG/cosign)均非必须——抽样的五个旗舰项目都没有签名。
2. **release 资产命名用 cargo-binstall 默认模板兼容格式**,如 `dynamic-diagram-v0.1.0-x86_64-unknown-linux-musl.tar.gz`(fd/bat 式,带 `v`)或 `dynamic-diagram-0.1.0-x86_64-unknown-linux-musl.tar.gz`(ripgrep 式,不带 v),两者都在 binstall 默认候选里。这样发 crates.io 后 `cargo binstall dynamic-diagram` 零配置可用。
3. **发布工具优先 cargo-dist**(`dist init` 一次到位:matrix 构建、归档、sha256、installer.sh、可选 Homebrew/npm installer);若嫌全家桶重,最小组合是 release-plz(版本+crates.io)+ taiki-e/upload-rust-binary-action(构建+上传),或一个手写 matrix workflow + softprops/action-gh-release。
4. **发 crates.io 前补齐 Cargo.toml 元数据**:`description`、`license`(SPDX 表达式,如 `"MIT OR Apache-2.0"`)为硬性要求;`repository`、`readme`、`homepage` 官方要求填写;`keywords`/`categories` 可选(`cargo publish --dry-run` 预检)。
5. **pi-diagram 侧推荐「首运行时惰性下载 + 缓存 + 校验」模式**(puppeteer 式),而非 postinstall:从 dynamic-diagram 的 GitHub Releases 按 `process.platform`/`process.arch` 映射 target triple 下载对应归档,sha256 校验后缓存到 `~/.cache/pi-diagram/`(或 pi 约定目录),并用 `DYNAMIC_DIAGRAM_VERSION` 类机制钉死兼容版本。保留现有 `DYNAMIC_DIAGRAM_BIN` 与 PATH 探测作为最高优先级覆盖(对应 `PUPPETEER_EXECUTABLE_PATH` 式逃生舱)。
6. **不建议初期上 esbuild/biome 式平台子包**:需要每版发布多个 `@pi-diagram/bin-*` npm 包的自动化流水线,收益(离线 install)在 pi 扩展场景(本身就是联网编码助手)里不抵成本。若日后有离线/企业代理需求再迁移,届时 GitHub Releases 产物可直接作为子包内容来源。
7. **下载失败兜底**:网络/被拦时输出明确指引(手动下载 URL + `DYNAMIC_DIAGRAM_BIN` 用法),对应 esbuild 对 `--no-optional` 场景的做法。

---

## 附:调研方法说明

- GitHub Releases 资产清单均通过 `api.github.com/repos/<owner>/<repo>/releases/latest` 实测抓取(2026-09-21),非文档转述。
- npm 包结构均通过 `registry.npmjs.org/<pkg>/latest` 实测 JSON 确认。
- cargo-binstall 默认模板引自官方 SUPPORT.md;PkgFmt 枚举引自 docs.rs。
- 签名使用率的结论基于上述五个项目最新 release 的抽样,不代表全生态统计。
