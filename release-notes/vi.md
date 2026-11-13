<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Đây là bản phát hành đầu tiên của Dango, một trình soạn thảo gốc cho macOS để làm việc với các tác nhân lập trình. Trình
soạn thảo, terminal, trình duyệt, Git, CI, trình gỡ lỗi và các luồng AI dùng chung một cửa sổ cho mỗi thư mục. Ứng dụng
được viết bằng Rust trên AppKit; WebKit chỉ xuất hiện trong tab trình duyệt. Giao diện có sẵn bằng 35 ngôn ngữ.

## Yêu cầu
- macOS 14 trở lên, trên máy Mac dùng chip Apple hoặc Intel.
- Với các luồng AI, cần tác nhân bạn chọn: Claude cần lệnh `claude` đã đăng nhập; Codex cần Node.js, và ứng dụng sẽ cài
  gói đã ghim phiên bản của nó khi bạn yêu cầu; Antigravity và OpenCode dùng `agy` hoặc `opencode` của riêng bạn, hoặc
  một bản tải xuống đã ghim phiên bản; tác nhân tích hợp cần khóa API, hoặc Ollama hay LM Studio trên máy Mac này.
- Máy chủ ngôn ngữ và bộ chuyển đổi gỡ lỗi được cài khi bạn yêu cầu. gopls và `dlv` cần Go; các máy chủ cho TypeScript,
  Python, JSON, YAML, CSS, HTML và shell cùng trình gỡ lỗi JavaScript cần Node.js; clangd, sourcekit-lsp và `lldb-dap`
  đi kèm Xcode Command Line Tools.
- Dự đoán chỉnh sửa cần gói đăng ký GitHub Copilot (gói miễn phí là đủ) hoặc một mô hình fill-in-the-middle trong Ollama
  hay LM Studio.

## Cửa sổ
- Mỗi tệp, terminal, trang web, luồng AI, diff và bản xem trước là một tab macOS trong cửa sổ của thư mục. Nút “+” ở
  cuối thanh tab và ⌘T cung cấp tệp mới, terminal, trình duyệt và từng tác nhân đã cài, kể cả trong một Git worktree
  riêng.
- Thanh bên chuyển giữa Tệp, Tìm kiếm, Tác nhân, Git, Gỡ lỗi và CI (⌘1–⌘6); bấm chuột phải để ẩn những mục bạn không
  dùng. Thanh công cụ chứa nút lùi và tiến qua những chỗ bạn đã sửa (⌃- và ⌃⇧-), nhánh, Mở nhanh và tổng quan Hoạt động.
- “Mở ở Bên cạnh” hiển thị tài liệu thứ hai bên cạnh tài liệu đầu tiên. Sau khi khởi động lại hoặc gặp sự cố, mọi cửa sổ
  trở lại cùng các tab của nó, và các trình soạn thảo có văn bản chưa lưu sẽ trở lại cùng văn bản đó.
- Hình ảnh, âm thanh, video, phông chữ và tài liệu Office mở trong một tab qua Quick Look, PDF qua PDFKit.

## Trình soạn thảo
- Tô màu cú pháp, số dòng, dấu thay đổi so với commit gần nhất, tìm và thay thế, Mở nhanh (⌘P) và tìm kiếm trong dự án
  (⇧⌘F).
- Tự động thụt lề, tab hoặc dấu cách theo `.editorconfig`, cặp dấu ngoặc, bật/tắt chú thích, thu gọn khối mã, đi tới
  dòng, hiển thị khoảng trắng và ngắt dòng.
- Nhiều con trỏ: ⌥-bấm, ⌥-kéo để chọn một cột, ⌘D cho lần xuất hiện tiếp theo.
- Tệp giữ nguyên bảng mã, dấu thứ tự byte và ký tự xuống dòng. Tệp bị thay đổi trên ổ đĩa sẽ được tải lại khi trình soạn
  thảo không có văn bản chưa lưu; nếu không, một thanh thông báo sẽ đưa ra các khác biệt và tùy chọn hợp nhất.
- **Dự đoán chỉnh sửa:** khi bạn gõ, một đề xuất xuất hiện bằng chữ xám; Tab sẽ nhận đề xuất đó. Đề xuất đến từ GitHub
  Copilot hoặc từ một mô hình cục bộ trong Ollama hay LM Studio, tùy bạn chọn trong cài đặt; mô hình cục bộ giữ mã của
  bạn trên máy Mac này.

