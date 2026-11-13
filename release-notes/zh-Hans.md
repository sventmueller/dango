<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
这是 Dango 的首个版本。Dango 是一款原生 macOS 编辑器，专为与编程智能体协作而设计。编辑器、终端、浏览器、Git、CI、调试器和 AI 对话共用每个文件夹的同一个窗口。App 用 Rust 基于 AppKit 编写；WebKit 仅用于浏览器标签页。界面支持 35 种语言。

## 系统要求
- macOS 14 或更高版本，搭载 Apple 芯片或 Intel 处理器。
- AI 对话需要你所选的智能体：Claude 需要已登录的 `claude` 命令；Codex 需要 Node.js，App 会在你要求时安装其固定版本的软件包；Antigravity 和 OpenCode 使用你自己的 `agy` 或 `opencode`，或固定版本的下载；内置智能体需要 API 密钥，或这台 Mac 上的 Ollama 或 LM Studio。
- 语言服务器和调试适配器会在你要求时安装。gopls 和 `dlv` 需要 Go；TypeScript、Python、JSON、YAML、CSS、HTML 和 Shell 的服务器以及 JavaScript 调试器需要 Node.js；clangd、sourcekit-lsp 和 `lldb-dap` 随 Xcode Command Line Tools 提供。
- 编辑预测需要 GitHub Copilot 订阅（免费版即可），或 Ollama 或 LM Studio 中的中间填充（fill-in-the-middle）模型。

## 窗口
- 每个文件、终端、浏览器页面、AI 对话、差异和预览都是文件夹窗口中的一个 macOS 标签页。标签栏末尾的“+”和 ⌘T 可新建文件、终端、浏览器以及每个已安装的智能体，也可以放在独立的 Git 工作树中。
- 边栏可在“文件”“搜索”“智能体”“Git”“CI”和“调试”之间切换（⌘1–⌘6）；右键点按可隐藏不用的项目。工具栏包含在编辑过的位置之间后退和前进（⌃- 和 ⌃⇧-）、分支、“快速打开”以及“活动”概览。
- “在旁边打开”会在第一个文稿旁边显示第二个文稿。重启或崩溃后，每个窗口都会连同其标签页恢复，含有未保存文本的编辑器也会连同这些文本恢复。
- 图像、音频、视频、字体和 Office 文稿通过 Quick Look 在标签页中打开，PDF 通过 PDFKit 打开。

## 编辑器
- 语法着色、行号、相对上次提交的更改标记、查找和替换、“快速打开”（⌘P）以及项目搜索（⇧⌘F）。
- 自动缩进、按 `.editorconfig` 使用制表符或空格、括号配对、切换注释、折叠、前往行、显示空白字符和自动换行。
- 多个插入点：⌥-点按，⌥-拖移选择一列，⌘D 选择下一个匹配项。
- 文件保留其编码、字节顺序标记和行尾符。磁盘上的文件发生变化时，如果编辑器中没有未保存的文本，就会重新载入；否则会出现一个提示栏，提供差异和合并。
- **编辑预测：**输入时会以灰色显示建议，按 Tab 即可采纳。建议来自 GitHub Copilot 或 Ollama、LM Studio 中的本地模型，取决于你在设置中的选择；本地模型会让你的代码留在这台 Mac 上。

## 语言服务器
Go、Rust、TypeScript 和 JavaScript、Python、C、C++ 和 Objective-C、Swift、Shell 脚本、JSON、YAML、CSS、HTML、TOML、Markdown 和 Lua，以及你在设置中添加的服务器。诊断、悬停信息、补全、签名帮助、跳转到定义、类型定义和实现、引用、调用层级和类型层级、重命名、格式化、语义着色、折叠和大纲；在行内显示类型和参数名称；灯泡（⌥⌘A）后的快速修复和重构；用 ⌥⌘O 或在“快速打开”中输入 `#` 查找项目符号。如果 TypeScript 项目自带的 `tsc` 可作为语言服务器，就会使用它。

## 调试
断点（也支持条件）、单步执行、线程及其调用堆栈、检查器（⌥⌘I）中的变量和监视表达式、程序暂停时显示在代码旁的值，以及在已暂停栈帧中求值的调试控制台。Rust、C、C++ 和 Swift 通过 `lldb-dap`，Go 通过 `dlv`，JavaScript 和 TypeScript 通过 JavaScript 调试器，Python 通过 debugpy。会读取项目的 `.vscode/launch.json`。“调试”菜单使用 Xcode 的快捷键。

## 终端
终端在标签页中运行，使用登录 Shell 的环境，并由 Metal 绘制。粘贴或拖放图像时，会将其交给在终端中运行的编程智能体，或粘贴已存副本的路径。⌘-点按可打开链接和文件名。终端启动的开发服务器可以在浏览器标签页中打开。

