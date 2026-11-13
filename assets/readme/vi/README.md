<p><img src="../../readme-icon.png" alt="Biểu tượng ứng dụng Dango" width="168" height="168"></p>

# Dango

**Trình soạn thảo gốc cho macOS để làm việc với các tác nhân lập trình.**

Dango đặt mã, các tác nhân thay đổi mã và mọi thứ xung quanh chúng vào một cửa sổ cho mỗi thư mục: trình soạn thảo với máy chủ ngôn ngữ và dự đoán chỉnh sửa, terminal, trình duyệt, Git, CI, trình gỡ lỗi và các luồng AI với Claude, Codex, Antigravity, OpenCode hoặc một tác nhân tích hợp sẵn trong ứng dụng. Mọi tác nhân đều làm việc trong cùng một dòng thời gian, hỏi trước khi thay đổi tệp hoặc chạy lệnh, và để lại một điểm kiểm tra trước mỗi prompt, nên bạn có thể xem lại và tua lại những gì nó đã làm theo từng tin nhắn. Không có gì do thư mục cấu hình được chạy trước khi bạn tin cậy thư mục đó, và văn bản từ bên ngoài đến với tác nhân kèm dấu không đáng tin cậy. Đọc chính tả vào trình soạn thảo và trường tin nhắn, và tính năng nhận dạng giọng nói có thể ở lại trên máy Mac của bạn. Dango được viết bằng Rust trên các điều khiển AppKit thực thụ, không dùng Electron, có 35 ngôn ngữ và không có dữ liệu đo từ xa.