## Máy chủ ngôn ngữ
Go, Rust, TypeScript và JavaScript, Python, C, C++ và Objective-C, Swift, shell script, JSON, YAML, CSS, HTML, TOML,
Markdown và Lua, cùng các máy chủ riêng của bạn từ cài đặt. Chẩn đoán, thông tin khi di chuột, tự động hoàn thành, trợ
giúp chữ ký hàm, đi tới định nghĩa, định nghĩa kiểu và triển khai, tham chiếu, phân cấp lệnh gọi và phân cấp kiểu, đổi
tên, định dạng, màu ngữ nghĩa, thu gọn khối mã và dàn ý; kiểu và tên tham số ngay trong dòng; sửa nhanh và tái cấu trúc
sau biểu tượng bóng đèn (⌥⌘A); ký hiệu của dự án bằng ⌥⌘O hoặc `#` trong Mở nhanh. `tsc` riêng của một dự án TypeScript
được dùng khi nó có thể làm máy chủ ngôn ngữ.

## Gỡ lỗi
Điểm ngắt, kể cả có điều kiện, chạy từng bước, các luồng thực thi cùng ngăn xếp lệnh gọi của chúng, biến và biểu thức
theo dõi trong Bộ kiểm tra (⌥⌘I), giá trị bên cạnh mã khi chương trình đang tạm dừng, và bảng điều khiển gỡ lỗi để tính
biểu thức trong khung đang tạm dừng. Rust, C, C++ và Swift qua `lldb-dap`, Go qua `dlv`, JavaScript và TypeScript qua
trình gỡ lỗi JavaScript, Python qua debugpy. Tệp `.vscode/launch.json` của dự án được đọc. Menu Gỡ lỗi dùng các phím tắt
của Xcode.

## Terminal
Terminal trong các tab với môi trường của login shell, vẽ bằng Metal. Dán hoặc thả một hình ảnh sẽ chuyển nó cho tác
nhân lập trình đang chạy trong terminal, hoặc dán đường dẫn của một bản sao đã lưu. ⌘-bấm để mở liên kết và tên tệp. Máy
chủ phát triển do terminal khởi động có thể mở trong tab trình duyệt.

## Markdown
Tệp Markdown mở cùng bản xem trước bên cạnh (⇧⌘V), với công thức, sơ đồ Mermaid, Graphviz và PlantUML và hình ảnh trong
thư mục. Menu Định dạng cho in đậm, in nghiêng, liên kết, tiêu đề, danh sách, trích dẫn và khối mã; văn bản dán từ Word
hoặc trang web sẽ thành Markdown. **Tệp › Xuất** ghi ra PDF, một tệp HTML duy nhất hoặc tài liệu Word. Nếu đã cài Splat,
**Tệp › Mở trong Splat** sẽ mở tài liệu trong đó.

## Trình duyệt
Tab trình duyệt với trường địa chỉ, lùi và tiến, tìm kiếm, tải xuống có hỏi trước, và Trình kiểm tra web.

## Git
Khung Git để stage và hủy thay đổi của cả tệp hoặc từng khối thay đổi, commit, commit rồi push hoặc đồng bộ, nhánh,
stash, xung đột hợp nhất và biểu đồ các commit gần đây. Blame cho biết ai đã sửa dòng tại con trỏ lần cuối, hoặc hiển
thị một cột cho cả tệp. Một nút viết thông điệp commit bằng AI; diff chỉ được gửi đi sau khi gitleaks không tìm thấy
thông tin bí mật nào trong đó, và mọi commit cũng được kiểm tra thông tin bí mật. Một luồng AI có thể làm việc trong Git
worktree riêng; khung hiển thị những gì luồng đó đã thay đổi và tiếp nhận công việc bằng hợp nhất hoặc cherry-pick.

## CI
GitHub Actions và GitLab CI, kể cả GitHub Enterprise Server và GitLab tự quản lý: các lần chạy, job và nhật ký của nhánh
hiện tại; chạy lại job thất bại, một job riêng lẻ hoặc (trên GitHub) cả một lần chạy, khởi động job thủ công hoặc phê
duyệt các lần triển khai đang chờ, khởi động pipeline mới hoặc workflow có thể chạy thủ công, và hủy. Merge request hoặc
pull request đang mở và issue được liệt kê trong cùng khung và mở trong tab trình duyệt. Nhật ký của job thất bại có thể
được gửi cho tác nhân, kèm dấu là văn bản từ bên ngoài. Token lấy từ Chuỗi khóa, `gh` hoặc `glab`, môi trường của login
shell, hoặc do bạn nhập.