## Markdown
Markdown 文件打开时预览显示在旁边（⇧⌘V），支持公式、Mermaid、Graphviz 和 PlantUML 图表以及文件夹中的图片。“格式”菜单提供粗体、斜体、链接、标题、列表、引用和代码块；从 Word 或网页粘贴的文本会转为 Markdown。**文件 › 导出**可写出 PDF、单个 HTML 文件或 Word 文稿。如果安装了 Splat，**文件 › 在 Splat 中打开**会在其中打开文稿。

## 浏览器
浏览器标签页，带有地址栏、后退和前进、搜索、先询问再下载，以及网页检查器。

## Git
Git 面板支持暂存和丢弃整个文件或单个变更块、提交、提交并推送或同步、分支、储藏、合并冲突，以及近期提交的图谱。Blame 显示谁最后更改了光标所在行，也可以为整个文件显示一列。一个按钮可用 AI 撰写提交信息；只有在 gitleaks 未在差异中发现机密信息后，差异才会发出，而且每次提交同样会检查机密信息。AI 对话可以在独立的 Git 工作树中工作；面板会显示它的更改，并通过合并或 cherry-pick 并入其工作。

## CI
GitHub Actions 和 GitLab CI，也支持 GitHub Enterprise Server 和自托管 GitLab：当前分支的运行、作业和日志；重新运行失败的作业、单个作业或（在 GitHub 上）整个运行，启动手动作业或批准等待中的部署，启动新的流水线或可手动触发的工作流，以及取消。打开的合并请求或拉取请求以及议题列在同一面板中，并在浏览器标签页中打开。失败作业的日志可以发送给智能体，并标记为外部文本。Token 来自钥匙串、`gh` 或 `glab`、登录 Shell 的环境，或由你输入。

## AI
每个智能体都在 AI 标签页中工作，时间线相同：回答、工具调用及其输出、权限请求、提问、待批准和评论的计划、待办事项、子智能体和后台任务；标签页标题栏中的两个按钮列出正在运行的子智能体和后台进程。每次发送提示词之前，App 都会保存一个检查点；更改可以按消息查看和回退。新对话以“手动”模式开始，具有破坏性的 Git 命令永远不会被永久允许。“活动”概览列出所有正在运行的内容。标签页上的绿点表示会话正在工作，红点表示会话在等你；你在看别处时，会话完成或等待都会有通知提醒你。
- **Claude** 通过原样的 `claude` 命令，支持会话、子智能体、后台任务、上下文和用量。
- **ACP 智能体：**Codex、Antigravity 和 OpenCode，以及在你要求时来自 ACP 注册表的更多智能体；已安装的智能体固定为经过测试的版本，并根据校验和进行验证。
- **内置智能体：**Anthropic、OpenAI、Google Gemini、OpenRouter、Ollama、LM Studio，以及兼容 OpenAI 或 Anthropic API 的服务器，带有用于文件、搜索、命令、诊断和网络请求的工具；密钥保存在钥匙串中。
- 每个对话都有一个沙盒开关，在智能体支持的范围内限制其命令可以执行的操作。
- 外部文本（例如终端输出、网页或 CI 日志）会标记为不可信后再交给智能体。

## 听写
通过 Lowspeak 的语音识别在 AI 消息栏和编辑器中听写：使用麦克风按钮、⌃⌥D，或在 Dango 为当前 App 时按 fn 键。听写使用 Lowspeak 的设置和模型；在线转写需要 OpenAI 或 Groq 的密钥。

## 更新
App 通过 Sparkle 自动更新。第二次启动时，它会询问是否可以自行检查更新；在你同意之前，它不会发出任何请求。**Dango › 检查更新…**会立即检查。

## 辅助功能
边栏、标签页、AI 时间线中的卡片、Git 和 CI 行、调试器以及终端文本都有 VoiceOver 名称；可完全用键盘操作，快捷键可在 `keybindings.json` 中更改；支持“增强对比度”和“减弱动态效果”；在从右到左的界面中，代码视图仍保持从左到右。详情见 ACCESSIBILITY.md。

## 隐私
没有遥测、没有分析，也没有崩溃报告。App 只为你启动的操作建立连接：你使用的智能体、你要求的下载、你登录的 CI 主机、你选择的 Copilot 预测、你选择的在线转写、你打开的网页，以及你允许后的更新检查。密钥和 Token 保存在 macOS 钥匙串中。

## 已知限制
- 通过浏览器登录 CI 的功能已关闭；请使用 `gh`、`glab`、Token 或环境变量。
- App 不在 App Sandbox 中运行，因为终端、Git、语言服务器和智能体都会继承它；文件夹必须先获得你的信任，其配置的任何内容才会运行。Antigravity 和 OpenCode 没有可由 App 开启的沙盒。
- Claude 的用量数据和上下文明细依赖其协议中标记为实验性的部分，缺少这些部分时会隐藏。回退对话内容仅适用于 Claude；对于其他智能体，只回退代码。
- 不提供 Gemini CLI：Google 已停止使用个人账户登录，并引导用户使用 Antigravity。以前的 Gemini 对话仍可阅读。
