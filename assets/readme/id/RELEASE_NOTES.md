## Kecepatan dan memori
- Semua yang dijalankan agen, dalam bahasa apa pun, seperti build, tes, shell, dan server MCP, berjalan dalam status
  latar belakang macOS: dengan prioritas terendah, serta akses disk dan jaringan yang dibatasi. Agen itu sendiri,
  beserta subagen Claude, berjalan dengan prioritas menengah. Agen yang bekerja dengan beban penuh tidak lagi
  memperlambat Dango; hal ini juga berlaku untuk agen Software Factory.
- Jendela yang dipulihkan hanya menjalankan apa yang terlihat: tab AI yang berada di belakang tab lain menjalankan
  CLI-nya saat dibawa ke depan atau menerima prompt, dan tab browser, pratinjau situs, serta terminal memuat halamannya
  atau menjalankan shell-nya saat pertama kali ditampilkan.
- Pencarian proyek yang panjang tidak lagi menggambar ulang semua hasilnya selama berjalan. Software Factory, log CI,
  dan panel Tes bekerja lebih sedikit selama tidak terlihat; tes dan konfigurasi debug dibaca saat panelnya pertama kali
  ditampilkan, dan panel CI serta Container yang tersembunyi tidak membaca apa pun hingga ditampilkan.
- Sesi yang panjang membutuhkan lebih sedikit memori: kartu perbedaan yang sudah selesai hanya menyimpan baris yang
  ditampilkannya, permintaan izin yang sudah dijawab melepaskan input-nya, cache glif terminal tidak lagi tumbuh tanpa
  batas, dan panel File dan Git melepaskan baris yang sudah tidak ada.
- Program itu sendiri sekitar 40% lebih kecil.

## Jendela dan menu
- Menutup sebuah area kini memindahkan tab-tabnya ke area sebelumnya, atau jika tidak ada, ke area sesudahnya, alih-alih
  menutupnya. Hanya area pratinjau dan area terakhir yang tertutup bersama tab-tabnya.
- Menu Lihat mencantumkan panel sesuai urutan simbol di bar samping.
- “Kelola Agen…” tidak lagi ada di menu +; item ini tetap ada di bawah “+ Sesi Baru” di panel Agen.

## Perbaikan
- “Sertakan dan Kecualikan File” di panel Pencarian tidak lagi menampilkan huruf nyasar di samping panahnya.
- Menutup popover komentar pada spesifikasi tidak lagi membuat aplikasi mogok.
