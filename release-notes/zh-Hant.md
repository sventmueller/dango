<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
這是 Dango 的第一個版本。Dango 是一款原生 macOS 編輯器，專為與程式設計代理程式協作而打造。編輯器、終端機、瀏覽器、Git、CI、除錯器和 AI 對話共用每個檔案夾的同一個視窗。App 以 Rust 在 AppKit 上撰寫；WebKit 只用於瀏覽器標籤頁。介面提供 35 種語言。

## 系統需求
- macOS 14 或以上版本，搭載 Apple 晶片或 Intel 處理器。
- AI 對話需要你選擇的代理程式：Claude 需要已登入的 `claude` 指令；Codex 需要 Node.js，App 會在你要求時安裝其固定版本的套件；Antigravity 和 OpenCode 使用你自己的 `agy` 或 `opencode`，或固定版本的下載項目；內建代理程式需要 API 金鑰，或這部 Mac 上的 Ollama 或 LM Studio。
- 語言伺服器和除錯轉接器會在你要求時安裝。gopls 和 `dlv` 需要 Go；TypeScript、Python、JSON、YAML、CSS、HTML 和 Shell 的伺服器以及 JavaScript 除錯器需要 Node.js；clangd、sourcekit-lsp 和 `lldb-dap` 隨 Xcode Command Line Tools 提供。
- 編輯預測需要 GitHub Copilot 訂閱（免費方案即可），或 Ollama 或 LM Studio 中的中間填充（fill-in-the-middle）模型。

## 視窗
- 每個檔案、終端機、瀏覽器頁面、AI 對話、差異和預覽都是檔案夾視窗中的一個 macOS 標籤頁。標籤列末端的「+」和 ⌘T 可開啟新檔案、終端機、瀏覽器以及每個已安裝的代理程式，也可放在獨立的 Git 工作樹中。
- 側邊欄可在「檔案」「搜尋」「代理程式」「Git」「CI」和「除錯」之間切換（⌘1–⌘6）；按一下右鍵可隱藏不使用的項目。工具列包含在編輯過的位置之間後退和前進（⌃- 和 ⌃⇧-）、分支、「快速打開」以及「活動」概覽。
- 「在旁邊打開」會在第一份文件旁顯示第二份文件。重新啟動或當機後，每個視窗都會連同其標籤頁恢復，含有未儲存文字的編輯器也會連同這些文字恢復。
- 影像、音訊、影片、字體和 Office 文件會透過 Quick Look 在標籤頁中打開，PDF 則透過 PDFKit。

## 編輯器
- 語法上色、行號、相對於上次提交的變更標記、尋找與取代、「快速打開」（⌘P）和專案搜尋（⇧⌘F）。
- 自動縮排、依 `.editorconfig` 使用 Tab 或空格、括號配對、切換註解、摺疊、前往行、顯示空白字元和自動換行。
- 多個插入點：⌥-按一下、⌥-拖移以選取一欄、⌘D 選取下一個相符項目。
- 檔案會保留其編碼、位元組順序標記和行尾字元。磁碟上的檔案有變更時，若編輯器中沒有未儲存的文字就會重新載入；否則會出現一列提示，提供差異與合併。
- **編輯預測：**輸入時會以灰色顯示建議，按 Tab 即可採用。建議來自 GitHub Copilot 或 Ollama、LM Studio 中的本機模型，依你在設定中的選擇而定；本機模型會讓你的程式碼留在這部 Mac 上。

## 語言伺服器
Go、Rust、TypeScript 和 JavaScript、Python、C、C++ 和 Objective-C、Swift、Shell 指令碼、JSON、YAML、CSS、HTML、TOML、Markdown 和 Lua，以及你在設定中加入的伺服器。診斷、懸停資訊、自動完成、簽章說明、前往定義、類型定義和實作、參照、呼叫階層和類型階層、重新命名、格式化、語意上色、摺疊和大綱；在行內顯示類型和參數名稱；燈泡（⌥⌘A）背後的快速修正和重構；用 ⌥⌘O 或在「快速打開」中輸入 `#` 尋找專案符號。若 TypeScript 專案本身的 `tsc` 可作為語言伺服器，就會使用它。

## 除錯
中斷點（也支援條件）、逐步執行、執行緒及其呼叫堆疊、檢閱器（⌥⌘I）中的變數和監看運算式、程式暫停時顯示在程式碼旁的值，以及在已暫停堆疊框中求值的除錯主控台。Rust、C、C++ 和 Swift 透過 `lldb-dap`，Go 透過 `dlv`，JavaScript 和 TypeScript 透過 JavaScript 除錯器，Python 透過 debugpy。會讀取專案的 `.vscode/launch.json`。「除錯」選單使用 Xcode 的快速鍵。

## 終端機
終端機在標籤頁中執行，使用登入 Shell 的環境，並以 Metal 繪製。貼上或拖放影像時，會將它交給在終端機中執行的程式設計代理程式，或貼上已儲存副本的路徑。⌘-按一下可打開連結和檔案名稱。終端機啟動的開發伺服器可以在瀏覽器標籤頁中打開。

