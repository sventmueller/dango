<p><img src="../../readme-icon.png" alt="Dango 應用程式圖像" width="147" height="147"></p>

# Dango

**與程式設計代理程式協作的原生 macOS 編輯器。**

Dango 把程式碼、修改程式碼的代理程式，以及圍繞它們的一切，依檔案夾放進一個視窗：具備語言伺服器和編輯預測的編輯器、終端機、瀏覽器、Git、CI、除錯器，以及與 Claude、Codex、Antigravity、OpenCode 或 App 內建代理程式進行的 AI 對話。每個代理程式都在同一條時間軸中工作，在變更檔案或執行指令之前會先詢問你，並在每次送出提示詞之前留下一個檢查點，因此你可以逐則訊息檢視並回溯它所做的一切。在你信任檔案夾之前，它所設定的任何項目都不會執行；來自外部的文字在交給代理程式時會標示為不可信任。你可以在編輯器和訊息欄位中聽寫，語音辨識可以完全留在你的 Mac 上。Dango 以 Rust 寫成，建立在真正的 AppKit 控制項之上，不使用 Electron，支援 35 種語言，沒有遙測功能。

[![下載 Dango.dmg](https://img.shields.io/badge/%E4%B8%8B%E8%BC%89_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[下載 Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: 已簽署並經 Apple 公證的磁碟映像檔，適用於搭載 Apple 晶片或 Intel 處理器的 macOS 14 或以上版本。連結將在第一個版本發布後可用。

**發現錯誤了嗎？** 請[提交 Issue](https://github.com/sventmueller/dango/issues/new)，並說明問題是在哪個代理程式和哪類專案中發生的。

[![Rust 2024 版](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org) [![macOS 14 或以上版本](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/) ![通用：Apple 晶片和 Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white) [![透過 objc2 使用 AppKit](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2) [![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com) [![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/) [![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/) [![終端機：Tapa term-vt](https://img.shields.io/badge/terminal-Tapa_term--vt-F46D01)](https://github.com/sventmueller/tapa) ![35 種介面語言](https://img.shields.io/badge/languages-35-4C9A2A) ![沒有遙測](https://img.shields.io/badge/telemetry-none-4C9A2A) ![以 Hardened Runtime 簽署](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A) [![授權條款：MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#授權條款)

[Bahasa Indonesia](../id/README.md) · [Bahasa Melayu](../ms/README.md) · [Català](../ca/README.md) · [Čeština](../cs/README.md) · [Dansk](../da/README.md) · [Deutsch](../de/README.md) · [English](../../../README.md) · [Español](../es/README.md) · [Français](../fr/README.md) · [Hrvatski](../hr/README.md) · [Italiano](../it/README.md) · [Magyar](../hu/README.md) · [Nederlands](../nl/README.md) · [Norsk](../no/README.md) · [Polski](../pl/README.md) · [Português (Brasil)](../pt_BR/README.md) · [Português (Portugal)](../pt_PT/README.md) · [Română](../ro/README.md) · [Slovenčina](../sk/README.md) · [Slovenščina](../sl/README.md) · [Suomi](../fi/README.md) · [Svenska](../sv/README.md) · [Tiếng Việt](../vi/README.md) · [Türkçe](../tr/README.md) · [Ελληνικά](../el/README.md) · [Русский](../ru/README.md) · [Українська](../uk/README.md) · [עברית](../he/README.md)‎ · [العربية](../ar/README.md)‎ · [हिन्दी](../hi/README.md) · [ไทย](../th/README.md) · [한국어](../ko/README.md) · [日本語](../ja/README.md) · [简体中文](../zh-Hans/README.md) · **繁體中文**

## 目錄

- [功能一覽](#功能一覽)
- [系統需求](#系統需求)
- [安裝](#安裝)
- [快速上手](#快速上手)
- [AI 代理程式](#ai-代理程式)
  - [時間軸](#時間軸)
  - [權限與模式](#權限與模式)
  - [檢查點與工作樹](#檢查點與工作樹)
- [編輯器](#編輯器)
- [語言伺服器](#語言伺服器)
- [測試](#測試)
- [除錯器](#除錯器)
- [終端機](#終端機)
- [瀏覽器](#瀏覽器)
- [Git](#git)
- [CI](#ci)
- [容器](#容器)
- [Markdown](#markdown)
- [聽寫](#聽寫)
- [信任](#信任)
- [沙盒](#沙盒)
- [外觀與輔助使用](#外觀與輔助使用)
- [語言](#語言)
- [鍵盤快速鍵](#鍵盤快速鍵)
- [隱私權與安全性](#隱私權與安全性)
- [資料存放位置](#資料存放位置)
- [已知限制](#已知限制)
- [從原始碼建置](#從原始碼建置)
- [授權條款](#授權條款)

## 功能一覽

- **所有代理程式，同一條時間軸。** Claude 透過它自己的 `claude` 指令執行，Codex、Antigravity 和 OpenCode 透過 Agent Client Protocol 執行，更多代理程式可來自 ACP 登錄庫；此外還有一個 App 內建的代理程式，支援 Anthropic、OpenAI、Google Gemini、OpenRouter、Ollama 和 LM Studio。無論執行的是哪個代理程式，回答、工具呼叫、權限請求、計畫、待辦事項、子代理程式和背景任務看起來都一樣。
- **檢視與回溯。** 每次送出提示詞之前，Dango 都會保存一個檢查點。代理程式所做的變更可以依訊息檢視和回溯；對話也可以在獨立的 Git 工作樹中工作，並透過合併或 cherry-pick 交出成果。
- **每個檔案夾一個視窗。** 檔案、終端機、瀏覽器頁面、AI 對話、差異和預覽都是該視窗的 macOS 標籤頁。側邊欄包含「檔案」「搜尋」「代理程式」「Git」「測試」「除錯」「CI」和「容器」。
- **真正的程式碼編輯器。** 語法上色、多個插入點、摺疊、`.editorconfig`、涵蓋 17 種語言的語言伺服器（支援自動完成、前往定義、參照、重新命名、快速修正和階層），以及來自 GitHub Copilot 或本機模型的編輯預測。
- **內建除錯器、Git 和 CI。** 適用於 Rust、C、C++、Swift、Go、JavaScript、TypeScript 和 Python 的中斷點與逐步執行；暫存單一變更區塊、分支、儲藏、衝突和提交圖表；來自 GitHub Actions 和 GitLab CI 的執行、作業和記錄。
- **謹慎對待你的檔案夾。** 未受信任的檔案夾不會啟動任何東西：沒有 Git 程序、語言伺服器、掛鉤或代理程式。具破壞性的 Git 指令永遠不會被永久允許，提交也會檢查是否含有機密資訊。
- **聽寫。** 對著編輯器和訊息欄位說話，可以透過 OpenAI 或 Groq 線上辨識，也可以用 Whisper 和 Parakeet 在本機辨識。
- **原生到骨子裡。** AppKit 視窗、標籤頁、選單和工具列，編輯器使用 TextKit 2，終端機使用 Metal，WebKit 只用於瀏覽器標籤頁。深色模式、「增加對比」和 VoiceOver 的運作方式與其他 Mac App 相同。
- **使用你的語言。** 介面提供 35 種語言：阿拉伯文、加泰隆尼亞文、簡體中文、繁體中文、克羅埃西亞文、捷克文、丹麥文、荷蘭文、英文、芬蘭文、法文、德文、希臘文、希伯來文、印地文、匈牙利文、印尼文、義大利文、日文、韓文、馬來文、挪威文、波蘭文、葡萄牙文（巴西）、葡萄牙文（葡萄牙）、羅馬尼亞文、俄文、斯洛伐克文、斯洛維尼亞文、西班牙文、瑞典文、泰文、土耳其文、烏克蘭文和越南文。

## 系統需求

- macOS 14 Sonoma 或以上版本，搭載 Apple 晶片或 Intel 處理器。Dango 在 macOS 27 上開發和測試。
- AI 對話需要你想使用的代理程式；請參閱 [AI 代理程式](#ai-代理程式)。
- 語言伺服器和除錯轉接器會在你要求時由 Dango 安裝，它們需要：gopls 和 `dlv` 需要 Go；TypeScript、Python、JSON、YAML、CSS、HTML 和 Shell 的伺服器以及 JavaScript 除錯器需要 Node.js；clangd、sourcekit-lsp 和 `lldb-dap` 需要 Xcode Command Line Tools。rust-analyzer 從其 GitHub 發布頁面下載。
- 編輯預測需要 GitHub Copilot 訂閱方案（免費方案即可），或 Ollama 或 LM Studio 中的中間填充（fill-in-the-middle）模型。

## 安裝

**從磁碟映像檔。** 每個版本都是一個磁碟映像檔 `Dango-<version>.dmg`，內含一個同時適用於 Apple 晶片和 Intel 處理器的通用 App，以 Developer ID 簽署並經 Apple 公證。打開它，然後將 Dango 拖到「應用程式」檔案夾。Homebrew Cask 安裝的也是同一個映像檔。目前尚未發布任何版本。

**更新。** 從磁碟映像檔安裝的 Dango 會透過 [Sparkle](https://sparkle-project.org) 自行保持最新。第二次啟動時，它會詢問是否可以自行檢查更新；在你同意之前，它不會發出任何請求。**Dango › 檢查更新項目⋯** 會立即檢查。更新以 App 自己的金鑰簽署，Sparkle 會在安裝任何項目之前先驗證它。自行建置的版本沒有更新功能。

**從原始碼。** 請參閱[從原始碼建置](#從原始碼建置)，然後將 `target/release/Dango.app` 移到 `/Applications`。

**終端機指令。** **Dango › 安裝「dango」指令⋯** 會將一個小型腳本寫入 `~/.local/bin/dango`，不需要管理者權限；`~/.local/bin` 必須在你的 `PATH` 中。之後，`dango .` 會打開目前的檔案夾，`dango file.rs` 會打開一個檔案。Dango 絕不會取代該位置中不是它自己寫入的檔案。

## 快速上手

1. 用 ⌘O 或 `dango <folder>` **打開檔案夾**。第一次打開時，Dango 會詢問你是否信任它（請參閱[信任](#信任)）。
2. **側邊欄**可在「檔案」（⌘1）、「搜尋」（⌘2）、「代理程式」（⌘3）、「Git」（⌘4）、「測試」（⌘5）、「除錯」（⌘6）、「CI」（⌘7）和「容器」（⌘8）之間切換；按一下右鍵可隱藏不使用的項目，⌃⌘S 可收合側邊欄。
3. **每份文件都是檔案夾視窗中的一個標籤頁**，也就是 macOS 自己的標籤頁。標籤列末端的「+」和 ⌘T 可開啟新檔案、終端機、瀏覽器以及每個已安裝的代理程式，也可放在獨立的 Git 工作樹中；按住「+」會以選單形式顯示相同的選項。
4. **快速打開**（⌘P）可尋找檔案，輸入 `#` 可尋找專案中的符號；⇧⌘F 在專案中搜尋和取代。
5. 在 AI 標籤頁中 **與代理程式對話**。⌘Esc 會將鍵盤焦點移到它的訊息欄位，再按一次則移回。
6. 以 ⌘, 打開的 **設定** 包含以下標籤頁：「一般」「CI」「代理程式」「內建代理程式」「語言伺服器」「聽寫」和「預測」。

工具列包含在編輯過的位置之間後退和前進（⌃- 和 ⌃⇧-）、分支、「快速打開」以及列出所有執行中項目的「活動」概覽。在檔案的快速選單中選擇 **在旁邊打開**，會在第一份文件旁顯示第二份文件。重新啟動或當機之後，每個視窗都會連同其標籤頁恢復，含有未儲存文字的編輯器也會連同這些文字恢復。影像、音訊、影片、字體和 Office 文件會透過快速查看在標籤頁中打開，PDF 則透過 PDFKit。

## AI 代理程式

| 代理程式 | 執行方式 | 需要什麼 |
|---|---|---|
| Claude | `claude` 指令，透過其 stream-json 協定直接溝通 | 已安裝並登入 `claude`；Dango 會在登入 Shell 的 `PATH`、`~/.local/bin`、`~/.claude/local`、`/opt/homebrew/bin` 和 `/usr/local/bin` 中尋找。登入只在 `claude` 本身中進行 |
| Codex | 透過 Agent Client Protocol（ACP）執行的 `codex-acp` | Node.js 和 npm；Dango 會在你要求時安裝固定版本的套件 |
| Antigravity | `agy acp` | `PATH` 中你自己的 `agy`，或在你要求時安裝的固定版本下載項目 |
| OpenCode | `opencode acp` | `PATH` 中你自己的 `opencode`，或在你要求時安裝的固定版本下載項目 |
| 其他 ACP 代理程式 | 來自 ACP 登錄庫，只在你要求時擷取 | 登錄庫項目中列出的內容；沒有核對碼的項目只有在你確認後才會安裝 |
| 內建代理程式 | 內建於 App，具備用於檔案、搜尋、指令、診斷和網路請求的工具 | Anthropic、OpenAI、Google Gemini 或 OpenRouter 的 API 金鑰，保存在鑰匙圈中；這部 Mac 上的 Ollama 或 LM Studio，不需要金鑰；任何相容 OpenAI 或 Anthropic API 的伺服器 |

**設定 › 代理程式** 會顯示這部 Mac 上有哪些代理程式，安裝其他代理程式及其更新，並打開 ACP 登錄庫。Dango 安裝的代理程式固定在經過測試的版本，並以其公布的核對碼（npm integrity 或 SHA-256）驗證；Dango 不執行任何未指定版本的 `npx` 套件。

### 時間軸

每個代理程式都在 AI 標籤頁中工作，時間軸都相同：回答、工具呼叫及其輸出、權限請求、提問、可核准和評論的計畫（⌥⌘K）、待辦事項、子代理程式和背景任務。標籤頁標題列中的兩個按鈕會列出執行中的子代理程式和背景程序。提及一個檔案、`@terminal` 或 `@browser`，即可將其內容交給代理程式。標籤頁上的綠點表示工作階段正在工作，紅點表示工作階段在等你；你在看別處時，工作階段完成或等待都會有通知提醒你。對於 Claude，狀態列會顯示上下文和用量。

### 權限與模式

新對話以「手動」模式開始：代理程式在變更檔案或執行指令之前會先詢問。對於具破壞性的 Git 指令（`push --force`、`reset --hard`、`clean -fd`），Dango 不提供「永遠允許」。內建代理程式會依專案保存其「永遠允許」規則，在 **設定 › 內建代理程式** 中列出，而且無論保存了什麼，每次遇到具破壞性的 Git 指令都會詢問；Claude 和 ACP 代理程式會將這類規則保存在它們自己的設定中。選擇「全部允許」且沒有沙盒時，對話會持續顯示一則警告。

### 檢查點與工作樹

每次送出提示詞之前，Dango 都會保存一個檢查點：在專案儲存庫中以 `refs/ide-checkpoints/` 下的提交保存，沒有 Git 的專案則保存在它自己的檔案夾中；一般的推送或擷取永遠不會帶上它們。代理程式所做的變更可以依訊息檢視和回溯。對話可以在獨立的 Git 工作樹中工作：Git 面板會顯示它變更的內容，並透過合併或 cherry-pick 整合其工作。

## 編輯器

- 相對於上次提交的 **語法上色、行號和變更標記**，尋找與取代，快速打開（⌘P）和專案搜尋（⇧⌘F）。
- **縮排** 依循檔案和 `.editorconfig`：Tab 或空格、自動縮排、括號配對。
- **多個插入點：** ⌥-按一下、⌥-拖移以選取一欄、⌃⇧↑ 和 ⌃⇧↓、⌘D 選取下一個相符項目。
- **摺疊**（⌥⌘← 和 ⌥⌘→，加上 ⇧ 則作用於全部）、切換註解（⌘/）、前往行（⌃G）、顯示空白字元和自動換行（⌥Z）。
- **檔案會保留其編碼**、位元組順序標記和行尾字元。磁碟上的檔案有變更時，若編輯器中沒有未儲存的文字就會重新載入；否則會出現一列提示，提供差異與合併。
- **編輯預測。** 輸入時會以灰色顯示建議，按 Tab 即可採用。建議來自 GitHub Copilot 或 Ollama、LM Studio 中的本機模型，依你在 **設定 › 預測** 中的選擇而定；本機模型會讓你的程式碼留在這部 Mac 上。
- **縮圖和固定捲動**（「編輯器」選單）：前者在文字旁以窄條顯示整個檔案，後者將最上方那一行所在各程式碼區塊的標題行固定在其上方。
- **在行尾顯示診斷**（「顯示方式」選單）：停頓一秒後，在每行末尾顯示該行最嚴重的訊息，並以符號標示其嚴重程度。
- **靜態頁面即時預覽：** 「編輯器 › 在瀏覽器標籤頁中打開」由 Dango 本身提供受信任檔案夾中的頁面，並在檔案儲存時重新載入標籤頁。
- **Prettier** 會在專案設定了它的情況下，使用專案本身的 `node_modules/.bin/prettier` 格式化 JavaScript、TypeScript、CSS、HTML、Markdown 等；設定項目 `formatter` 可依語言選擇格式化工具。
- **Vim 模式**（「編輯器 › Vim 模式」）：一般模式、插入模式、選取模式和取代模式，計數、搭配移動和文字物件的運算子、`.`、暫存器、標記和巨集。`u` 和 ⌃R 使用編輯器的還原，一次連續插入算作一步。`"+` 和 `"*` 即剪貼板；設定項目 `vimClipboard` 會讓無名暫存器也成為剪貼板。`/`、`?`、`n`、`N`、`*`、`:s` 和 `:%s` 使用 Rust 的正規表示式搜尋，而不是 Vim 的模式：不支援 `\v`，以 `\b` 取代 `\<` 和 `\>`。Ex 指令有 `:w`、`:q`、`:wq`、`:x`、`:noh` 和 `:<line>`。在一般模式下，⌃G 交給 Vim，而不是「前往行」。
- **設定同步**（**設定 › 一般**）：透過 iCloud 雲碟中的一個檔案夾，你的設定和快速鍵會跟著你到其他 Mac 上；路徑、本機位址和各項同意則保留在每部 Mac 上。

## 語言伺服器

Go、Rust、TypeScript 和 JavaScript、Python、C、C++ 和 Objective-C、Swift、Shell 指令碼、JSON、YAML、CSS、HTML、TOML、Markdown 和 Lua，以及你在 **設定 › 語言伺服器** 中加入的伺服器。Dango 會在你要求時將伺服器安裝到它自己的工具檔案夾中；若 TypeScript 專案本身的 `tsc` 可作為語言伺服器，就會使用它。

它們帶來的功能：診斷（⌘' 和 ⇧⌘' 在診斷之間移動）、懸停資訊（⌃⌘I）、自動完成（⌥Esc）、簽章說明、前往定義（⌃⌘J）、類型定義和實作、參照（⌃⇧⌘F）、呼叫階層和類型階層（⌃⇧⌘H）、重新命名（⌃⌘E）、格式化（⌥⇧F）、語意上色、摺疊和大綱（⇧⌘O）；在行內顯示類型和參數名稱；燈泡（⌥⌘A）背後的快速修正和重構；用 ⌥⌘O 尋找專案中的符號。

除了檔案本身的伺服器，ESLint 還會檢查使用它的專案中的 JavaScript 和 TypeScript，並提供其快速修正；Emmet 的縮寫會出現在 HTML、CSS 和 JSX 的自動完成中。Dango 會在你要求時安裝這兩者；如果你願意，**設定 › 一般** 可在儲存時修正 ESLint 的問題。

## 測試

「測試」面板按套件和檔案列出檔案夾中的測試及其狀態，支援 `cargo test` 或 cargo-nextest、Go、pytest、Vitest 和 Jest。行號欄中每個測試旁的符號可執行該測試，⌥-按一下可除錯，失敗的斷言會標示所在行。「除錯」選單可執行插入點的測試（⌃⌥⌘U）、再次執行上次的測試（⌃⌥⌘G）或執行全部測試，也可除錯插入點的測試。測試只在受信任的檔案夾中執行，因為它們會執行專案的程式碼；`cargo test` 只在你要求時列出測試，因為列出測試需要建置每個測試程式。

## 除錯器

中斷點（也支援條件和記錄點）、逐步執行、執行緒及其呼叫堆疊、檢閱器（⌥⌘I）中的變數和監看運算式、程式暫停時顯示在程式碼旁的值，以及在已暫停堆疊框中求值的除錯主控台。

| 語言 | 除錯轉接器 |
|---|---|
| Rust、C、C++、Swift | Xcode Command Line Tools 中的 `lldb-dap` |
| Go | `dlv` |
| JavaScript、TypeScript | 在 Node.js 上執行的 JavaScript 除錯器 |
| Python | debugpy |

Dango 會讀取專案的 `.vscode/launch.json`。「除錯」選單使用 Xcode 的快速鍵：⌘\\ 設定中斷點，⌘Y 開啟或關閉所有中斷點，⌃⌘Y 繼續，F6、F7 和 F8 分別為步過、步入和步出，⌘. 停止。

## 終端機

終端機在標籤頁中執行（⌃\`），使用登入 Shell 的環境，並以 Metal 繪製。⌘K 清除畫面。貼上或拖放影像時，會將它交給在終端機中執行的程式設計代理程式，或貼上已儲存副本的路徑。⌘-按一下可打開連結和檔案名稱。終端機啟動的開發伺服器可以在瀏覽器標籤頁中打開。

## 瀏覽器

瀏覽器標籤頁，具備網址欄位（⌘L）、上一頁和下一頁（⌘[ 和 ⌘]）、重新載入（⌘R）、搜尋、下載前先詢問，以及網頁檢閱器。`@browser` 會將頁面交給代理程式，並標示為外部文字。

## Git

Git 面板可以暫存和丟棄整個檔案或單一變更區塊、提交、提交並推送或同步、切換和建立分支、保存儲藏、解決合併衝突，並繪製近期提交的圖表。Blame 會顯示誰最後修改了游標所在行，也可以為整個檔案顯示一欄。一個按鈕可用 AI 撰寫提交訊息；只有在 gitleaks 未在差異中找到機密資訊後，差異才會送出，而且每次提交也同樣會檢查機密資訊。

## CI

GitHub Actions 和 GitLab CI，也支援 GitHub Enterprise Server 和自行架設的 GitLab：

- 目前分支的執行、作業和記錄；
- 重新執行失敗的作業、單一作業或（在 GitHub 上）整個執行；啟動手動作業或核准等待中的部署；啟動新的管線或可手動觸發的工作流程；取消；
- 開啟中的拉取請求或合併請求以及議題，它們會在瀏覽器標籤頁中打開；
- 將失敗作業的記錄交給代理程式，並標示為外部文字。

Token 來自鑰匙圈、`gh` 或 `glab`、登入 Shell 的環境，或你在 **設定 › CI** 中輸入的 Token。

## 容器

「容器」面板顯示 Docker 的 Compose 專案及其服務、單獨的容器、映像檔和磁碟區，並即時跟隨 Docker 的事件。檔案夾中的 Compose 檔案即使還沒有任何容器，也會顯示為專案。容器可以啟動、停止和重新啟動；它們的記錄和 Shell 會在終端機標籤頁中打開，已發佈的連接埠則在瀏覽器標籤頁中打開。刪除容器、映像檔或磁碟區以及清理未使用的資料之前都會先詢問。Compose 的 up、down 和 pull 只在受信任的檔案夾中執行，因為 Compose 會建置並執行專案所描述的內容。面板的選單為視窗選擇 Docker 上下文，而不改變你的 Shell 所用的上下文；相同的指令也在「容器」選單中。需要 `docker` 命令列和 Docker 24.0.6 或以上版本。

## Markdown

- **預覽。** Markdown 檔案打開時，預覽會顯示在旁邊；**顯示方式 › 顯示/隱藏旁邊的預覽**（⇧⌘V）可關閉和打開預覽，**設定 › 一般** 可關閉自動預覽。預覽會繪製公式、Mermaid、Graphviz 和 PlantUML 圖表以及檔案夾中的圖片；來自網路的圖片只會在受信任的檔案夾中，或在你於預覽中要求後才會載入。
- **「格式」選單：** 粗體（⌘B）、斜體（⌘I）、刪除線（⇧⌘X）、程式碼（⌥⇧⌘C）、連結（⇧⌘K）、標題 1–3（⌥⌘1–⌥⌘3）、項目符號列表和編號列表、任務、引文和程式碼區塊。每個指令都作用於所選內容或所在行，可來回切換，而且只需一步即可還原。
- **貼上。** 來自 Word 或網頁的文字會以 Markdown 形式貼上，其中的圖片會作為檔案保存在文件旁邊；⌥⇧⌘V 貼上純文字。
- **輸出。** **檔案 › 輸出** 可寫出 PDF、內含圖片的單一 HTML 檔案或 Word 文件。只有從受信任的檔案夾輸出時，才會帶上來自網路的圖片。
- **在 Splat 中打開。** 若已安裝 [Splat](https://github.com/sventmueller/splat)，**檔案 › 在 Splat 中打開** 以及檔案樹和標籤頁的快速選單會先儲存文件，再在 Splat 中打開它。

## 聽寫

在 AI 標籤頁的訊息欄位或編輯器中聽寫：使用那裡的麥克風按鈕、**編輯 › 開始聽寫**（⌃⌥D）或 fn 鍵（按住即錄音，點兩下則持續錄音）。語音辨識來自 [Lowspeak](https://github.com/sventmueller/lowspeak)；不需要安裝 Lowspeak 本身。

- **設定 › 聽寫** 用來設定模式和模型（透過 OpenAI 或 Groq 線上辨識，透過 Whisper 或 Parakeet 在本機辨識）、語言、後續處理、麥克風以及你自己的詞彙。
- **本機模型** 會在你選擇時從 Hugging Face 下載，顯示進度並可取消。模型會存放在 Lowspeak 使用的檔案夾中，因此兩個 App 都能使用對方下載的模型。本機模型會讓錄音留在這部 Mac 上。
- **線上轉錄** 需要 OpenAI 或 Groq 的金鑰，Dango 會將它保存在自己的鑰匙圈項目中。
- **從 Lowspeak 移轉。** 若 Lowspeak 的設定與預設值不同，「聽寫」標籤頁會提議一次性接手其設定，並在你要求時接手其金鑰；Dango 讀取金鑰之前，macOS 會先詢問你（請選擇「允許」，而非「永遠允許」）。
- **權限。** 第一次聽寫時，macOS 會要求麥克風權限。fn 鍵在 Dango 為目前使用中的 App 且 Lowspeak 未執行時有效；它需要「輔助使用」權限，Dango 會在第一次聽寫之後要求該權限，絕不會在啟動時要求。

## 信任

檔案夾可以讓程式在其中有東西啟動時立即執行：Git 掛鉤和 `core.fsmonitor` 等儲存庫設定、來自 `node_modules` 的語言伺服器、代理程式掛鉤、`.mcp.json`。第一次打開檔案夾時，Dango 會詢問你是否信任它。

- **受信任：** Git、語言伺服器、除錯器、代理程式和 CI 會為此檔案夾及其下的所有內容執行。
- **受限制：** 你可以閱讀和編輯；編輯器中的變更標記來自直接讀取 `.git`。檔案夾所設定的任何項目都不會啟動。Git、CI 和 AI 面板會說明這一點，並提供信任該檔案夾的選項。

不是來自你的文字可能含有意圖操控代理程式的指示（提示詞注入）。因此，終端機的輸出（`@terminal`）、瀏覽器頁面（`@browser`）、交給代理程式的 CI 記錄、專案外的檔案，以及從網頁貼上的較長文字，在傳遞時都會帶有標示：標籤上顯示「外部內容」，代理程式收到文字時會附帶一則說明，指出這是不可信任的資料，不得遵循其中的指示。內建代理程式取得的網頁也帶有同樣的說明。當瀏覽器在剪貼板中註明來源時（Safari、Chrome 和 Firefox 都會這麼做），貼上的文字會被視為來自網頁；來自其他地方的文字，以及只有幾個字的短語，仍算是你自己的文字。

## 沙盒

Dango 本身不在 App Sandbox 中執行，因為它啟動的每個程式都會繼承沙盒：終端機、Git、語言伺服器和代理程式將無法運作。作為替代，每個 AI 對話都有一個沙盒開關，預設為關閉。它限制什麼取決於代理程式，開關的工具提示會說明這一點：

| 代理程式 | 開啟沙盒時 |
|---|---|
| Claude | Shell 指令在 Claude 自己的沙盒中執行；它的檔案和網路工具則不受限制 |
| Codex | 指令和變更只能寫入專案內，且無法存取網路 |
| 內建代理程式 | 檔案、網路請求和指令都限制在專案和允許的檔案夾內，且無法存取網路（`sandbox-exec`） |
| Antigravity、OpenCode | 沒有 Dango 可以開啟的沙盒 |

## 外觀與輔助使用

- **深色模式** 跟隨系統；**增加對比** 和 **減少動態效果** 也會得到遵循。
- **VoiceOver** 會為側邊欄、標籤頁、AI 時間軸中的卡片、Git 和 CI 列、除錯器以及終端機的文字提供名稱。中斷點絕不會只靠顏色來區分。
- **鍵盤。** 每個指令都有快速鍵，所有快速鍵都可以在 `keybindings.json` 中更改；⌥⌘\` 移到下一個面板。
- **由右至左。** 在阿拉伯文和希伯來文中，視窗會左右鏡像，而程式碼檢視仍維持由左至右。

Dango 在 AppKit 之上依領域補充的內容，請參閱 [ACCESSIBILITY.md](../../../ACCESSIBILITY.md)。

## 語言

介面已翻譯成 35 種語言：阿拉伯文、加泰隆尼亞文、簡體中文、繁體中文、克羅埃西亞文、捷克文、丹麥文、荷蘭文、英文、芬蘭文、法文、德文、希臘文、希伯來文、印地文、匈牙利文、印尼文、義大利文、日文、韓文、馬來文、挪威文、波蘭文、葡萄牙文（巴西）、葡萄牙文（葡萄牙）、羅馬尼亞文、俄文、斯洛伐克文、斯洛維尼亞文、西班牙文、瑞典文、泰文、土耳其文、烏克蘭文和越南文。

Dango 會依照 **系統設定 › 一般 › 語言與地區** 中的語言順序；你也可以在那裡的 **應用程式** 下，單獨為 Dango 選擇語言。如果沒有你的語言的翻譯，就會使用英文。用語依循同一份詞彙表 [docs/glossary.md](../../../docs/glossary.md) 和 Apple 自己的翻譯，所以 Dango 讀起來就像 Mac 的其他部分。

## 鍵盤快速鍵

| 動作 | 快速鍵 |
|---|---|
| 打開檔案夾、新增視窗、關閉視窗 | ⌘O, ⇧⌘N, ⇧⌘W |
| 新增檔案、儲存、儲存為 | ⌘N, ⌘S, ⇧⌘S |
| 新增標籤頁、關閉標籤頁、下一個和上一個標籤頁 | ⌘T, ⌘W, ⇧⌘] 和 ⇧⌘[ |
| 快速打開、在專案中搜尋 | ⌘P, ⇧⌘F |
| 檔案、搜尋、代理程式、Git、測試、除錯、CI、容器 | ⌘1 至 ⌘8 |
| 顯示或隱藏側邊欄、下一個面板 | ⌃⌘S, ⌥⌘\` |
| 在編輯過的位置之間後退和前進 | ⌃- 和 ⌃⇧- |
| 切換註解、前往行 | ⌘/, ⌃G |
| 下一個相符項目、在上方和下方加入插入點 | ⌘D, ⌃⇧↑ 和 ⌃⇧↓ |
| 摺疊、展開、全部摺疊、全部展開 | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| 自動換行、檢閱器 | ⌥Z, ⌥⌘I |
| 自動完成、懸停資訊、前往定義 | ⌥Esc, ⌃⌘I, ⌃⌘J |
| 參照、呼叫階層、重新命名 | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| 檔案中的符號、專案中的符號 | ⇧⌘O, ⌥⌘O |
| 快速修正、格式化 | ⌥⌘A, ⌥⇧F |
| 下一個和上一個問題 | ⌘' 和 ⇧⌘' |
| Markdown 預覽、貼上為純文字 | ⇧⌘V, ⌥⇧⌘V |
| 粗體、斜體、刪除線、程式碼、連結 | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| 標題 1 至 3 | ⌥⌘1 至 ⌥⌘3 |
| 新增終端機、清除終端機 | ⌃\`, ⌘K |
| 聚焦代理程式的訊息欄位、評論計畫 | ⌘Esc, ⌥⌘K |
| 網址欄位、重新載入、上一頁、下一頁（瀏覽器） | ⌘L, ⌘R, ⌘[, ⌘] |
| 開始或停止聽寫 | ⌃⌥D |
| 中斷點、開啟或關閉所有中斷點 | ⌘\\, ⌘Y |
| 執行插入點的測試、再次執行上次的測試 | ⌃⌥⌘U, ⌃⌥⌘G |
| 繼續、步過、步入、步出、停止 | ⌃⌘Y, F6, F7, F8, ⌘. |
| 設定 | ⌘, |

App 資料檔案夾中的 `keybindings.json`（請參閱[資料存放位置](#資料存放位置)）可以更改其中任何一個快速鍵。

## 隱私權與安全性

- **沒有遙測，** 沒有分析，也不會向任何地方傳送當機報告。錯誤會寫入系統記錄（「主控台」App 中，位於 App 名稱下）。
- **只為你啟動的項目使用網路：** 你使用的代理程式，各自與自己的服務溝通；你要求時的 ACP 登錄庫，以及代理程式、語言伺服器、除錯轉接器和聽寫模型的下載；你登入的 CI 主機；你選擇後來自 Copilot 的編輯預測；內建代理程式的供應商；你選擇後的線上轉錄；你在瀏覽器標籤頁中打開的網頁；以及你允許後的更新檢查。
- **程式碼離開 Mac**，只會發生在你為此選擇了服務的地方：代理程式、Copilot 的預測、由 AI 撰寫的提交訊息。用於撰寫提交訊息的差異，只有在 gitleaks 未在其中找到機密資訊後才會送出。
- **金鑰和 Token** 保存在 macOS 鑰匙圈中，絕不會寫入設定檔。其他 App 的金鑰（例如 Lowspeak 的金鑰）只有在你同意時才會讀取，而且只讀取一次。
- **固定版本的下載項目。** Dango 安裝的代理程式、語言伺服器和除錯轉接器都固定在經過測試的版本，並以其核對碼驗證。
- **程式碼簽署** 使用 Hardened Runtime；更新只有帶有 App 自己的簽名才會安裝。

## 資料存放位置

`<id>` 是套件識別碼，正式版為 `local.agentide`（除錯版為 `local.agentide.debug`）。它不包含顯示名稱，因此重新命名後所有資料都會保留。

| 位置 | 內容 |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`、`state.json`、`keybindings.json`、`trust.json`；`threads/`（AI 面板的列表）、`agent-threads/`（內建代理程式的對話）、`layouts/`（每個專案的標籤頁）、`recovery/`（當機後未儲存的編輯器）、`checkpoints/`（用於沒有 Git 的檔案夾）、`worktrees/`、`agents/` 和 `tools/`（已安裝的代理程式、語言伺服器和除錯轉接器）、`task-output/` |
| `~/Library/Caches/<id>/` | 貼上的影像、Shell 整合腳本、ACP 登錄庫；可以刪除 |
| `~/Library/Logs/<id>/dictation/` | 聽寫的記錄檔 |
| `~/Library/Application Support/Lowspeak` 和 `FluidAudio` | 本機聽寫模型，與 Lowspeak 共用 |
| `~/Library/WebKit/<id>/` | 瀏覽器標籤頁的 Cookie 和網站資料 |
| `<project>/.dango/settings.json` | 專案自己的設定 |
| 專案的 Git 儲存庫 | 檢查點，以 `refs/ide-checkpoints/` 下的提交形式保存；一般的推送或擷取永遠不會帶上它們 |
| 鑰匙圈，服務 `<id>` | 內建代理程式和聽寫的 API 金鑰、CI Token |
| `~/.local/bin/dango` | 終端機指令（如果你安裝了它） |

Claude 會將自己的工作階段保存在 `~/.claude/projects` 下；Dango 會讀取它們以繼續工作階段，但不會變更它們。

## 已知限制

- **App 本身沒有 App Sandbox**，因為終端機、Git、語言伺服器和代理程式都會繼承它；檔案夾必須先受到信任，其設定的任何項目才會執行。Antigravity 和 OpenCode 沒有 Dango 可以開啟的沙盒。
- **Claude 的用量數據和上下文明細** 依賴其協定中標示為實驗性的部分，缺少這些部分時會隱藏。
- **回溯對話內容** 僅適用於 Claude；對於其他代理程式，只會回溯程式碼。
- **透過瀏覽器登入 CI** 的功能已關閉；請使用 `gh`、`glab`、Token 或環境變數。
- **不提供 Gemini CLI：** Google 已停止以個人帳號登入，並引導使用者改用 Antigravity。先前的 Gemini 對話仍可閱讀。

## 從原始碼建置

你需要：

- macOS 14 或以上版本，並安裝 Xcode Command Line Tools（`xcode-select --install`）；不需要 Xcode 本身。
- `rust-toolchain.toml` 所指定的 Rust。Homebrew 的 `rust` 只為這部 Mac 的處理器建置；正式版的通用建置需要加入目標 `aarch64-apple-darwin` 和 `x86_64-apple-darwin` 的 rustup。
- [splat](https://github.com/sventmueller/splat) 和 [lowspeak](https://github.com/sventmueller/lowspeak) 的檢出副本，放在本儲存庫旁邊（`../splat`、`../lowspeak`），兩者都位於各自的 `main` 分支；工作區依路徑從它們引入 crate。
- cmake，用於每個包含聽寫的建置（`make run`、`make build`、`make release`），以建置 whisper.cpp。`make check` 不需要它。

| 指令 | 作用 |
|---|---|
| `make run` | 建置除錯版 App（`target/debug/Dango Debug.app`）並打開它 |
| `make build` | 建置正式版 App（`target/release/Dango.app`），以首次建置時在登入鑰匙圈中建立的本機憑證簽署；保持 `Cargo.toml` 不變 |
| `make lint` | 格式化和 clippy；有警告即失敗 |
| `make check` | 先 `lint`，然後執行所有測試；不需要鑰匙圈、視窗或網路 |
| `make test` | 先 `check`，然後在真正的 App 中執行自我測試 `probe` 和輔助使用稽核，分別以由左至右和由右至左進行 |
| `make perf` | 在正式版建置中檢查時間預算；只有在 Mac 沒有其他負載時才有意義 |
| `make release` | 將正式版演練到 `target/release-dry-run/`，以本機憑證簽署；不向任何地方傳送任何東西 |
| `make release-notarized` | 先 `check`，然後將正式版本身發布到 `dist/`：Developer ID 簽名、公證、以 App 自己的金鑰簽署的更新來源；保持 `Cargo.toml` 不變 |
| `make deploy` | 在乾淨的 `main` 上，無論有無 CI 印章：公證下一個版本，提交 `Cargo.toml`，上傳發布檔案，推送程式碼和版本標籤（若有綠色 CI 印章則一併推送），然後自行發布該版本 |

除錯版和正式版是兩個獨立的 App，各有自己的設定和資料。建置、測試、自我測試、發布以及程式碼依循的規則見 [docs/development.md](../../../docs/development.md)，其背後的決策見 [docs/plan.md](../../../docs/plan.md)。版本說明是 [RELEASE_NOTES.md](../../../RELEASE_NOTES.md)，其譯文與每份 README 一起放在 `assets/readme/<language>/` 中。

## 授權條款

Dango 採用以下任一授權條款：

- Apache License, Version 2.0（[LICENSE-APACHE](../../../LICENSE-APACHE)）
- MIT License（[LICENSE-MIT](../../../LICENSE-MIT)）

由你選擇。

除非你另有明確聲明，否則依 Apache-2.0 授權條款所定義，你有意提交以納入 Dango 的任何貢獻，都將採用如上的雙重授權，不附加任何其他條款或條件。