## AI
Mỗi tác nhân làm việc trong một tab AI với cùng một dòng thời gian: câu trả lời, lệnh gọi công cụ cùng đầu ra, yêu cầu
cấp quyền, câu hỏi, kế hoạch để phê duyệt và nhận xét, việc cần làm, tác nhân phụ và tác vụ nền; hai nút trong phần đầu
tab liệt kê các tác nhân phụ và tiến trình nền đang chạy. Trước mỗi prompt, ứng dụng lưu một điểm kiểm tra; các thay đổi
có thể được xem lại và tua lại theo từng tin nhắn. Luồng mới bắt đầu ở chế độ “Thủ công”, và các lệnh Git mang tính phá
hủy không bao giờ được cho phép vĩnh viễn. Tổng quan Hoạt động liệt kê mọi thứ đang chạy. Chấm xanh lá trên tab cho biết
phiên đang làm việc, chấm đỏ cho biết phiên đang chờ bạn, và một thông báo cho bạn biết khi phiên hoàn tất hoặc đang chờ
trong lúc bạn xem chỗ khác.
- **Claude** qua lệnh `claude` nguyên bản, với phiên, tác nhân phụ, tác vụ nền, ngữ cảnh và mức sử dụng.
- **Tác nhân ACP:** Codex, Antigravity và OpenCode, cùng các tác nhân khác từ sổ đăng ký ACP khi bạn yêu cầu; tác nhân
  đã cài được ghim vào các phiên bản đã kiểm thử và được đối chiếu với tổng kiểm tra của chúng.
- **Tác nhân tích hợp:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio và các máy chủ dùng API của
  OpenAI hoặc Anthropic, với công cụ cho tệp, tìm kiếm, lệnh, chẩn đoán và yêu cầu web; khóa được giữ trong Chuỗi khóa.
- Công tắc sandbox cho từng luồng giới hạn những gì lệnh của tác nhân được phép làm, trong phạm vi tác nhân hỗ trợ.
- Văn bản từ bên ngoài, như đầu ra terminal, trang web hoặc nhật ký CI, được gửi đến tác nhân kèm dấu không đáng tin
  cậy.

## Đọc chính tả
Đọc chính tả vào trường tin nhắn AI và trình soạn thảo qua tính năng nhận dạng giọng nói của Lowspeak: nút micrô, ⌃⌥D
hoặc phím fn khi Dango là ứng dụng đang hoạt động. Tính năng này dùng cài đặt và mô hình của Lowspeak; chép lời trực
tuyến cần khóa OpenAI hoặc Groq.

## Cập nhật
Ứng dụng tự cập nhật qua Sparkle. Ở lần mở thứ hai, ứng dụng hỏi xem có được tự kiểm tra bản cập nhật không; cho đến khi
bạn đồng ý, ứng dụng không gửi yêu cầu nào. **Dango › Kiểm tra Bản cập nhật…** kiểm tra ngay lập tức.

## Trợ năng
Tên VoiceOver cho thanh bên, tab, thẻ trong dòng thời gian AI, hàng Git và CI, trình gỡ lỗi và văn bản của terminal;
điều khiển hoàn toàn bằng bàn phím với các phím tắt bạn có thể đổi trong `keybindings.json`; Tăng độ tương phản và Giảm
chuyển động; khung xem mã vẫn hiển thị từ trái sang phải trong giao diện viết từ phải sang trái. Chi tiết có trong
ACCESSIBILITY.md.

## Quyền riêng tư
Không có dữ liệu đo từ xa, không phân tích và không báo cáo sự cố. Ứng dụng chỉ kết nối cho những gì bạn khởi động: các
tác nhân bạn dùng, các bản tải xuống bạn yêu cầu, các máy chủ CI bạn đăng nhập, dự đoán từ Copilot nếu bạn đã chọn, chép
lời trực tuyến nếu bạn đã chọn, các trang bạn mở và việc kiểm tra bản cập nhật khi bạn đã cho phép. Khóa và token được
giữ trong Chuỗi khóa của macOS.

## Giới hạn đã biết
- Đăng nhập CI qua trình duyệt đang tắt; hãy dùng `gh`, `glab`, token hoặc môi trường.
- Ứng dụng chạy không có App Sandbox, vì terminal, Git, máy chủ ngôn ngữ và tác nhân sẽ thừa hưởng nó; bạn phải tin cậy
  một thư mục trước khi bất cứ thứ gì nó cấu hình được chạy. Antigravity và OpenCode không có sandbox mà ứng dụng có thể
  bật.
- Số liệu sử dụng và phân tích ngữ cảnh của Claude dựa vào các phần trong giao thức được đánh dấu là thử nghiệm và bị ẩn
  khi các phần đó không có. Tua lại cuộc hội thoại chỉ hoạt động với Claude; với các tác nhân khác chỉ có mã được tua
  lại.
- Gemini CLI không được cung cấp: Google đã ngừng đăng nhập bằng tài khoản cá nhân và hướng người dùng sang Antigravity.
  Các luồng Gemini trước đó vẫn đọc được.