## Markdown
Markdown 檔案打開時預覽會顯示在旁邊（⇧⌘V），支援公式、Mermaid、Graphviz 和 PlantUML 圖表以及檔案夾中的圖片。「格式」選單提供粗體、斜體、連結、標題、列表、引文和程式碼區塊；從 Word 或網頁貼上的文字會轉為 Markdown。**檔案 › 輸出**可寫出 PDF、單一 HTML 檔案或 Word 文件。若已安裝 Splat，**檔案 › 在 Splat 中打開**會在其中打開文件。

## 瀏覽器
瀏覽器標籤頁，具備網址欄位、上一頁和下一頁、搜尋、下載前先詢問，以及網頁檢閱器。

## Git
Git 面板支援暫存和丟棄整個檔案或單一變更區塊、提交、提交並推送或同步、分支、儲藏、合併衝突，以及近期提交的圖表。Blame 會顯示誰最後修改了游標所在行，也可以為整個檔案顯示一欄。一個按鈕可用 AI 撰寫提交訊息；只有在 gitleaks 未在差異中找到機密資訊後，差異才會送出，而且每次提交也同樣會檢查機密資訊。AI 對話可以在獨立的 Git 工作樹中工作；面板會顯示它變更的內容，並透過合併或 cherry-pick 整合其工作。

## CI
GitHub Actions 和 GitLab CI，也支援 GitHub Enterprise Server 和自行架設的 GitLab：目前分支的執行、作業和記錄；重新執行失敗的作業、單一作業或（在 GitHub 上）整個執行，啟動手動作業或核准等待中的部署，啟動新的管線或可手動觸發的工作流程，以及取消。開啟中的合併請求或拉取請求以及議題會列在同一個面板中，並在瀏覽器標籤頁中打開。失敗作業的記錄可以傳送給代理程式，並標示為外部文字。Token 來自鑰匙圈、`gh` 或 `glab`、登入 Shell 的環境，或由你輸入。

## AI
每個代理程式都在 AI 標籤頁中工作，時間軸相同：回答、工具呼叫及其輸出、權限請求、提問、待核准與評論的計畫、待辦事項、子代理程式和背景任務；標籤頁標題列中的兩個按鈕會列出執行中的子代理程式和背景程序。每次送出提示詞前，App 都會保存一個檢查點；變更可以依訊息檢視和回溯。新對話以「手動」模式開始，具破壞性的 Git 指令永遠不會被永久允許。「活動」概覽會列出所有執行中的項目。標籤頁上的綠點表示工作階段正在工作，紅點表示工作階段在等你；你在看別處時，工作階段完成或等待都會有通知提醒你。
- **Claude** 透過原封不動的 `claude` 指令，支援工作階段、子代理程式、背景任務、上下文和用量。
- **ACP 代理程式：**Codex、Antigravity 和 OpenCode，以及在你要求時來自 ACP 登錄庫的更多代理程式；已安裝的代理程式固定在經過測試的版本，並以核對碼驗證。
- **內建代理程式：**Anthropic、OpenAI、Google Gemini、OpenRouter、Ollama、LM Studio，以及相容 OpenAI 或 Anthropic API 的伺服器，具備用於檔案、搜尋、指令、診斷和網路請求的工具；金鑰保存在鑰匙圈中。
- 每個對話都有沙盒開關，在代理程式支援的範圍內限制其指令可執行的操作。
- 外部文字（例如終端機輸出、網頁或 CI 記錄）會標示為不可信任後再交給代理程式。

## 聽寫
透過 Lowspeak 的語音辨識在 AI 訊息欄位和編輯器中聽寫：使用麥克風按鈕、⌃⌥D，或在 Dango 為目前使用中的 App 時按 fn 鍵。聽寫使用 Lowspeak 的設定和模型；線上轉錄需要 OpenAI 或 Groq 的金鑰。

## 更新
App 透過 Sparkle 自動更新。第二次啟動時，它會詢問是否可自行檢查更新；在你同意之前，它不會發出任何請求。**Dango › 檢查更新項目⋯**會立即檢查。

## 輔助使用
側邊欄、標籤頁、AI 時間軸中的卡片、Git 和 CI 列、除錯器以及終端機文字都有 VoiceOver 名稱；可完全以鍵盤操作，快速鍵可在 `keybindings.json` 中更改；支援「增加對比」和「減少動態效果」；在由右至左的介面中，程式碼檢視仍維持由左至右。詳細資訊請見 ACCESSIBILITY.md。

## 隱私權
沒有遙測、沒有分析，也沒有當機報告。App 只會為你啟動的項目建立連線：你使用的代理程式、你要求的下載、你登入的 CI 主機、你選擇的 Copilot 預測、你選擇的線上轉錄、你打開的網頁，以及你允許後的更新檢查。金鑰和 Token 保存在 macOS 鑰匙圈中。

## 已知限制
- 透過瀏覽器登入 CI 的功能已關閉；請使用 `gh`、`glab`、Token 或環境變數。
- App 不在 App Sandbox 中執行，因為終端機、Git、語言伺服器和代理程式都會繼承它；檔案夾必須先獲得你的信任，其設定的任何項目才會執行。Antigravity 和 OpenCode 沒有可由 App 開啟的沙盒。
- Claude 的用量數據和上下文明細依賴其協定中標示為實驗性的部分，缺少這些部分時會隱藏。回溯對話內容僅適用於 Claude；對於其他代理程式，只會回溯程式碼。
- 不提供 Gemini CLI：Google 已停止以個人帳號登入，並引導使用者改用 Antigravity。先前的 Gemini 對話仍可閱讀。
