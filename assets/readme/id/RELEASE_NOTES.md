## Memori
- Sesi agen yang panjang kini membutuhkan jauh lebih sedikit memori:
  - Membuka sesi lama membaca transkripnya baris demi baris; sesi dengan transkrip 100 MB kini paling banyak memakai
    sekitar 20 MB, bukan 65 MB.
  - Panggilan alat yang sudah selesai hanya menyimpan apa yang ditampilkan kartunya: 400 baris pertama output-nya, dan
    tidak lagi input mentahnya begitu kartu menampilkan kolom-kolomnya. “N baris lagi” tetap menghitung setiap baris.
  - Tab AI yang tidak terlihat selama 30 detik melepaskan kartunya, dan saat ditampilkan lagi, tab itu membangun kartu
    yang terlihat, di posisi terakhirnya.
  - Setelah tab ditutup atau sesi lama dibuka, Dango mengembalikan memori yang dibebaskannya ke macOS.
- Daftar tidak lagi membengkak di memori saat diperbarui. Di macOS 27, setiap pembaruan daftar dengan baris yang dapat
  dibuka meninggalkan segitiga pengungkapan secara permanen, dan setiap pembaruan meninggalkan sedikit sisa dari setiap
  baris:
  - Daftar CI, Container, Software Factory, Tes, Debug, Git, dan Pencarian serta hierarki panggilan menggambar panahnya
    sendiri.
  - Daftar yang tidak terlihat tidak lagi diperbarui; daftar itu menyusul saat ditampilkan.
  - Daftar Sesi, CI, Software Factory, dan Container mempertahankan baris yang tidak berubah, dan Git hanya memperbarui
    baris file yang statusnya berubah.
- Panel dari jendela yang ditutup ikut dilepaskan bersamanya.

## Tab AI
- “Tampilkan Output” untuk perintah latar belakang yang menulis output-nya sendiri ke file, seperti
  `make > build.log 2>&1`, menampilkan file tersebut saat CLI tidak memiliki output apa pun.
