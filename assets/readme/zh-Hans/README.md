<p><img src="../../readme-icon.png" alt="Dango 应用程序图标" width="147" height="147"></p>

# Dango

**与编程智能体协作的原生 macOS 编辑器。**

Dango 把代码、修改代码的智能体以及围绕它们的一切，按文件夹放进一个窗口：带语言服务器和编辑预测的编辑器、终端、浏览器、Git、CI、调试器，以及与 Claude、Codex、Antigravity、OpenCode 或 App 内置智能体进行的 AI 对话。每个智能体都在同一条时间线中工作，在更改文件或运行命令之前先询问你，并在每次发送提示词之前留下一个检查点，因此你可以逐条消息地查看并回退它所做的一切。在你信任文件夹之前，它所配置的任何内容都不会运行；来自外部的文本在交给智能体时会被标记为不可信。你可以在编辑器和消息栏中听写，语音识别可以完全留在你的 Mac 上。Dango 用 Rust 基于真正的 AppKit 控件编写，不使用 Electron，支持 35 种语言，没有遥测功能。

[![下载 Dango.dmg](https://img.shields.io/badge/%E4%B8%8B%E8%BD%BD_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[下载 Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: 已签名并经 Apple 公证的磁盘映像，适用于 Apple 芯片和 Intel 处理器上的 macOS 14 或更高版本。链接将在首个版本发布后可用。

**发现了错误？** 请[提交 Issue](https://github.com/sventmueller/dango/issues/new)，并说明问题出现在哪个智能体和哪类项目中。

[![Rust 2024 版](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 或更高版本](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![通用：Apple 芯片和 Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![通过 objc2 使用 AppKit](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![终端：Tapa](https://img.shields.io/badge/terminal-Tapa-F46D01)](https://github.com/sventmueller/tapa) ![35 种界面语言](https://img.shields.io/badge/languages-35-4C9A2A) ![无遥测](https://img.shields.io/badge/telemetry-none-4C9A2A) ![使用 Hardened Runtime 签名](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![许可证：MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#许可证)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · [Hrvatski](../hr/README.md) · [Italiano](../it/README.md) · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · [Polski](../pl/README.md) · [Português (Brasil)](../pt_BR/README.md) · [Português (Portugal)](../pt_PT/README.md) · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · **简体中文** · [繁體中文](../zh-Hant/README.md)

## 目录

- [功能一览](#功能一览)
- [系统要求](#系统要求)
- [安装](#安装)
- [快速上手](#快速上手)
- [AI 智能体](#ai-智能体)
  - [时间线](#时间线)
  - [权限与模式](#权限与模式)
  - [检查点与工作树](#检查点与工作树)
- [编辑器](#编辑器)
- [语言服务器](#语言服务器)
- [测试](#测试)
- [调试器](#调试器)
- [终端](#终端)
- [浏览器](#浏览器)
- [Git](#git)
- [CI](#ci)
- [容器](#容器)
- [Markdown](#markdown)
- [听写](#听写)
- [信任](#信任)
- [沙盒](#沙盒)
- [外观与辅助功能](#外观与辅助功能)
- [语言](#语言)
- [键盘快捷键](#键盘快捷键)
- [隐私与安全](#隐私与安全)
- [数据存放位置](#数据存放位置)
- [已知限制](#已知限制)
- [从源代码构建](#从源代码构建)
- [许可证](#许可证)

## 功能一览

- **所有智能体，同一条时间线。** Claude 通过它自己的 `claude` 命令运行，Codex、Antigravity 和 OpenCode 通过 Agent Client Protocol 运行，更多智能体可来自 ACP 注册表；此外还有一个 App 内置的智能体，支持 Anthropic、OpenAI、Google Gemini、OpenRouter、Ollama 和 LM Studio。无论运行的是哪个智能体，回答、工具调用、权限请求、计划、待办事项、子智能体和后台任务看起来都一样。
- **查看与回退。** 每次发送提示词之前，Dango 都会保存一个检查点。智能体所做的更改可以按消息查看和回退；对话还可以在独立的 Git 工作树中工作，并通过合并或 cherry-pick 交付其成果。
- **每个文件夹一个窗口。** 文件、终端、浏览器页面、AI 对话、差异和预览都是该窗口的 macOS 标签页。边栏包含“文件”“搜索”“智能体”“Git”“测试”“调试”“CI”和“容器”。
- **真正的代码编辑器。** 语法着色、多个插入点、折叠、`.editorconfig`，覆盖 17 种语言的语言服务器（支持补全、跳转到定义、引用、重命名、快速修复和层级），以及来自 GitHub Copilot 或本地模型的编辑预测。
- **内置调试器、Git 和 CI。** 适用于 Rust、C、C++、Swift、Go、JavaScript、TypeScript 和 Python 的断点和单步执行；暂存单个变更块、分支、储藏、冲突和提交图谱；来自 GitHub Actions 和 GitLab CI 的运行、作业和日志。
- **Tapa 的终端。** 命令块的输出可以拷贝、保存或交给智能体；输入栏提供来自历史记录和项目的建议；可搜索整个历史记录；终端中的程序可发出通知和进度。
- **谨慎对待你的文件夹。** 未获信任的文件夹不会启动任何东西：没有 Git 进程、语言服务器、钩子或智能体。具有破坏性的 Git 命令永远不会被永久允许，提交也会检查是否含有机密信息。
- **听写。** 对着编辑器和消息栏说话，可以通过 OpenAI 或 Groq 在线识别，也可以用 Whisper 和 Parakeet 在本地识别。
- **原生到骨子里。** AppKit 窗口、标签页、菜单和工具栏，编辑器使用 TextKit 2，终端使用 Metal，WebKit 仅用于浏览器标签页。深色模式、“增强对比度”和 VoiceOver 的效果与其他 Mac App 一样。
- **使用你的语言。** 界面提供 35 种语言：阿拉伯语、加泰罗尼亚语、简体中文、繁体中文、克罗地亚语、捷克语、丹麦语、荷兰语、英语、芬兰语、法语、德语、希腊语、希伯来语、印地语、匈牙利语、印度尼西亚语、意大利语、日语、韩语、马来语、挪威语、波兰语、葡萄牙语（巴西）、葡萄牙语（葡萄牙）、罗马尼亚语、俄语、斯洛伐克语、斯洛文尼亚语、西班牙语、瑞典语、泰语、土耳其语、乌克兰语和越南语。

## 系统要求

- macOS 14 Sonoma 或更高版本，搭载 Apple 芯片或 Intel 处理器。Dango 在 macOS 27 上开发和测试。
- AI 对话需要你想使用的智能体；请参阅 [AI 智能体](#ai-智能体)。
- 语言服务器和调试适配器会在你要求时由 Dango 安装，它们需要：gopls 和 `dlv` 需要 Go；TypeScript、Python、JSON、YAML、CSS、HTML 和 Shell 的服务器以及 JavaScript 调试器需要 Node.js；clangd、sourcekit-lsp 和 `lldb-dap` 需要 Xcode Command Line Tools。rust-analyzer 从其 GitHub 发布页下载。
- 编辑预测需要 GitHub Copilot 订阅方案（免费方案即可），或 Ollama 或 LM Studio 中的中间填充（fill-in-the-middle）模型。

## 安装

**通过磁盘映像。** 每个版本都是一个磁盘映像 `Dango-<version>.dmg`，内含一个同时适用于 Apple 芯片和 Intel 处理器的通用 App，使用 Developer ID 签名并经 Apple 公证。打开它，将 Dango 拖到“应用程序”文件夹。Homebrew Cask 安装的也是同一个映像。目前尚未发布任何版本。

**更新。** 通过磁盘映像安装的 Dango 会借助 [Sparkle](https://sparkle-project.org) 自行保持最新。第二次启动时，它会询问是否可以自行检查更新；在你同意之前，它不会发出任何请求。**Dango › 检查更新…** 会立即检查。更新使用 App 自己的密钥签名，Sparkle 会在安装任何内容之前先验证它。自行构建的版本没有更新功能。

**通过源代码。** 请参阅[从源代码构建](#从源代码构建)，然后将 `target/release/Dango.app` 移到 `/Applications`。

**终端命令。** **Dango › 安装“dango”命令…** 会将一个小脚本写入 `~/.local/bin/dango`，无需管理员权限；`~/.local/bin` 必须在你的 `PATH` 中。之后，`dango .` 会打开当前文件夹，`dango file.rs` 会打开一个文件。Dango 绝不会替换该位置中不是它自己写入的文件。

## 快速上手

1. 用 ⌘O 或 `dango <folder>` **打开文件夹**。第一次打开时，Dango 会询问你是否信任它（请参阅[信任](#信任)）。
2. **边栏**可在“文件”（⌘1）、“搜索”（⌘2）、“智能体”（⌘3）、“Git”（⌘4）、“测试”（⌘5）、“调试”（⌘6）、“CI”（⌘7）和“容器”（⌘8）之间切换；右键点按可隐藏不用的项目，⌃⌘S 可收起边栏。
3. **每个文稿都是文件夹窗口中的一个标签页**，即 macOS 自己的标签页。标签栏末尾的“+”和 ⌘T 可新建文件、终端、浏览器以及每个已安装的智能体，也可以放在独立的 Git 工作树中；按住“+”会以菜单形式显示相同的选项。
4. **快速打开**（⌘P）可查找文件，输入 `#` 可查找项目中的符号；⇧⌘F 在项目中搜索和替换。
5. 在 AI 标签页中 **与智能体交谈**。⌘Esc 会将键盘焦点移到它的消息栏，再按一次则移回。
6. 用 ⌘, 打开的 **设置** 包含以下标签页：“通用”“终端”“CI”“智能体”“Software Factory”“内置智能体”“语言服务器”“听写”和“预测”。

工具栏包含在编辑过的位置之间后退和前进（⌃- 和 ⌃⇧-）、分支、“快速打开”以及列出所有正在运行内容的“活动”概览。在文件的上下文菜单中选择 **在旁边打开**，会在第一个文稿旁边显示第二个文稿。重启或崩溃之后，每个窗口都会连同其标签页恢复，含有未保存文本的编辑器也会连同这些文本恢复。图像、音频、视频、字体和 Office 文稿通过快速查看在标签页中打开，PDF 通过 PDFKit 打开。

## AI 智能体

| 智能体 | 运行方式 | 所需条件 |
|---|---|---|
| Claude | `claude` 命令，通过其 stream-json 协议直接通信 | 已安装并登录 `claude`；Dango 会在登录 Shell 的 `PATH`、`~/.local/bin`、`~/.claude/local`、`/opt/homebrew/bin` 和 `/usr/local/bin` 中查找。登录只在 `claude` 自身中进行 |
| Codex | 通过 Agent Client Protocol（ACP）运行的 `codex-acp` | Node.js 和 npm；Dango 会在你要求时安装固定版本的软件包 |
| Antigravity | `agy acp` | `PATH` 中你自己的 `agy`，或在你要求时安装的固定版本下载 |
| OpenCode | `opencode acp` | `PATH` 中你自己的 `opencode`，或在你要求时安装的固定版本下载 |
| 其他 ACP 智能体 | 来自 ACP 注册表，仅在你要求时获取 | 注册表条目中列出的内容；没有校验和的条目只有在你确认后才会安装 |
| 内置智能体 | 内置于 App，带有用于文件、搜索、命令、诊断和网络请求的工具 | Anthropic、OpenAI、Google Gemini 或 OpenRouter 的 API 密钥，保存在钥匙串中；这台 Mac 上的 Ollama 或 LM Studio，无需密钥；任何兼容 OpenAI 或 Anthropic API 的服务器 |

**设置 › 智能体** 会显示这台 Mac 上有哪些智能体，安装其他智能体及其更新，并打开 ACP 注册表。Dango 安装的智能体固定为经过测试的版本，并根据其公布的校验和（npm integrity 或 SHA-256）进行验证；Dango 不运行任何未指定版本的 `npx` 软件包。

### 时间线

每个智能体都在 AI 标签页中工作，时间线都相同：回答、工具调用及其输出、权限请求、提问、可批准和评论的计划（⌥⌘K）、待办事项、子智能体和后台任务。标签页标题栏中的两个按钮会列出正在运行的子智能体和后台进程。提及一个文件、`@terminal` 或 `@browser`，即可将其内容交给智能体。标签页上的绿点表示会话正在工作，红点表示会话在等你；你在看别处时，会话完成或等待都会有通知提醒你。对于 Claude，状态行会显示上下文和用量。

### 权限与模式

新对话以“手动”模式开始：智能体在更改文件或运行命令之前会先询问。对于具有破坏性的 Git 命令（`push --force`、`reset --hard`、`clean -fd`），Dango 不提供“始终允许”。内置智能体按项目保存其“始终允许”规则，在 **设置 › 内置智能体** 中列出它们，并且无论保存了什么，每次遇到具有破坏性的 Git 命令都会询问；Claude 和 ACP 智能体将此类规则保存在它们自己的配置中。选择“全部允许”且没有沙盒时，对话会持续显示一条警告。

### 检查点与工作树

每次发送提示词之前，Dango 都会保存一个检查点：在项目仓库中作为 `refs/ide-checkpoints/` 下的提交，对于没有 Git 的项目则保存在它自己的文件夹中；普通的推送或抓取永远不会带上它们。智能体所做的更改可以按消息查看和回退。对话可以在独立的 Git 工作树中工作：Git 面板会显示它所做的更改，并通过合并或 cherry-pick 并入其工作。

## 编辑器

- 相对上次提交的 **语法着色、行号和更改标记**，查找和替换，快速打开（⌘P）和项目搜索（⇧⌘F）。
- **缩进** 遵循文件和 `.editorconfig`：制表符或空格、自动缩进、括号配对。
- **多个插入点：** ⌥-点按，⌥-拖移选择一列，⌃⇧↑ 和 ⌃⇧↓，⌘D 选择下一个匹配项。
- **折叠**（⌥⌘← 和 ⌥⌘→，加 ⇧ 则作用于全部）、切换注释（⌘/）、前往行（⌃G）、显示空白字符和自动换行（⌥Z）。
- **文件保留其编码**、字节顺序标记和行尾符。磁盘上的文件发生变化时，如果编辑器中没有未保存的文本，就会重新载入；否则会出现一个提示栏，提供差异和合并。
- **编辑预测。** 输入时会以灰色显示建议，按 Tab 即可采纳。建议来自 GitHub Copilot 或 Ollama、LM Studio 中的本地模型，取决于你在 **设置 › 预测** 中的选择；本地模型会让你的代码留在这台 Mac 上。
- **缩略图和粘性滚动**（“编辑器”菜单）：前者在文本旁以窄条显示整个文件，后者将顶行所在各代码块的标题行固定在其上方。
- **在行尾显示诊断**（“显示”菜单）：停顿一秒后，在每行末尾显示该行最严重的消息，并以符号标明其严重程度。
- **静态页面实时预览：** “编辑器 › 在浏览器标签页中打开”由 Dango 自身提供受信任文件夹中的页面，并在文件保存时重新载入标签页。
- **Prettier** 会在项目配置了它的情况下，使用项目自己的 `node_modules/.bin/prettier` 格式化 JavaScript、TypeScript、CSS、HTML、Markdown 等；设置项 `formatter` 可按语言选择格式化工具。
- **Vim 模式**（“编辑器 › Vim 模式”）：正常模式、插入模式、可视模式和替换模式，计数、配合移动和文本对象的操作符、`.`、寄存器、标记和宏。`u` 和 ⌃R 使用编辑器的撤销，一次连续插入算作一步。`"+` 和 `"*` 即剪贴板；设置项 `vimClipboard` 会让无名寄存器也成为剪贴板。`/`、`?`、`n`、`N`、`*`、`:s` 和 `:%s` 使用 Rust 的正则表达式搜索，而不是 Vim 的模式：不支持 `\v`，用 `\b` 代替 `\<` 和 `\>`。Ex 命令有 `:w`、`:q`、`:wq`、`:x`、`:noh` 和 `:<line>`。在正常模式下，⌃G 交给 Vim，而不是“前往行”。
- **设置同步**（**设置 › 通用**）：通过 iCloud 云盘中的一个文件夹，你的设置和快捷键会跟随你到其他 Mac 上；路径、本地地址和各项同意则保留在每台 Mac 上。

## 语言服务器

Go、Rust、TypeScript 和 JavaScript、Python、C、C++ 和 Objective-C、Swift、Shell 脚本、JSON、YAML、CSS、HTML、TOML、Markdown 和 Lua，以及你在 **设置 › 语言服务器** 中添加的服务器。Dango 会在你要求时将服务器安装到它自己的工具文件夹中；如果 TypeScript 项目自带的 `tsc` 可作为语言服务器，就会使用它。

它们带来的功能：诊断（⌘' 和 ⇧⌘' 在诊断之间移动）、悬停信息（⌃⌘I）、补全（⌥Esc）、签名帮助、跳转到定义（⌃⌘J）、类型定义和实现、引用（⌃⇧⌘F）、调用层级和类型层级（⌃⇧⌘H）、重命名（⌃⌘E）、格式化（⌥⇧F）、语义着色、折叠和大纲（⇧⌘O）；在行内显示类型和参数名称；灯泡（⌥⌘A）后的快速修复和重构；用 ⌥⌘O 查找项目中的符号。

除了文件自身的服务器，ESLint 还会检查使用它的项目中的 JavaScript 和 TypeScript，并提供其快速修复；Emmet 的缩写会出现在 HTML、CSS 和 JSX 的补全中。Dango 会在你要求时安装这两者；如果你愿意，**设置 › 通用** 可在保存时修复 ESLint 的问题。

## 测试

“测试”面板按包和文件列出文件夹中的测试及其状态，支持 `cargo test` 或 cargo-nextest、Go、pytest、Vitest 和 Jest。行号栏中每个测试旁的符号可运行该测试，⌥-点按可调试它，失败的断言会标记所在行。“调试”菜单可运行插入点处的测试（⌃⌥⌘U）、再次运行上次的测试（⌃⌥⌘G）或运行全部测试，也可调试插入点处的测试。测试只在受信任的文件夹中运行，因为它们会运行项目的代码；`cargo test` 只在你要求时列出测试，因为列出测试需要构建每个测试程序。

## 调试器

断点（也支持条件断点和日志点）、单步执行、线程及其调用堆栈、检查器（⌥⌘I）中的变量和监视表达式、程序暂停时显示在代码旁的值，以及在已暂停栈帧中求值的调试控制台。

| 语言 | 调试适配器 |
|---|---|
| Rust、C、C++、Swift | Xcode Command Line Tools 中的 `lldb-dap` |
| Go | `dlv` |
| JavaScript、TypeScript | 在 Node.js 上运行的 JavaScript 调试器 |
| Python | debugpy |

Dango 会读取项目的 `.vscode/launch.json`。“调试”菜单使用 Xcode 的快捷键：⌘\\ 设置断点，⌘Y 打开或关闭所有断点，⌃⌘Y 继续，F6、F7 和 F8 分别为单步跳过、单步进入和单步跳出，⌘. 停止。

## 终端

终端以标签页形式打开（**文件 › 新建终端**，⌃\`），使用登录 Shell 及其环境。终端来自 [Tapa](https://github.com/sventmueller/tapa)，拥有自己的 VT 内核，由 Metal 绘制：支持 xterm 和 VT520，以及 Kitty 键盘协议、OSC 8 链接、同步输出、真彩色和鼠标报告，使用编辑器的文字大小以及 Tapa 的浅色和深色配色。它的 Shell 集成适用于 zsh、bash 和 fish，会自行设置而不改动你的配置文件，当你用 `ssh` 连接到另一台主机时也会随之带上。重启后，终端会在原来的文件夹中以新的 Shell 恢复，但仅限受信任的文件夹；之前显示的内容不会恢复。

- **命令块。** 每条命令及其输出构成一个块。⌘↑ 和 ⌘↓ 在提示符之间跳转。**选择块**（⇧⌘A）、**拷贝输出**（⌥⌘C）、**保存输出…**（⌥⌘S）和 **折叠/展开块**（⇧⌘↩）作用于当前块；**设置/移除书签**（⇧⌘M）标记一行，⇧⌘↑ 和 ⇧⌘↓ 在书签之间跳转。开启 **将命令标记为块** 后，边距中的竖条会标出每个块，失败的命令会被标记，正在运行的命令在其输出滚动时停留在顶部，指针下方的块上会出现按钮，显示其状态、耗时以及同样的操作。
- **将块发送给智能体**（“编辑”菜单和终端的上下文菜单）会打开一个新的 AI 标签页，将当前块的命令行和输出作为“外部内容”附上（请参阅[信任](#信任)），并在命令失败时附上一份询问原因的草稿，就像失败的 CI 作业日志那样。在你于 `keybindings.json` 中为 `terminal.blockToAgent` 指定快捷键之前，它没有快捷键。
- **输入栏。** **显示 › 显示/隐藏输入栏**（⌘I）会把命令行变成终端底部的一个文本栏，上方显示文件夹和 Git 分支；新终端会沿用你上次的选择。Return 把整个输入栏发送给 Shell。它需要 Shell 集成，遇到 vim、htop 或 claude 等全屏程序时会让开。
- **建议** 以灰色紧跟在你输入的内容之后显示，无论是否使用输入栏：来自你的历史记录（附带每条命令运行时所在的文件夹、分支和退出码）、700 多个命令行工具的规格，以及项目，例如 git 分支、docker 容器、`package.json` 脚本，以及 make、just 和 cargo 目标。按 → 采用建议；Tab 进行补全，有多个候选项时会打开一个菜单，继续输入即可筛选。在你输入之前，输入栏可以建议通常接下来会运行的命令。命令会按组成部分着色，Shell 无法运行的命令会带有红色虚线下划线。建议从不联网，并且只从登录 Shell 的 `PATH` 启动程序。
- **查找**（终端拥有焦点时按 ⌘F、⌘G 和 ⇧⌘G）使用正则表达式、区分大小写和全字匹配搜索整个历史记录，并统计结果数；滚动条会标出结果、提示符和失败的命令。
- **选择和拷贝。** 连按两次可整体选中 URL、路径、哈希值或 IP 地址，按住 ⌥ 拖移可选中矩形区域。**使用键盘选择**（⇧⌘C）用 Vim 的按键移动选区，**带格式拷贝**（⇧⌥⌘C）保留颜色和样式。程序只有在其终端拥有键盘焦点时才能通过 OSC 52 拷贝，终端会告诉你这一点；任何程序都无法读取剪贴板。
- **粘贴。** 会在换行处执行命令的文本，或含有控制字符的文本，会先显示在一个表单中，除非程序将其作为括号粘贴读取。粘贴图像时，会将其交给在终端中运行的编程智能体，由它自己从剪贴板读取；否则会粘贴保存在缓存中的副本的路径；通过 `ssh` 时，终端会说明为什么不这样做。拖到终端上的文件（包括图片）会以加引号的路径出现；从别处拖入的图片（例如来自网页）会像粘贴的图片一样处理。
- **链接。** ⌘-点按会在浏览器标签页中打开网址，在“邮件”中打开 `mailto:` 链接，在 Dango 的编辑器中打开文件名（也可带行号和列号）；即使没有 OSC 8 也能识别链接。其他类型的链接不会打开。
- **开发服务器。** 当终端中的程序启动开发服务器时，终端上方的横条会显示其地址，**在浏览器标签页中打开** 会在那里打开它。
- **通知和进度。** 响铃以及程序请求的通知（OSC 9 和 OSC 777）会在你看不到终端时，在终端的标签页上显示红点；当其他 App 位于前面时，通知还会发送到 macOS，点按通知即可回到终端。程序报告的进度（OSC 9;4）会以圆环显示在其标签页上，并以进度条显示在程序坞图标上。
- **安全键盘输入**（**Dango › 安全键盘输入**）可防止其他 App 读取你在终端中键入的内容；当程序要求输入密码时，它也会自动开启。
- **按键。** ⌘K 清空终端（**编辑 › 清空终端**）；⌃C 和 Esc 会发送给终端中的程序。终端的命令位于“编辑”和“显示”菜单中，其快捷键仅在终端拥有键盘焦点时起作用，因此 ⌘I、⌘↑ 或 ⌘F 在编辑器中保持原有含义。上下文菜单还可以打开 **在此处新建终端**，并用 **关闭窗格** 关闭终端。

**设置 › 终端** 可切换新终端的输入栏、其 Vim 按键、建议、命令着色、未知命令的下划线、命令块、立即拷贝所选内容、先显示多行粘贴，以及将 ⌥ 用作 Meta 键。

## 浏览器

浏览器标签页，带有地址栏（⌘L）、后退和前进（⌘[ 和 ⌘]）、重新载入（⌘R）、搜索、先询问再下载，以及网页检查器。`@browser` 会将页面交给智能体，并标记为外部文本。

## Git

Git 面板可以暂存和丢弃整个文件或单个变更块、提交、提交并推送或同步、切换和创建分支、保存储藏、解决合并冲突，并绘制近期提交的图谱。Blame 显示谁最后更改了光标所在行，也可以为整个文件显示一列。一个按钮可用 AI 撰写提交信息；只有在 gitleaks 未在差异中发现机密信息后，差异才会发出，而且每次提交同样会检查机密信息。

## CI

GitHub Actions 和 GitLab CI，也支持 GitHub Enterprise Server 和自托管 GitLab：

- 当前分支的运行、作业和日志；
- 重新运行失败的作业、单个作业或（在 GitHub 上）整个运行；启动手动作业或批准等待中的部署；启动新的流水线或可手动触发的工作流；取消；
- 打开的拉取请求或合并请求以及议题，它们会在浏览器标签页中打开；
- 将失败作业的日志交给智能体，并标记为外部文本。

Token 来自钥匙串、`gh` 或 `glab`、登录 Shell 的环境，或你在 **设置 › CI** 中输入的 Token。

## 容器

“容器”面板显示 Docker 的 Compose 项目及其服务、单独的容器、镜像和卷，并实时跟随 Docker 的事件。文件夹中的 Compose 文件即使还没有任何容器，也会显示为项目。容器可以启动、停止和重新启动；它们的日志和 Shell 在终端标签页中打开，已发布的端口在浏览器标签页中打开。删除容器、镜像或卷以及清理未使用的数据之前都会先询问。Compose 的 up、down 和 pull 只在受信任的文件夹中运行，因为 Compose 会构建并运行项目所描述的内容。面板的菜单为窗口选择 Docker 上下文，而不改变你的 Shell 所用的上下文；相同的命令也在“容器”菜单中。需要 `docker` 命令行和 Docker 24.0.6 或更高版本。

## Markdown

- **预览。** Markdown 文件打开时，预览会显示在旁边；**显示 › 显示/隐藏旁边的预览**（⇧⌘V）可关闭和打开预览，**设置 › 通用** 可关闭自动预览。预览会绘制公式、Mermaid、Graphviz 和 PlantUML 图表以及文件夹中的图片；来自网络的图片只在受信任的文件夹中，或在你于预览中请求后才会载入。
- **“格式”菜单：** 粗体（⌘B）、斜体（⌘I）、删除线（⇧⌘X）、代码（⌥⇧⌘C）、链接（⇧⌘K）、标题 1–3（⌥⌘1–⌥⌘3）、项目符号列表和编号列表、任务、引用和代码块。每个命令都作用于所选内容或所在行，可来回切换，并且只需一步即可撤销。
- **粘贴。** 来自 Word 或网页的文本会以 Markdown 形式粘贴，其中的图片会作为文件保存在文稿旁边；⌥⇧⌘V 粘贴纯文本。
- **导出。** **文件 › 导出** 可写出 PDF、内含图片的单个 HTML 文件或 Word 文稿。只有从受信任的文件夹导出时，才会带上来自网络的图片。
- **在 Splat 中打开。** 如果安装了 [Splat](https://github.com/sventmueller/splat)，**文件 › 在 Splat 中打开** 以及文件树和标签页的上下文菜单会先保存文稿，再在 Splat 中打开它。

## 听写

在 AI 标签页的消息栏或编辑器中听写：使用那里的麦克风按钮、**编辑 › 开始听写**（⌃⌥D）或 fn 键（按住即录音，连按两下则持续录音）。语音识别来自 [Lowspeak](https://github.com/sventmueller/lowspeak)；无需安装 Lowspeak 本身。

- **设置 › 听写** 用于设置模式和模型（通过 OpenAI 或 Groq 在线识别，通过 Whisper 或 Parakeet 在本地识别）、语言、后期处理、麦克风以及你自己的词汇。
- **本地模型** 会在你选择时从 Hugging Face 下载，显示进度并可取消。模型会存放在 Lowspeak 使用的文件夹中，因此两个 App 都能使用对方下载的模型。本地模型会让录音留在这台 Mac 上。
- **在线转写** 需要 OpenAI 或 Groq 的密钥，Dango 会将其保存在自己的钥匙串项目中。
- **从 Lowspeak 迁移。** 如果 Lowspeak 的设置与默认值不同，“听写”标签页会提议一次性接管其设置，并在你要求时接管其密钥；Dango 读取密钥之前，macOS 会先询问你（请选择“允许”，而不是“始终允许”）。
- **权限。** 第一次听写时，macOS 会请求麦克风权限。fn 键在 Dango 为当前 App 且 Lowspeak 未运行时有效；它需要“辅助功能”权限，Dango 会在第一次听写之后请求该权限，绝不会在启动时请求。

## 信任

文件夹可以让程序在其中有内容启动时立即运行：Git 钩子和 `core.fsmonitor` 等仓库设置、来自 `node_modules` 的语言服务器、智能体钩子、`.mcp.json`。第一次打开文件夹时，Dango 会询问你是否信任它。

- **受信任：** Git、语言服务器、调试器、智能体和 CI 会为此文件夹及其下的所有内容运行。
- **受限制：** 你可以阅读和编辑；编辑器中的更改标记来自直接读取 `.git`。文件夹所配置的任何内容都不会启动。Git、CI 和 AI 面板会说明这一点，并提供信任该文件夹的选项。

不是来自你的文本可能含有意图操纵智能体的指令（提示词注入）。因此，终端的输出（`@terminal`，或交给智能体的命令块）、浏览器页面（`@browser`）、交给智能体的 CI 日志、项目外的文件，以及从网页粘贴的较长文本，在传递时都会带有标记：标签上显示“外部内容”，智能体收到文本时会附带一条说明，指出这是不可信的数据，不得遵循其中的指令。内置智能体获取的网页也带有同样的说明。当浏览器在剪贴板中注明来源时（Safari、Chrome 和 Firefox 都会这样做），粘贴的文本会被视为来自网页；来自其他地方的文本，以及只有几个词的短语，仍算作你自己的文本。

## 沙盒

Dango 本身不在 App Sandbox 中运行，因为它启动的每个程序都会继承沙盒：终端、Git、语言服务器和智能体将无法工作。作为替代，每个 AI 对话都有一个沙盒开关，默认关闭。它限制什么取决于智能体，开关的工具提示会说明这一点：

| 智能体 | 打开沙盒时 |
|---|---|
| Claude | Shell 命令在 Claude 自己的沙盒中运行；它的文件和网络工具则不受限制 |
| Codex | 命令和更改只能写入项目内，且无法访问网络 |
| 内置智能体 | 文件、网络请求和命令都限制在项目和允许的文件夹内，且无法访问网络（`sandbox-exec`） |
| Antigravity、OpenCode | 没有 Dango 可以开启的沙盒 |

## 外观与辅助功能

- **深色模式** 跟随系统；**增强对比度** 和 **减弱动态效果** 也会得到遵循。
- **VoiceOver** 会为边栏、标签页、AI 时间线中的卡片、Git 和 CI 行、调试器提供名称，朗读终端的各行并播报新的输出。断点绝不会只靠颜色来区分。
- **键盘。** 每个命令都有快捷键，所有快捷键都可以在 `keybindings.json` 中更改；⌥⌘\` 移到下一个面板。
- **从右到左。** 在阿拉伯语和希伯来语中，窗口会左右镜像，而代码视图和终端的网格仍保持从左到右。

Dango 在 AppKit 之上按领域补充的内容，请参阅 [ACCESSIBILITY.md](../../../ACCESSIBILITY.md)。

## 语言

界面已翻译为 35 种语言：阿拉伯语、加泰罗尼亚语、简体中文、繁体中文、克罗地亚语、捷克语、丹麦语、荷兰语、英语、芬兰语、法语、德语、希腊语、希伯来语、印地语、匈牙利语、印度尼西亚语、意大利语、日语、韩语、马来语、挪威语、波兰语、葡萄牙语（巴西）、葡萄牙语（葡萄牙）、罗马尼亚语、俄语、斯洛伐克语、斯洛文尼亚语、西班牙语、瑞典语、泰语、土耳其语、乌克兰语和越南语。

Dango 遵循 **系统设置 › 通用 › 语言与地区** 中的语言顺序；你也可以在那里的 **应用程序** 下单独为 Dango 选择语言。如果没有你所用语言的翻译，它会使用英语。术语遵循同一份词汇表 [docs/glossary.md](../../../docs/glossary.md) 和 Apple 自己的翻译，因此 Dango 读起来与 Mac 上的其他部分一致。

## 键盘快捷键

| 操作 | 快捷键 |
|---|---|
| 打开文件夹、新建窗口、关闭窗口 | ⌘O, ⇧⌘N, ⇧⌘W |
| 新建文件、保存、另存为 | ⌘N, ⌘S, ⇧⌘S |
| 新建标签页、关闭标签页、下一个和上一个标签页 | ⌘T, ⌘W, ⇧⌘] 和 ⇧⌘[ |
| 快速打开、在项目中搜索 | ⌘P, ⇧⌘F |
| 文件、搜索、智能体、Git、测试、调试、CI、容器 | ⌘1 至 ⌘8 |
| 显示或隐藏边栏、下一个面板 | ⌃⌘S, ⌥⌘\` |
| 在编辑过的位置之间后退和前进 | ⌃- 和 ⌃⇧- |
| 切换注释、前往行 | ⌘/, ⌃G |
| 下一个匹配项、在上方和下方添加插入点 | ⌘D, ⌃⇧↑ 和 ⌃⇧↓ |
| 折叠、展开、全部折叠、全部展开 | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| 自动换行、检查器 | ⌥Z, ⌥⌘I |
| 补全、悬停信息、跳转到定义 | ⌥Esc, ⌃⌘I, ⌃⌘J |
| 引用、调用层级、重命名 | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| 文件中的符号、项目中的符号 | ⇧⌘O, ⌥⌘O |
| 快速修复、格式化 | ⌥⌘A, ⌥⇧F |
| 下一个和上一个问题 | ⌘' 和 ⇧⌘' |
| Markdown 预览、粘贴为纯文本 | ⇧⌘V, ⌥⇧⌘V |
| 粗体、斜体、删除线、代码、链接 | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| 标题 1 至 3 | ⌥⌘1 至 ⌥⌘3 |
| 新建终端、清除终端 | ⌃\`, ⌘K |
| 在终端中：查找、查找下一个、查找上一个 | ⌘F, ⌘G, ⇧⌘G |
| 在终端中：上一个和下一个提示符、上一个和下一个书签、设置或移除书签 | ⌘↑ 和 ⌘↓, ⇧⌘↑ 和 ⇧⌘↓, ⇧⌘M |
| 在终端中：选择块、拷贝输出、保存输出、折叠块 | ⇧⌘A, ⌥⌘C, ⌥⌘S, ⇧⌘↩ |
| 在终端中：输入栏、使用键盘选择、带格式拷贝 | ⌘I, ⇧⌘C, ⇧⌥⌘C |
| 聚焦智能体的消息栏、评论计划 | ⌘Esc, ⌥⌘K |
| 地址栏、重新载入、后退、前进（浏览器） | ⌘L, ⌘R, ⌘[, ⌘] |
| 开始或停止听写 | ⌃⌥D |
| 断点、打开或关闭所有断点 | ⌘\\, ⌘Y |
| 运行插入点处的测试、再次运行上次的测试 | ⌃⌥⌘U, ⌃⌥⌘G |
| 继续、单步跳过、单步进入、单步跳出、停止 | ⌃⌘Y, F6, F7, F8, ⌘. |
| 设置 | ⌘, |

App 数据文件夹中的 `keybindings.json`（请参阅[数据存放位置](#数据存放位置)）可以更改其中任何一个快捷键。

## 隐私与安全

- **没有遥测，** 没有分析，也不会向任何地方发送崩溃报告。错误会写入系统日志（“控制台” App 中，位于 App 名称下）。
- **只为你启动的操作使用网络：** 你使用的智能体，各自与自己的服务通信；你要求时的 ACP 注册表，以及智能体、语言服务器、调试适配器和听写模型的下载；你登录的 CI 主机；你选择后来自 Copilot 的编辑预测；内置智能体的提供商；你选择后的在线转写；你在浏览器标签页中打开的网页；以及你允许后的更新检查。
- **代码离开 Mac**，只发生在你为此选择了服务的地方：智能体、Copilot 的预测、由 AI 撰写的提交信息。用于撰写提交信息的差异，只有在 gitleaks 未在其中发现机密信息后才会发出。
- **密钥和 Token** 保存在 macOS 钥匙串中，绝不会写入设置文件。其他 App 的密钥（例如 Lowspeak 的密钥）只有在你同意时才会读取，而且只读取一次。
- **固定版本的下载。** Dango 安装的智能体、语言服务器和调试适配器都固定为经过测试的版本，并根据其校验和进行验证。
- **代码签名** 使用 Hardened Runtime；更新只有带有 App 自己的签名才会安装。

## 数据存放位置

`<id>` 是包标识符，正式版为 `local.agentide`（调试版为 `local.agentide.debug`）。它不包含显示名称，因此重命名后所有数据都会保留。

| 位置 | 内容 |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`、`state.json`、`keybindings.json`、`trust.json`、`terminal-history.sqlite`（终端的命令历史记录）；`threads/`（AI 面板的列表）、`agent-threads/`（内置智能体的对话）、`layouts/`（每个项目的标签页）、`recovery/`（崩溃后未保存的编辑器）、`checkpoints/`（用于没有 Git 的文件夹）、`worktrees/`、`agents/` 和 `tools/`（已安装的智能体、语言服务器和调试适配器）、`task-output/` |
| `~/Library/Caches/<id>/` | 粘贴的图像、Shell 集成脚本、ACP 注册表；可以删除 |
| `~/Library/Logs/<id>/dictation/` | 听写的日志文件 |
| `~/Library/Application Support/Lowspeak` 和 `FluidAudio` | 本地听写模型，与 Lowspeak 共享 |
| `~/Library/WebKit/<id>/` | 浏览器标签页的 Cookie 和网站数据 |
| `<project>/.dango/settings.json` | 项目自己的设置 |
| 项目的 Git 仓库 | 检查点，以 `refs/ide-checkpoints/` 下的提交形式保存；普通的推送或抓取永远不会带上它们 |
| 钥匙串，服务 `<id>` | 内置智能体和听写的 API 密钥、CI Token |
| `~/.local/bin/dango` | 终端命令（如果你安装了它） |

Claude 将自己的会话保存在 `~/.claude/projects` 下；Dango 会读取它们以恢复会话，但不会更改它们。

## 已知限制

- **App 本身没有 App Sandbox**，因为终端、Git、语言服务器和智能体都会继承它；文件夹必须先获得信任，其配置的任何内容才会运行。Antigravity 和 OpenCode 没有 Dango 可以开启的沙盒。
- **Claude 的用量数据和上下文明细** 依赖其协议中标记为实验性的部分，缺少这些部分时会隐藏。
- **回退对话内容** 仅适用于 Claude；对于其他智能体，只回退代码。
- **通过浏览器登录 CI** 的功能已关闭；请使用 `gh`、`glab`、Token 或环境变量。
- **终端中不显示图片：** Kitty 图形、Sixel 和 iTerm2 图片不会显示。
- **不提供 Gemini CLI：** Google 已停止使用个人账户登录，并引导用户使用 Antigravity。以前的 Gemini 对话仍可阅读。

## 从源代码构建

你需要：

- macOS 14 或更高版本，并安装 Xcode Command Line Tools（`xcode-select --install`）；不需要 Xcode 本身。
- `rust-toolchain.toml` 所指定的 Rust。Homebrew 的 `rust` 只为这台 Mac 的处理器构建；正式版的通用构建需要带有目标 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 的 rustup。
- [splat](https://github.com/sventmueller/splat)、[lowspeak](https://github.com/sventmueller/lowspeak) 和 [tapa](https://github.com/sventmueller/tapa) 的检出副本，放在本仓库旁边（`../splat`、`../lowspeak`、`../tapa`），各自位于自己的 `main` 分支；工作区按路径从它们引入 crate，终端来自 Tapa。
- cmake，用于每个包含听写的构建（`make run`、`make build`、`make release`），以构建 whisper.cpp。`make check` 不需要它。

| 命令 | 作用 |
|---|---|
| `make run` | 构建调试版 App（`target/debug/Dango Debug.app`）并打开它 |
| `make build` | 构建正式版 App（`target/release/Dango.app`），使用首次构建时在登录钥匙串中创建的本地证书签名；保持 `Cargo.toml` 不变 |
| `make lint` | 格式化和 clippy；有警告即失败 |
| `make check` | 先 `lint`，然后运行所有测试；不需要钥匙串、窗口或网络 |
| `make test` | 先 `check`，然后在真实 App 中运行自检 `probe` 和辅助功能审查，分别按从左到右和从右到左 |
| `make perf` | 在正式版构建中检查时间预算；只有在 Mac 没有其他负载时才有意义 |
| `make release` | 将正式版演练到 `target/release-dry-run/`，使用本地证书签名；不向任何地方发送任何内容 |
| `make release-notarized` | 先 `check`，然后将正式版本身发布到 `dist/`：Developer ID 签名、公证、使用 App 自己的密钥签名的更新源；保持 `Cargo.toml` 不变 |
| `make deploy` | 在干净的 `main` 上，无论有无 CI 印章：公证下一个版本，提交 `Cargo.toml`，上传发布文件，推送代码和版本标签（若有绿色 CI 印章则一并推送），然后自行发布该版本 |

调试版和正式版是两个独立的 App，各有自己的设置和数据。构建、测试、自检、发布以及代码遵循的规则见 [docs/development.md](../../../docs/development.md)，其背后的决策见 [docs/plan.md](../../../docs/plan.md)。发布说明是 [RELEASE_NOTES.md](../../../RELEASE_NOTES.md)，其译文与每份 README 一起放在 `assets/readme/<language>/` 中。

## 许可证

Dango 依据以下任一许可证授权

- Apache License, Version 2.0（[LICENSE-APACHE](../../../LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](../../../LICENSE-MIT)）

由你选择。

除非你明确另行声明，否则你有意提交以纳入 Dango 的任何贡献，按 Apache-2.0 许可证中的定义，均应按上述方式双重授权，不附加任何其他条款或条件。
