## Tốc độ và bộ nhớ
- Mọi thứ mà một tác nhân khởi chạy, bằng bất kỳ ngôn ngữ nào, chẳng hạn bản dựng, kiểm thử, shell và máy chủ MCP, đều
  chạy ở trạng thái nền của macOS: với mức ưu tiên thấp nhất, cùng quyền truy cập ổ đĩa và mạng bị điều tiết. Bản thân
  các tác nhân, cùng với các tác nhân phụ của Claude, chạy ở mức ưu tiên trung bình. Các tác nhân khi chạy hết công suất
  không còn làm Dango chậm đi; điều này cũng áp dụng cho các tác nhân của Software Factory.
- Các cửa sổ được khôi phục chỉ khởi chạy những gì đang hiển thị: một tab AI nằm sau các tab khác chỉ khởi chạy CLI của
  nó khi được đưa lên phía trước hoặc nhận được prompt, còn các tab trình duyệt, xem trước trang web và terminal chỉ tải
  trang hoặc khởi chạy shell của chúng khi hiển thị lần đầu.
- Một lần tìm kiếm dài trong dự án không còn vẽ lại toàn bộ kết quả trong khi đang chạy. Software Factory, nhật ký CI và
  khung Kiểm thử làm ít việc hơn khi không hiển thị; kiểm thử và cấu hình gỡ lỗi được đọc khi khung của chúng hiển thị
  lần đầu, còn các khung CI và Container bị ẩn không đọc gì cho đến khi hiển thị.
- Các phiên dài cần ít bộ nhớ hơn: một khác biệt đã hoàn tất chỉ giữ lại những dòng mà thẻ của nó hiển thị, một yêu cầu
  cấp quyền đã được trả lời sẽ giải phóng dữ liệu đầu vào của nó, bộ đệm glyph của terminal không còn tăng không giới
  hạn, và các khung Tệp và Git giải phóng những hàng không còn tồn tại.
- Bản thân chương trình nhỏ hơn khoảng 40%.

## Cửa sổ và menu
- Đóng một ngăn sẽ chuyển các tab của nó sang ngăn phía trước, nếu không có thì sang ngăn phía sau, thay vì đóng chúng.
  Chỉ ngăn của bản xem trước và ngăn cuối cùng mới đóng cùng các tab của chúng.
- Menu Xem liệt kê các khung theo thứ tự các biểu tượng trên thanh bên.
- “Quản lý tác nhân…” đã được bỏ khỏi menu +; mục này vẫn nằm dưới “+ Phiên mới” trong khung Tác nhân.

## Sửa lỗi
- “Bao gồm và loại trừ tệp” trong khung Tìm kiếm không còn hiển thị một chữ cái thừa bên cạnh mũi tên của nó.
- Đóng cửa sổ bật lên nhận xét của một đặc tả không còn khiến ứng dụng bị thoát đột ngột.
