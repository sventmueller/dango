## Bộ nhớ
- Các phiên tác nhân dài cần ít bộ nhớ hơn nhiều:
  - Khi mở một phiên trước đây, bản ghi của phiên được đọc từng dòng một; một phiên có bản ghi 100 MB giờ chỉ dùng tối
    đa khoảng 20 MB thay vì 65 MB.
  - Một lệnh gọi công cụ đã hoàn tất chỉ giữ những gì thẻ của nó hiển thị: 400 dòng đầu tiên của đầu ra, và không còn
    giữ đầu vào thô khi thẻ đã hiển thị các trường của nó. “N dòng nữa” vẫn đếm mọi dòng.
  - Một tab AI nằm ngoài tầm nhìn trong 30 giây sẽ giải phóng các thẻ của nó, và khi hiển thị lại sẽ dựng các thẻ nằm
    trong tầm nhìn, tại vị trí cũ.
  - Sau khi đóng một tab hoặc mở một phiên trước đây, Dango trả lại cho macOS bộ nhớ mà nó đã giải phóng.
- Danh sách không còn phình to trong bộ nhớ khi làm mới. Trên macOS 27, mỗi lần làm mới một danh sách có các hàng mở ra
  được lại để lại vĩnh viễn một tam giác mở rộng, và mỗi lần làm mới để lại một chút của mỗi hàng:
  - Các danh sách CI, Container, Software Factory, Kiểm thử, Gỡ lỗi, Git và Tìm kiếm cùng phân cấp lệnh gọi tự vẽ mũi
    tên của mình.
  - Danh sách nằm ngoài tầm nhìn không còn làm mới; chúng cập nhật bù khi hiển thị.
  - Các danh sách Phiên, CI, Software Factory và Container giữ lại các hàng không thay đổi, và Git chỉ làm mới các hàng
    của những tệp có trạng thái thay đổi.
- Các khung của một cửa sổ đã đóng được giải phóng cùng với cửa sổ đó.

## Tab AI
- “Hiển thị đầu ra” của một lệnh nền tự ghi đầu ra của nó vào tệp, chẳng hạn `make > build.log 2>&1`, sẽ hiển thị tệp đó
  khi CLI không có chút đầu ra nào.
