## Bảo mật
- Liên kết trong câu trả lời, kế hoạch và báo cáo của tác nhân, trong bản xem trước Markdown và trong PDF không còn mở
  bất kỳ ứng dụng nào chỉ bằng một lần bấm. Địa chỉ web mở trong tab trình duyệt, tệp trong dự án mở trong tab trình
  soạn thảo; mọi thứ khác, chẳng hạn một tệp ở nơi khác, một mục chia sẻ trên mạng hoặc liên kết của ứng dụng khác, sẽ
  hỏi trước và cho biết nó dẫn đến đâu.
- Địa chỉ đăng nhập và Remote Control mà tác nhân hoặc máy chủ MCP chuyển đến chỉ mở khi đó là địa chỉ web.
- Các thư mục chứa nhiều thứ hơn hẳn một dự án, chẳng hạn thư mục người dùng, Màn hình nền, Tài liệu, Tải về hoặc thư
  mục gốc của một ổ đĩa, không còn có thể được tin cậy toàn bộ; hãy tin cậy từng dự án một. Câu hỏi về sự tin cậy giờ
  cho biết sự tin cậy cũng áp dụng cho các thư mục bên trong.
- Token CI và thông tin đăng nhập không bao giờ được gửi qua http thường đến một máy chủ khác ngoài máy Mac này; kho lưu
  trữ có remote bắt đầu bằng `http://` sẽ được truy vấn qua https.
- Khi không có gitleaks, dự đoán khi chỉnh sửa sẽ chờ trước khi mã được gửi đến nhà cung cấp bên ngoài máy Mac này, cho
  đến khi bạn chọn gửi mà không kiểm tra. Cài đặt gitleaks riêng của dự án không còn tắt được việc kiểm tra.
- Tác nhân tích hợp không còn có thể bị dẫn đến việc ghi ra ngoài dự án qua một liên kết xuất hiện sau khi thao tác ghi
  của nó đã được phê duyệt, và các tệp có thêm liên kết cứng sẽ nhận một tệp riêng thay vì bị ghi xuyên qua.
- Tính năng hoàn thành mã của máy chủ ngôn ngữ không còn có thể làm treo trình soạn thảo, và việc đổi tên hay xóa một
  tệp liên kết tác động lên chính liên kết, không phải lên thứ mà liên kết trỏ tới.
- Hình ảnh bạn đính kèm vào prompt được giải mã bởi dịch vụ giải mã chạy trong sandbox, như ở mọi nơi khác.
- Một chương trình trong terminal chỉ có thể thay thế bảng nhớ tạm khi terminal của nó đang ở phía trước, và terminal sẽ
  thông báo điều đó.
- rust-analyzer được cài đặt từ một bản phát hành được ghim cùng tổng kiểm tra của nó thay vì bản nào mới nhất lúc đó.

## Tìm kiếm và điều hướng
- “Mở nhanh” cho biết khi không có tệp nào phù hợp và hiển thị nó đang tìm trong tệp hay trong ký hiệu của dự án.
- Ký hiệu của dự án bao gồm cả TypeScript và JavaScript mà không cần mở tệp nào của chúng. Kết quả khớp chính xác từ dự
  án và trong ngôn ngữ bạn đang làm việc được xếp trước; ký hiệu từ thư viện và gói đã cài đặt được đánh dấu.
- Xem › Tìm kiếm đưa tiêu điểm vào trường tìm kiếm.

## Trình soạn thảo
- Trình soạn thảo giữ tiêu điểm khi một tệp Markdown mở kèm bản xem trước bên cạnh, nên menu Định dạng hoạt động ngay.
  Các lệnh của trình soạn thảo cũng hoạt động khi tiêu điểm nằm ở thanh bên.
- Đề xuất ở cuối dòng hoặc trong một hộp bên dưới dòng cho biết phím Tab sẽ nhận đề xuất đó.
- Các tab cùng tên trong tổng quan tab giờ được phân biệt, chẳng hạn một tệp và bản xem trước của nó trong trình duyệt.
- Menu Xem không còn cung cấp mục “Ẩn thanh tab” của chính macOS, vốn không được các tab của Dango sử dụng.

## Ngôn ngữ và gỡ lỗi
- Máy chủ JSON giờ kiểm tra tệp, và các máy chủ đăng ký chẩn đoán sau khi khởi động, chẳng hạn basedpyright và
  sourcekit-lsp, sẽ hiển thị chẩn đoán đó.
- Ở những nơi pip từ chối cài đặt vào Python, như Python của Homebrew, trình gỡ lỗi đề nghị cài đặt debugpy vào một môi
  trường ảo của dự án.