**Chưa phát hành.** Bản phát hành đầu tiên sẽ là một ảnh đĩa được ký bằng Developer ID và được Apple công chứng, dành cho macOS 14 trở lên trên Apple Silicon và Intel. Cho đến lúc đó, hãy [biên dịch Dango từ mã nguồn](#biên-dịch-từ-mã-nguồn).

**Phát hiện lỗi?** [Mở một issue](https://github.com/sventmueller/dango/issues/new) và cho biết lỗi xảy ra với tác nhân nào và trong loại dự án nào.

[![Rust phiên bản 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 trở lên](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon và Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit qua objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 ngôn ngữ giao diện](https://img.shields.io/badge/languages-35-4C9A2A)
![Không có dữ liệu đo từ xa](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Được ký với Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Giấy phép: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#giấy-phép)

[Bahasa Indonesia](../id/README.md) ·
[Bahasa Melayu](../ms/README.md) ·
[Català](../ca/README.md) ·
[Čeština](../cs/README.md) ·
[Dansk](../da/README.md) ·
[Deutsch](../de/README.md) ·
[English](../../../README.md) ·
[Español](../es/README.md) ·
[Français](../fr/README.md) ·
[Hrvatski](../hr/README.md) ·
[Italiano](../it/README.md) ·
[Magyar](../hu/README.md) ·
[Nederlands](../nl/README.md) ·
[Norsk](../no/README.md) ·
[Polski](../pl/README.md) ·
[Português (Brasil)](../pt_BR/README.md) ·
[Português (Portugal)](../pt_PT/README.md) ·
[Română](../ro/README.md) ·
[Slovenčina](../sk/README.md) ·
[Slovenščina](../sl/README.md) ·
[Suomi](../fi/README.md) ·
[Svenska](../sv/README.md) ·
**Tiếng Việt** ·
[Türkçe](../tr/README.md) ·
[Ελληνικά](../el/README.md) ·
[Русский](../ru/README.md) ·
[Українська](../uk/README.md) ·
[עברית](../he/README.md)‎ ·
[العربية](../ar/README.md)‎ ·
[हिन्दी](../hi/README.md) ·
[ไทย](../th/README.md) ·
[한국어](../ko/README.md) ·
[日本語](../ja/README.md) ·
[简体中文](../zh-Hans/README.md) ·
[繁體中文](../zh-Hant/README.md)

## Mục lục

- [Tóm tắt tính năng](#tóm-tắt-tính-năng)
- [Yêu cầu](#yêu-cầu)
- [Cài đặt](#cài-đặt)
- [Bắt đầu](#bắt-đầu)
- [Tác nhân AI](#tác-nhân-ai)
  - [Dòng thời gian](#dòng-thời-gian)
  - [Quyền và chế độ](#quyền-và-chế-độ)
  - [Điểm kiểm tra và worktree](#điểm-kiểm-tra-và-worktree)
- [Trình soạn thảo](#trình-soạn-thảo)
- [Máy chủ ngôn ngữ](#máy-chủ-ngôn-ngữ)
- [Kiểm thử](#kiểm-thử)
- [Trình gỡ lỗi](#trình-gỡ-lỗi)
- [Terminal](#terminal)
- [Trình duyệt](#trình-duyệt)
- [Git](#git)
- [CI](#ci)
- [Container](#container)
- [Markdown](#markdown)
- [Đọc chính tả](#đọc-chính-tả)
- [Tin cậy](#tin-cậy)
- [Sandbox](#sandbox)
- [Giao diện và trợ năng](#giao-diện-và-trợ-năng)
- [Ngôn ngữ](#ngôn-ngữ)
- [Phím tắt](#phím-tắt)
- [Quyền riêng tư và bảo mật](#quyền-riêng-tư-và-bảo-mật)
- [Nơi lưu dữ liệu](#nơi-lưu-dữ-liệu)
- [Giới hạn đã biết](#giới-hạn-đã-biết)
- [Biên dịch từ mã nguồn](#biên-dịch-từ-mã-nguồn)
- [Giấy phép](#giấy-phép)

## Tóm tắt tính năng

- **Mọi tác nhân trong một dòng thời gian.** Claude qua lệnh `claude` riêng của nó, Codex, Antigravity và OpenCode qua Agent Client Protocol, thêm nhiều tác nhân khác từ sổ đăng ký ACP, và một tác nhân tích hợp sẵn trong ứng dụng cho Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama và LM Studio. Câu trả lời, lệnh gọi công cụ, yêu cầu cấp quyền, kế hoạch, việc cần làm, tác nhân phụ và tác vụ nền trông giống nhau dù tác nhân nào đang chạy.
- **Xem lại và tua lại.** Trước mỗi prompt, Dango lưu một điểm kiểm tra. Những gì tác nhân đã thay đổi có thể được xem lại và tua lại theo từng tin nhắn, và một luồng có thể làm việc trong Git worktree riêng rồi bàn giao công việc bằng hợp nhất hoặc cherry-pick.
- **Một cửa sổ cho mỗi thư mục.** Tệp, terminal, trang trình duyệt, luồng AI, diff và bản xem trước là các tab macOS của cửa sổ đó. Thanh bên chứa Tệp, Tìm kiếm, Tác nhân, Git, Kiểm thử, Gỡ lỗi, CI và Container.
- **Một trình soạn thảo mã thực thụ.** Tô màu cú pháp, nhiều con trỏ, thu gọn khối mã, `.editorconfig`, máy chủ ngôn ngữ cho 17 ngôn ngữ với tự động hoàn thành, đi tới định nghĩa, tham chiếu, đổi tên, sửa nhanh và phân cấp, cùng dự đoán chỉnh sửa từ GitHub Copilot hoặc một mô hình cục bộ.
- **Tích hợp sẵn trình gỡ lỗi, Git và CI.** Điểm ngắt và chạy từng bước cho Rust, C, C++, Swift, Go, JavaScript, TypeScript và Python; stage từng khối thay đổi, nhánh, stash, xung đột và biểu đồ commit; các lần chạy, job và nhật ký từ GitHub Actions và GitLab CI.
- **Cẩn trọng với thư mục của bạn.** Thư mục bạn chưa tin cậy không khởi động gì cả: không tiến trình Git, máy chủ ngôn ngữ, hook hay tác nhân nào. Các lệnh Git mang tính phá hủy không bao giờ được cho phép vĩnh viễn, và commit được kiểm tra thông tin bí mật.
- **Đọc chính tả.** Nói vào trình soạn thảo và trường tin nhắn, trực tuyến qua OpenAI hoặc Groq, hoặc cục bộ với Whisper và Parakeet.
- **Gốc đến tận lõi.** Cửa sổ, tab, menu và thanh công cụ AppKit, TextKit 2 cho trình soạn thảo, Metal cho terminal, WebKit chỉ trong tab trình duyệt. Chế độ tối, Tăng độ tương phản và VoiceOver hoạt động như trong mọi ứng dụng Mac.
- **Bằng ngôn ngữ của bạn.** Giao diện có sẵn bằng 35 ngôn ngữ: Ả Rập, Catalan, Trung Quốc (Giản thể), Trung Quốc (Phồn thể), Croatia, Séc, Đan Mạch, Hà Lan, Anh, Phần Lan, Pháp, Đức, Hy Lạp, Do Thái, Hindi, Hungary, Indonesia, Ý, Nhật, Hàn, Mã Lai, Na Uy, Ba Lan, Bồ Đào Nha (Brazil), Bồ Đào Nha (Bồ Đào Nha), Romania, Nga, Slovakia, Slovenia, Tây Ban Nha, Thụy Điển, Thái, Thổ Nhĩ Kỳ, Ukraina và Việt Nam.

## Yêu cầu

- macOS 14 Sonoma trở lên, trên Apple Silicon hoặc Intel. Dango được phát triển và kiểm thử trên macOS 27.
- Với các luồng AI, cần tác nhân bạn muốn dùng; xem [Tác nhân AI](#tác-nhân-ai).
- Với máy chủ ngôn ngữ và bộ chuyển đổi gỡ lỗi, những thứ Dango cài khi bạn yêu cầu: Go cho gopls và `dlv`; Node.js cho các máy chủ TypeScript, Python, JSON, YAML, CSS, HTML và shell cùng trình gỡ lỗi JavaScript; Xcode Command Line Tools cho clangd, sourcekit-lsp và `lldb-dap`. rust-analyzer được tải xuống từ các bản phát hành của nó trên GitHub.
- Với dự đoán chỉnh sửa, cần gói đăng ký GitHub Copilot (gói miễn phí là đủ), hoặc một mô hình fill-in-the-middle trong Ollama hay LM Studio.

## Cài đặt

**Từ ảnh đĩa.** Mỗi bản phát hành là một ảnh đĩa, `Dango-<version>.dmg`, chứa một ứng dụng universal cho Apple Silicon và Intel, được ký bằng Developer ID và được Apple công chứng. Mở ảnh đĩa và kéo Dango vào thư mục Ứng dụng. Một Homebrew Cask cài cùng ảnh đĩa đó. Hiện chưa có bản phát hành nào.

**Cập nhật.** Dango cài từ ảnh đĩa tự cập nhật bằng [Sparkle](https://sparkle-project.org). Ở lần mở thứ hai, ứng dụng hỏi xem có được tự tìm bản cập nhật không; cho đến khi bạn đồng ý, ứng dụng không gửi yêu cầu nào. **Dango › Kiểm tra Bản cập nhật…** kiểm tra ngay lập tức. Bản cập nhật được ký bằng khóa riêng của ứng dụng, và Sparkle kiểm tra khóa đó trước khi cài bất cứ thứ gì. Bản build do bạn tự biên dịch không có trình cập nhật.

**Từ mã nguồn.** Xem [Biên dịch từ mã nguồn](#biên-dịch-từ-mã-nguồn), sau đó chuyển `target/release/Dango.app` vào `/Applications`.

**Lệnh terminal.** **Dango › Cài đặt lệnh “dango”…** ghi một tập lệnh nhỏ vào `~/.local/bin/dango`, không cần quyền quản trị; `~/.local/bin` phải nằm trong `PATH` của bạn. Sau đó `dango .` mở thư mục hiện tại và `dango file.rs` mở một tệp. Dango không bao giờ thay thế một tệp ở đó mà không phải do chính nó ghi.

## Bắt đầu

1. **Mở một thư mục** bằng ⌘O hoặc `dango <folder>`. Lần đầu tiên, Dango hỏi bạn có tin cậy thư mục đó không (xem [Tin cậy](#tin-cậy)).
2. **Thanh bên** chuyển giữa Tệp (⌘1), Tìm kiếm (⌘2), Tác nhân (⌘3), Git (⌘4), Kiểm thử (⌘5), Gỡ lỗi (⌘6), CI (⌘7) và Container (⌘8); bấm chuột phải để ẩn những mục bạn không dùng, và ⌃⌘S thu gọn thanh bên.
3. **Mỗi tài liệu là một tab** trong cửa sổ của thư mục, chính là tab của macOS. Nút “+” ở cuối thanh tab và ⌘T cung cấp tệp mới, terminal, trình duyệt và từng tác nhân đã cài, kể cả trong một Git worktree riêng; giữ nút “+” sẽ hiện các mục đó dưới dạng menu.
4. **Mở nhanh** bằng ⌘P tìm tệp, và với `#` tìm ký hiệu của dự án; ⇧⌘F tìm và thay thế trong dự án.
5. **Trò chuyện với tác nhân** trong một tab AI. ⌘Esc chuyển tiêu điểm bàn phím vào trường tin nhắn của nó và quay lại.
6. **Cài đặt** bằng ⌘, chứa các tab Cài đặt chung, CI, Tác nhân, Tác nhân tích hợp, Máy chủ ngôn ngữ, Đọc chính tả và Dự đoán.

Thanh công cụ chứa nút lùi và tiến qua những chỗ bạn đã sửa (⌃- và ⌃⇧-), nhánh, Mở nhanh và tổng quan Hoạt động, nơi liệt kê mọi thứ đang chạy. **Mở ở Bên cạnh** trong menu ngữ cảnh của một tệp hiển thị tài liệu thứ hai bên cạnh tài liệu đầu tiên. Sau khi khởi động lại hoặc gặp sự cố, mọi cửa sổ trở lại cùng các tab của nó, và các trình soạn thảo có văn bản chưa lưu sẽ trở lại cùng văn bản đó. Hình ảnh, âm thanh, video, phông chữ và tài liệu Office mở trong một tab qua Quick Look, PDF qua PDFKit.

## Tác nhân AI

| Tác nhân | Cách chạy | Cần gì |
|---|---|---|
| Claude | lệnh `claude`, giao tiếp trực tiếp qua giao thức stream-json của nó | `claude` đã cài và đã đăng nhập; Dango tìm trong `PATH` của login shell, trong `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` và `/usr/local/bin`. Việc đăng nhập chỉ diễn ra trong chính `claude` |
| Codex | `codex-acp` qua Agent Client Protocol (ACP) | Node.js và npm; Dango cài gói đã ghim phiên bản khi bạn yêu cầu |
| Antigravity | `agy acp` | `agy` của riêng bạn trong `PATH`, hoặc bản tải xuống đã ghim phiên bản, được cài khi bạn yêu cầu |
| OpenCode | `opencode acp` | `opencode` của riêng bạn trong `PATH`, hoặc bản tải xuống đã ghim phiên bản, được cài khi bạn yêu cầu |
| Các tác nhân ACP khác | từ sổ đăng ký ACP, chỉ được tải về khi bạn yêu cầu | những gì mục trong sổ đăng ký nêu ra; các mục không có tổng kiểm tra chỉ được cài sau khi bạn xác nhận |
| Tác nhân tích hợp | tích hợp sẵn trong ứng dụng, với công cụ cho tệp, tìm kiếm, lệnh, chẩn đoán và yêu cầu web | khóa API cho Anthropic, OpenAI, Google Gemini hoặc OpenRouter, được giữ trong Chuỗi khóa; Ollama hoặc LM Studio trên máy Mac này mà không cần khóa; bất kỳ máy chủ nào dùng API của OpenAI hoặc Anthropic |

**Cài đặt › Tác nhân** cho biết máy Mac này có những tác nhân nào, cài các tác nhân khác cùng bản cập nhật của chúng, và mở sổ đăng ký ACP. Các tác nhân Dango cài được ghim vào các phiên bản đã kiểm thử và được đối chiếu với tổng kiểm tra đã công bố của chúng (npm integrity hoặc SHA-256); Dango không chạy gói `npx` nào không có phiên bản.

### Dòng thời gian

Mọi tác nhân đều làm việc trong một tab AI với cùng một dòng thời gian: câu trả lời, lệnh gọi công cụ cùng đầu ra, yêu cầu cấp quyền, câu hỏi, kế hoạch để phê duyệt và nhận xét (⌥⌘K), việc cần làm, tác nhân phụ và tác vụ nền. Hai nút trong phần đầu tab liệt kê các tác nhân phụ và tiến trình nền đang chạy. Nhắc đến một tệp, `@terminal` hoặc `@browser` để chuyển nội dung của chúng cho tác nhân. Chấm xanh lá trên tab cho biết phiên đang làm việc, chấm đỏ cho biết phiên đang chờ bạn, và một thông báo cho bạn biết khi phiên hoàn tất hoặc đang chờ trong lúc bạn xem chỗ khác. Với Claude, dòng trạng thái hiển thị ngữ cảnh và mức sử dụng.

### Quyền và chế độ

Luồng mới bắt đầu ở chế độ “Thủ công”: tác nhân hỏi trước khi thay đổi tệp hoặc chạy lệnh. Với các lệnh Git mang tính phá hủy (`push --force`, `reset --hard`, `clean -fd`), Dango không cung cấp “Luôn cho phép”. Tác nhân tích hợp giữ các quy tắc “Luôn cho phép” theo từng dự án, liệt kê chúng trong **Cài đặt › Tác nhân tích hợp** và luôn hỏi đối với lệnh Git mang tính phá hủy, bất kể đã lưu gì; Claude và các tác nhân ACP giữ các quy tắc như vậy trong cấu hình riêng của chúng. Với “Cho phép mọi thứ” và không có sandbox, luồng hiển thị một cảnh báo thường trực.

### Điểm kiểm tra và worktree

Trước mỗi prompt, Dango lưu một điểm kiểm tra, dưới dạng commit trong `refs/ide-checkpoints/` của kho lưu trữ dự án, hoặc trong thư mục riêng của nó đối với dự án không dùng Git; một lần push hoặc fetch thông thường không bao giờ mang chúng theo. Những gì tác nhân đã thay đổi có thể được xem lại và tua lại theo từng tin nhắn. Một luồng có thể làm việc trong Git worktree riêng: khung Git hiển thị những gì luồng đó đã thay đổi và tiếp nhận công việc bằng hợp nhất hoặc cherry-pick.

## Trình soạn thảo

- **Tô màu cú pháp, số dòng và dấu thay đổi** so với commit gần nhất, tìm và thay thế, Mở nhanh (⌘P) và tìm kiếm trong dự án (⇧⌘F).
- **Thụt lề** theo tệp và `.editorconfig`: tab hoặc dấu cách, tự động thụt lề, cặp dấu ngoặc.
- **Nhiều con trỏ:** ⌥-bấm, ⌥-kéo để chọn một cột, ⌃⇧↑ và ⌃⇧↓, ⌘D cho lần xuất hiện tiếp theo.
- **Thu gọn khối mã** (⌥⌘← và ⌥⌘→, kèm ⇧ cho tất cả), bật/tắt chú thích (⌘/), đi tới dòng (⌃G), hiển thị khoảng trắng và ngắt dòng (⌥Z).
- **Tệp giữ nguyên bảng mã**, dấu thứ tự byte và ký tự xuống dòng. Tệp bị thay đổi trên ổ đĩa sẽ được tải lại khi trình soạn thảo không có văn bản chưa lưu; nếu không, một thanh thông báo sẽ đưa ra các khác biệt và tùy chọn hợp nhất.
- **Dự đoán chỉnh sửa.** Khi bạn gõ, một đề xuất xuất hiện bằng chữ xám, và Tab sẽ nhận đề xuất đó. Đề xuất đến từ GitHub Copilot hoặc từ một mô hình cục bộ trong Ollama hay LM Studio, tùy lựa chọn trong **Cài đặt › Dự đoán**; mô hình cục bộ giữ mã của bạn trên máy Mac này.
- **Bản đồ thu nhỏ và Cuộn cố định** (menu Trình soạn thảo): hình hẹp của toàn bộ tệp bên cạnh văn bản, và các dòng tiêu đề của những khối chứa dòng trên cùng, được giữ cố định phía trên dòng đó.
- **Chẩn đoán ở cuối dòng** (menu Xem): thông báo nghiêm trọng nhất của mỗi dòng sau một giây không gõ, kèm biểu tượng cho mức độ nghiêm trọng.
- **Xem trước trực tiếp trang tĩnh:** Trình soạn thảo › Mở trong tab trình duyệt phục vụ các trang của một thư mục được tin cậy ngay từ Dango và tải lại tab khi một tệp được lưu.
- **Prettier** định dạng JavaScript, TypeScript, CSS, HTML, Markdown và nhiều loại khác ở nơi dự án cấu hình nó, bằng `node_modules/.bin/prettier` riêng của dự án; cài đặt `formatter` chọn trình định dạng cho từng ngôn ngữ.
- **Chế độ Vim** (Trình soạn thảo › Chế độ Vim): chế độ Normal, Chèn, Visual và Thay thế, số đếm, toán tử với chuyển động và đối tượng văn bản, `.`, thanh ghi, dấu và macro. `u` và ⌃R dùng chức năng hoàn tác của trình soạn thảo, và một lượt chèn là một bước. `"+` và `"*` là bảng nhớ tạm; cài đặt `vimClipboard` biến cả thanh ghi không tên thành bảng nhớ tạm. `/`, `?`, `n`, `N`, `*`, `:s` và `:%s` tìm kiếm bằng biểu thức chính quy của Rust, không phải mẫu của Vim: không có `\v`, và `\b` thay cho `\<` và `\>`. Lệnh Ex là `:w`, `:q`, `:wq`, `:x`, `:noh` và `:<line>`. Ở chế độ Normal, ⌃G thuộc về Vim thay vì đi tới dòng.
- **Đồng bộ hóa cài đặt** qua một thư mục trong iCloud Drive (**Cài đặt › Cài đặt chung**): cài đặt và phím tắt của bạn theo bạn sang các máy Mac khác; đường dẫn, địa chỉ cục bộ và các sự đồng ý vẫn ở riêng trên từng máy.

## Máy chủ ngôn ngữ

Go, Rust, TypeScript và JavaScript, Python, C, C++ và Objective-C, Swift, shell script, JSON, YAML, CSS, HTML, TOML, Markdown và Lua, cùng các máy chủ riêng của bạn từ **Cài đặt › Máy chủ ngôn ngữ**. Dango cài máy chủ khi bạn yêu cầu, vào thư mục công cụ riêng của nó, và dùng `tsc` riêng của một dự án TypeScript khi nó có thể làm máy chủ ngôn ngữ.

Những gì chúng mang lại: chẩn đoán (⌘' và ⇧⌘' di chuyển giữa các chẩn đoán), thông tin khi di chuột (⌃⌘I), tự động hoàn thành (⌥Esc), trợ giúp chữ ký hàm, đi tới định nghĩa (⌃⌘J), định nghĩa kiểu và triển khai, tham chiếu (⌃⇧⌘F), phân cấp lệnh gọi và phân cấp kiểu (⌃⇧⌘H), đổi tên (⌃⌘E), định dạng (⌥⇧F), màu ngữ nghĩa, thu gọn khối mã và dàn ý (⇧⌘O); kiểu và tên tham số ngay trong dòng; sửa nhanh và tái cấu trúc sau biểu tượng bóng đèn (⌥⌘A); ký hiệu của dự án bằng ⌥⌘O.

Bên cạnh máy chủ riêng của tệp, ESLint kiểm tra JavaScript và TypeScript của những dự án dùng nó, kèm các bản sửa nhanh của nó, và các viết tắt của Emmet xuất hiện trong tự động hoàn thành cho HTML, CSS và JSX. Dango cài cả hai khi bạn yêu cầu; **Cài đặt › Cài đặt chung** sửa các vấn đề của ESLint khi lưu nếu bạn muốn.

## Kiểm thử

Khung Kiểm thử liệt kê các kiểm thử của một thư mục theo gói và tệp cùng trạng thái của chúng: `cargo test` hoặc cargo-nextest, Go, pytest, Vitest và Jest. Một biểu tượng cạnh mỗi kiểm thử ở lề sẽ chạy kiểm thử đó, ⌥-bấm để gỡ lỗi nó, và một assert thất bại đánh dấu dòng của nó. Menu Gỡ lỗi chạy kiểm thử tại con trỏ (⌃⌥⌘U), chạy lại các kiểm thử gần nhất (⌃⌥⌘G) hoặc tất cả, và gỡ lỗi kiểm thử tại con trỏ. Kiểm thử chỉ chạy trong thư mục được tin cậy, vì chúng chạy mã của dự án; `cargo test` chỉ liệt kê các kiểm thử của nó khi bạn yêu cầu, vì việc liệt kê sẽ build mọi chương trình kiểm thử.

## Trình gỡ lỗi

Điểm ngắt, kể cả có điều kiện và dưới dạng điểm ghi nhật ký, chạy từng bước, các luồng thực thi cùng ngăn xếp lệnh gọi của chúng, biến và biểu thức theo dõi trong Bộ kiểm tra (⌥⌘I), giá trị bên cạnh mã khi chương trình đang tạm dừng, và bảng điều khiển gỡ lỗi để tính biểu thức trong khung đang tạm dừng.

| Ngôn ngữ | Bộ chuyển đổi gỡ lỗi |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` từ Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | trình gỡ lỗi JavaScript, chạy trên Node.js |
| Python | debugpy |

Tệp `.vscode/launch.json` của dự án được đọc. Menu Gỡ lỗi dùng các phím tắt của Xcode: ⌘\\ đặt điểm ngắt, ⌘Y bật hoặc tắt tất cả, ⌃⌘Y tiếp tục, F6, F7 và F8 bước qua, bước vào và bước ra, ⌘. dừng.

## Terminal

Terminal trong các tab (⌃\`) với môi trường của login shell, vẽ bằng Metal. ⌘K xóa màn hình. Dán hoặc thả một hình ảnh sẽ chuyển nó cho tác nhân lập trình đang chạy trong terminal, hoặc dán đường dẫn của một bản sao đã lưu. ⌘-bấm để mở liên kết và tên tệp. Máy chủ phát triển do terminal khởi động có thể mở trong tab trình duyệt.

## Trình duyệt

Tab trình duyệt với trường địa chỉ (⌘L), lùi và tiến (⌘[ và ⌘]), tải lại (⌘R), tìm kiếm, tải xuống có hỏi trước, và Trình kiểm tra web. `@browser` chuyển trang cho tác nhân, kèm dấu là văn bản từ bên ngoài.

## Git

Khung Git stage và hủy thay đổi của tệp hoặc từng khối thay đổi, commit, commit rồi push hoặc đồng bộ, chuyển và tạo nhánh, giữ stash, giải quyết xung đột hợp nhất và vẽ biểu đồ các commit gần đây. Blame cho biết ai đã sửa dòng tại con trỏ lần cuối, hoặc hiển thị một cột cho cả tệp. Một nút viết thông điệp commit bằng AI; diff chỉ được gửi đi sau khi gitleaks không tìm thấy thông tin bí mật nào trong đó, và mọi commit cũng được kiểm tra thông tin bí mật.

## CI

GitHub Actions và GitLab CI, kể cả GitHub Enterprise Server và GitLab tự quản lý:

- các lần chạy, job và nhật ký của nhánh hiện tại;
- chạy lại job thất bại, một job riêng lẻ hoặc, trên GitHub, cả một lần chạy; khởi động job thủ công hoặc phê duyệt các lần triển khai đang chờ; khởi động pipeline mới hoặc workflow có thể chạy thủ công; hủy;
- các pull request hoặc merge request đang mở và các issue, mở trong tab trình duyệt;
- nhật ký của job thất bại được chuyển cho tác nhân, kèm dấu là văn bản từ bên ngoài.

Token lấy từ Chuỗi khóa, `gh` hoặc `glab`, môi trường của login shell, hoặc token bạn nhập trong **Cài đặt › CI**.

## Container

Khung Container hiển thị các dự án Compose của Docker cùng các service của chúng, các container riêng lẻ, image và volume, đồng thời theo dõi sự kiện của Docker ngay khi chúng xảy ra. Tệp Compose của thư mục xuất hiện như một dự án ngay cả trước khi có container nào của nó. Container có thể khởi động, dừng và khởi động lại; nhật ký của chúng và một shell mở trong các tab terminal, còn cổng đã công bố mở trong tab trình duyệt. Việc xóa một container, image hoặc volume, và dọn dẹp dữ liệu không dùng, đều hỏi trước. Các lệnh up, down và pull của Compose chỉ chạy trong thư mục được tin cậy, vì Compose build và chạy những gì dự án mô tả. Menu của khung chọn ngữ cảnh Docker cho cửa sổ mà không thay đổi ngữ cảnh shell của bạn đang dùng; các lệnh tương tự có trong menu Container. Khung cần dòng lệnh `docker`, Docker 24.0.6 trở lên.

## Markdown

- **Xem trước.** Tệp Markdown mở cùng bản xem trước bên cạnh; **Xem › Hiện/Ẩn Bản xem trước ở Bên cạnh** (⇧⌘V) đóng và mở bản xem trước, và **Cài đặt › Cài đặt chung** tắt xem trước tự động. Bản xem trước vẽ công thức, sơ đồ Mermaid, Graphviz và PlantUML và hình ảnh trong thư mục; hình ảnh từ web chỉ tải trong thư mục được tin cậy hoặc sau khi bạn yêu cầu trong bản xem trước.
- **Menu Định dạng:** in đậm (⌘B), in nghiêng (⌘I), gạch ngang (⇧⌘X), mã (⌥⇧⌘C), liên kết (⇧⌘K), tiêu đề 1–3 (⌥⌘1–⌥⌘3), danh sách dấu đầu dòng và đánh số, việc cần làm, trích dẫn và khối mã. Mỗi lệnh bật hoặc tắt định dạng, trên vùng chọn hoặc các dòng, và là một bước hoàn tác.
- **Dán.** Văn bản từ Word hoặc trang web đến dưới dạng Markdown, hình ảnh của nó thành các tệp bên cạnh tài liệu; ⌥⇧⌘V dán văn bản thuần.
- **Xuất.** **Tệp › Xuất** ghi ra PDF, một tệp HTML duy nhất chứa sẵn hình ảnh, hoặc tài liệu Word. Hình ảnh từ web chỉ được đưa vào khi xuất từ thư mục được tin cậy.
- **Mở trong Splat.** Nếu đã cài [Splat](https://github.com/sventmueller/splat), **Tệp › Mở trong Splat** và menu ngữ cảnh của cây tệp và các tab sẽ mở tài liệu trong đó, sau khi lưu tài liệu.

## Đọc chính tả

Đọc chính tả vào trường tin nhắn của tab AI hoặc trình soạn thảo: bằng nút micrô ở đó, **Sửa › Bắt đầu đọc chính tả** (⌃⌥D), hoặc phím fn (giữ để ghi âm, chạm hai lần để tiếp tục ghi âm). Tính năng nhận dạng giọng nói là của [Lowspeak](https://github.com/sventmueller/lowspeak); không cần cài chính Lowspeak.

- **Cài đặt › Đọc chính tả** đặt chế độ và mô hình (trực tuyến với OpenAI hoặc Groq, cục bộ với Whisper hoặc Parakeet), ngôn ngữ, hậu xử lý, micrô và từ vựng riêng của bạn.
- **Mô hình cục bộ** được tải xuống từ Hugging Face khi bạn chọn, có hiển thị tiến trình và cách để hủy. Chúng được đặt trong các thư mục Lowspeak dùng, nên mỗi ứng dụng đều dùng được những gì ứng dụng kia đã tải xuống. Mô hình cục bộ giữ bản ghi âm trên máy Mac này.
- **Chép lời trực tuyến** cần khóa OpenAI hoặc Groq, được Dango giữ trong mục Chuỗi khóa riêng của nó.
- **Từ Lowspeak.** Nếu Lowspeak được thiết lập khác với mặc định, tab Đọc chính tả đề nghị một lần việc lấy cài đặt của nó, và lấy khóa của nó khi bạn yêu cầu; macOS sẽ hỏi trước khi Dango đọc một khóa (hãy chọn “Cho phép”, không chọn “Luôn cho phép”).
- **Quyền.** macOS hỏi quyền dùng micrô ở lần đọc chính tả đầu tiên. Phím fn hoạt động khi Dango là ứng dụng đang hoạt động và Lowspeak không chạy; phím này cần quyền Trợ năng, quyền mà Dango hỏi sau lần đọc chính tả đầu tiên, không bao giờ khi khởi chạy.

## Tin cậy

Một thư mục có thể khiến chương trình chạy ngay khi có thứ gì đó được khởi động trong đó: hook Git và cài đặt kho lưu trữ như `core.fsmonitor`, máy chủ ngôn ngữ từ `node_modules`, hook của tác nhân, `.mcp.json`. Lần đầu bạn mở một thư mục, Dango hỏi bạn có tin cậy thư mục đó không.

- **Được tin cậy:** Git, máy chủ ngôn ngữ, trình gỡ lỗi, tác nhân và CI chạy cho thư mục này và mọi thứ bên trong nó.
- **Bị hạn chế:** bạn có thể đọc và sửa; dấu thay đổi trong trình soạn thảo đến từ việc đọc trực tiếp `.git`. Không có gì do thư mục cấu hình được khởi động. Các khung Git, CI và AI thông báo điều này và đề nghị tin cậy thư mục.

Văn bản không đến từ bạn có thể mang các chỉ dẫn nhằm điều khiển tác nhân (prompt injection). Vì vậy, đầu ra của terminal (`@terminal`), trang trình duyệt (`@browser`), nhật ký CI được chuyển cho tác nhân, tệp nằm ngoài dự án và văn bản dài hơn được dán từ trang web đều được gửi kèm dấu: chip ghi “Nội dung bên ngoài”, và tác nhân nhận văn bản cùng lưu ý rằng đó là dữ liệu không đáng tin cậy mà nó không được làm theo các chỉ dẫn trong đó. Các trang web của tác nhân tích hợp cũng đến kèm lưu ý tương tự. Văn bản được dán được xem là đến từ trang web khi trình duyệt nói như vậy trên bảng nhớ tạm, như Safari, Chrome và Firefox vẫn làm; văn bản từ nơi khác, và một cụm ngắn vài từ, vẫn là của bạn.

## Sandbox

Bản thân Dango chạy không có App Sandbox, vì mọi chương trình nó khởi động đều sẽ thừa hưởng sandbox đó: terminal, Git, máy chủ ngôn ngữ và tác nhân sẽ không thể hoạt động. Thay vào đó, mỗi luồng AI có một công tắc sandbox, mặc định tắt. Phạm vi giới hạn tùy thuộc vào tác nhân, và chú giải của công tắc cho biết điều đó:

| Tác nhân | Khi bật sandbox |
|---|---|
| Claude | lệnh shell chạy trong sandbox riêng của Claude; công cụ tệp và web của nó thì không |
| Codex | lệnh và thay đổi chỉ ghi trong dự án, không có mạng |
| Tác nhân tích hợp | tệp, yêu cầu web và lệnh chỉ ở trong dự án và các thư mục được cho phép, không có mạng (`sandbox-exec`) |
| Antigravity, OpenCode | không có sandbox nào Dango có thể bật |

## Giao diện và trợ năng

- **Chế độ tối** theo hệ thống; **Tăng độ tương phản** và **Giảm chuyển động** được tôn trọng.
- **VoiceOver** đọc tên thanh bên, các tab, thẻ của dòng thời gian AI, các hàng Git và CI, trình gỡ lỗi và văn bản của terminal. Điểm ngắt không bao giờ được phân biệt chỉ bằng màu sắc.
- **Bàn phím.** Mọi lệnh đều có phím tắt, và tất cả đều có thể thay đổi trong `keybindings.json`; ⌥⌘\` chuyển sang khung tiếp theo.
- **Phải sang trái.** Trong tiếng Ả Rập và tiếng Do Thái, cửa sổ được lật gương, còn khung xem mã vẫn từ trái sang phải.

Những gì Dango bổ sung trên nền AppKit, theo từng khu vực, có trong [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Ngôn ngữ

Giao diện được dịch sang 35 ngôn ngữ: Ả Rập, Catalan, Trung Quốc (Giản thể), Trung Quốc (Phồn thể), Croatia, Séc, Đan Mạch, Hà Lan, Anh, Phần Lan, Pháp, Đức, Hy Lạp, Do Thái, Hindi, Hungary, Indonesia, Ý, Nhật, Hàn, Mã Lai, Na Uy, Ba Lan, Bồ Đào Nha (Brazil), Bồ Đào Nha (Bồ Đào Nha), Romania, Nga, Slovakia, Slovenia, Tây Ban Nha, Thụy Điển, Thái, Thổ Nhĩ Kỳ, Ukraina và Việt Nam.

Dango theo thứ tự ngôn ngữ trong **Cài đặt hệ thống › Cài đặt chung › Ngôn ngữ & Vùng**; tại đó bạn cũng có thể chọn riêng một ngôn ngữ cho Dango trong **Ứng dụng**. Nếu không có bản dịch cho ngôn ngữ của bạn, Dango dùng tiếng Anh. Thuật ngữ theo một bảng thuật ngữ, [docs/glossary.md](../../../docs/glossary.md), và bản dịch của chính Apple, nên Dango đọc như phần còn lại của máy Mac.

## Phím tắt

| Thao tác | Phím tắt |
|---|---|
| Mở thư mục, cửa sổ mới, đóng cửa sổ | ⌘O, ⇧⌘N, ⇧⌘W |
| Tệp mới, lưu, lưu thành | ⌘N, ⌘S, ⇧⌘S |
| Tab mới, đóng tab, tab tiếp theo và trước đó | ⌘T, ⌘W, ⇧⌘] và ⇧⌘[ |
| Mở nhanh, tìm kiếm trong dự án | ⌘P, ⇧⌘F |
| Tệp, Tìm kiếm, Tác nhân, Git, Kiểm thử, Gỡ lỗi, CI, Container | ⌘1 đến ⌘8 |
| Hiện hoặc ẩn thanh bên, khung tiếp theo | ⌃⌘S, ⌥⌘\` |
| Lùi và tiến qua những chỗ đã sửa | ⌃- và ⌃⇧- |
| Bật/tắt chú thích, đi tới dòng | ⌘/, ⌃G |
| Lần xuất hiện tiếp theo, con trỏ phía trên và phía dưới | ⌘D, ⌃⇧↑ và ⌃⇧↓ |
| Thu gọn, mở rộng, thu gọn tất cả, mở rộng tất cả | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Ngắt dòng, Bộ kiểm tra | ⌥Z, ⌥⌘I |
| Tự động hoàn thành, thông tin khi di chuột, đi tới định nghĩa | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Tham chiếu, phân cấp lệnh gọi, đổi tên | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Ký hiệu trong tệp, trong dự án | ⇧⌘O, ⌥⌘O |
| Sửa nhanh, định dạng | ⌥⌘A, ⌥⇧F |
| Vấn đề tiếp theo và trước đó | ⌘' và ⇧⌘' |
| Xem trước Markdown, dán dưới dạng văn bản thuần | ⇧⌘V, ⌥⇧⌘V |
| In đậm, in nghiêng, gạch ngang, mã, liên kết | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Tiêu đề 1 đến 3 | ⌥⌘1 đến ⌥⌘3 |
| Terminal mới, xóa terminal | ⌃\`, ⌘K |
| Đưa tiêu điểm vào trường tin nhắn của tác nhân, nhận xét kế hoạch | ⌘Esc, ⌥⌘K |
| Trường địa chỉ, tải lại, lùi, tiến (trình duyệt) | ⌘L, ⌘R, ⌘[, ⌘] |
| Bắt đầu hoặc dừng đọc chính tả | ⌃⌥D |
| Điểm ngắt, bật hoặc tắt tất cả điểm ngắt | ⌘\\, ⌘Y |
| Chạy kiểm thử tại con trỏ, chạy lại các kiểm thử gần nhất | ⌃⌥⌘U, ⌃⌥⌘G |
| Tiếp tục, bước qua, bước vào, bước ra, dừng | ⌃⌘Y, F6, F7, F8, ⌘. |
| Cài đặt | ⌘, |

`keybindings.json` trong thư mục dữ liệu của ứng dụng (xem [Nơi lưu dữ liệu](#nơi-lưu-dữ-liệu)) thay đổi bất kỳ phím tắt nào trong số này.

## Quyền riêng tư và bảo mật

- **Không có dữ liệu đo từ xa,** không phân tích, không gửi báo cáo sự cố đi bất cứ đâu. Lỗi được ghi vào nhật ký hệ thống (Console.app, dưới tên của ứng dụng).
- **Chỉ dùng mạng cho những gì bạn khởi động:** các tác nhân bạn dùng, mỗi tác nhân giao tiếp với dịch vụ riêng của nó; sổ đăng ký ACP và việc tải xuống tác nhân, máy chủ ngôn ngữ, bộ chuyển đổi gỡ lỗi và mô hình đọc chính tả khi bạn yêu cầu; các máy chủ CI bạn đăng nhập; dự đoán chỉnh sửa từ Copilot nếu bạn đã chọn; các nhà cung cấp của tác nhân tích hợp; chép lời trực tuyến nếu bạn đã chọn; các trang bạn mở trong tab trình duyệt; và việc kiểm tra bản cập nhật khi bạn đã cho phép.
- **Mã rời khỏi máy Mac** chỉ đến nơi bạn đã chọn một dịch vụ cho việc đó: một tác nhân, dự đoán của Copilot, một thông điệp commit do AI viết. Diff cho thông điệp commit chỉ được gửi đi sau khi gitleaks không tìm thấy thông tin bí mật nào trong đó.
- **Khóa và token** được giữ trong Chuỗi khóa của macOS, không bao giờ nằm trong tệp cài đặt. Khóa của các ứng dụng khác, chẳng hạn khóa của Lowspeak, chỉ được đọc khi bạn đồng ý, và chỉ một lần.
- **Bản tải xuống được ghim phiên bản.** Các tác nhân, máy chủ ngôn ngữ và bộ chuyển đổi gỡ lỗi Dango cài được ghim vào các phiên bản đã kiểm thử và được đối chiếu với tổng kiểm tra của chúng.
- **Ký mã** với Hardened Runtime; bản cập nhật chỉ được cài khi có chữ ký riêng của ứng dụng.

## Nơi lưu dữ liệu

`<id>` là mã định danh gói, `local.agentide` cho bản phát hành (`local.agentide.debug` cho bản build gỡ lỗi). Mã này không chứa tên hiển thị, nên việc đổi tên vẫn giữ toàn bộ dữ liệu.

| Vị trí | Nội dung |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (danh sách của khung AI), `agent-threads/` (cuộc hội thoại của tác nhân tích hợp), `layouts/` (các tab theo từng dự án), `recovery/` (trình soạn thảo chưa lưu sau sự cố), `checkpoints/` (cho thư mục không dùng Git), `worktrees/`, `agents/` và `tools/` (tác nhân, máy chủ ngôn ngữ và bộ chuyển đổi gỡ lỗi đã cài), `task-output/` |
| `~/Library/Caches/<id>/` | hình ảnh đã dán, tập lệnh tích hợp shell, sổ đăng ký ACP; có thể xóa |
| `~/Library/Logs/<id>/dictation/` | tệp nhật ký của tính năng đọc chính tả |
| `~/Library/Application Support/Lowspeak` và `FluidAudio` | mô hình đọc chính tả cục bộ, dùng chung với Lowspeak |
| `~/Library/WebKit/<id>/` | cookie và dữ liệu trang web của tab trình duyệt |
| `<project>/.dango/settings.json` | cài đặt riêng của dự án |
| kho lưu trữ Git của dự án | điểm kiểm tra, dưới dạng commit trong `refs/ide-checkpoints/`; một lần push hoặc fetch thông thường không bao giờ mang chúng theo |
| Chuỗi khóa, dịch vụ `<id>` | khóa API của tác nhân tích hợp và của tính năng đọc chính tả, token CI |
| `~/.local/bin/dango` | lệnh terminal, nếu bạn đã cài |

Claude giữ các phiên của riêng nó trong `~/.claude/projects`; Dango đọc chúng để tiếp tục một phiên nhưng không thay đổi chúng.

## Giới hạn đã biết

- **Không có App Sandbox** cho chính ứng dụng, vì terminal, Git, máy chủ ngôn ngữ và tác nhân sẽ thừa hưởng nó; bạn phải tin cậy một thư mục trước khi bất cứ thứ gì nó cấu hình được chạy. Antigravity và OpenCode không có sandbox mà Dango có thể bật.
- **Số liệu sử dụng và phân tích ngữ cảnh của Claude** dựa vào các phần trong giao thức của nó được đánh dấu là thử nghiệm và bị ẩn khi các phần đó không có.
- **Tua lại cuộc hội thoại** chỉ hoạt động với Claude; với các tác nhân khác chỉ có mã được tua lại.
- **Đăng nhập CI qua trình duyệt** đang tắt; hãy dùng `gh`, `glab`, token hoặc môi trường.
- **Gemini CLI không được cung cấp:** Google đã ngừng đăng nhập bằng tài khoản cá nhân và hướng người dùng sang Antigravity. Các luồng Gemini trước đó vẫn đọc được.

## Biên dịch từ mã nguồn

Bạn cần:

- macOS 14 trở lên với Xcode Command Line Tools (`xcode-select --install`); không cần chính Xcode.
- Rust theo phiên bản mà `rust-toolchain.toml` nêu. `rust` của Homebrew biên dịch cho bộ xử lý của máy Mac này; bản build universal của một bản phát hành cần rustup với các target `aarch64-apple-darwin` và `x86_64-apple-darwin`.
- Bản checkout của [splat](https://github.com/sventmueller/splat) và [lowspeak](https://github.com/sventmueller/lowspeak) nằm cạnh bản này (`../splat`, `../lowspeak`), cả hai ở nhánh `main`; workspace lấy crate từ chúng theo đường dẫn.
- cmake, cho mọi bản build có đọc chính tả (`make run`, `make build`, `make release`), để biên dịch whisper.cpp. `make check` biên dịch mà không cần nó.

| Lệnh | Tác dụng |
|---|---|
| `make run` | biên dịch ứng dụng gỡ lỗi (`target/debug/Dango Debug.app`) và mở nó |
| `make build` | biên dịch ứng dụng phát hành (`target/release/Dango.app`), được ký bằng chứng chỉ cục bộ mà bản build đầu tiên tạo trong chuỗi khóa đăng nhập; tăng số build trong `VERSION` |
| `make lint` | định dạng và clippy; cảnh báo sẽ làm lệnh thất bại |
| `make check` | `lint`, sau đó toàn bộ kiểm thử; không cần chuỗi khóa, cửa sổ hay mạng |
| `make test` | `check`, sau đó tự kiểm thử `probe` và kiểm tra trợ năng, từ trái sang phải và từ phải sang trái, trong ứng dụng thật |
| `make perf` | ngân sách thời gian, trong bản build phát hành; chỉ có ý nghĩa trên một máy Mac không làm việc gì khác |
| `make release` | chạy thử bản phát hành vào `target/release-dry-run/`, được ký bằng chứng chỉ cục bộ; không gửi gì đi đâu cả |
| `make release-notarized` | `check`, sau đó là bản phát hành thật vào `dist/`: chữ ký Developer ID, công chứng, nguồn cập nhật được ký bằng khóa riêng của ứng dụng; tăng số build |

Bản gỡ lỗi và bản phát hành là hai ứng dụng riêng biệt với cài đặt và dữ liệu riêng. Việc biên dịch, kiểm thử, tự kiểm thử, phát hành và các quy tắc mà mã tuân theo được mô tả trong [docs/development.md](../../../docs/development.md), còn các quyết định đằng sau chúng nằm trong [docs/plan.md](../../../docs/plan.md). Ghi chú phát hành là [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), được dịch bên cạnh mỗi README trong `assets/readme/<language>/`.

## Giấy phép

Dango được cấp phép theo một trong hai giấy phép sau

- Giấy phép Apache, Phiên bản 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Giấy phép MIT ([LICENSE-MIT](../../../LICENSE-MIT))

tùy bạn chọn.

Trừ khi bạn tuyên bố rõ ràng khác đi, mọi đóng góp được bạn cố ý gửi để đưa vào Dango, theo định nghĩa trong giấy phép Apache-2.0, sẽ được cấp phép kép như trên, không có thêm điều khoản hay điều kiện nào.
