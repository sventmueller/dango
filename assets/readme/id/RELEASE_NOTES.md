## Terminal
- Terminalnya kini milik Tapa, app terminal saudara Dango, dengan semua yang ditambahkannya, sementara Dango tetap mempertahankan apa yang dilakukan terminalnya sebelumnya: menempel gambar ke agen, server pengembangan di tab browser, ⌘-klik ke editor.
- **Lihat › Tampilkan/Sembunyikan Bidang Input** (⌘I saat terminal memiliki fokus) mengubah baris perintah menjadi bidang teks dengan folder dan branch Git di atasnya. Perintah diwarnai menurut bagian-bagiannya, dan perintah yang tidak dapat dijalankan shell Anda mendapat garis bawah merah putus-putus.
- Saran tampil abu-abu saat Anda mengetik, di bidang input maupun tanpanya: dari riwayat Anda, dari spesifikasi untuk lebih dari 700 alat baris perintah, dan dari proyek, seperti branch git, skrip `package.json`, serta target make, just, dan cargo. → menerima saran; Tab melengkapi, dan bila ada beberapa kandidat membuka menu yang tersaring saat Anda mengetik. Saran tidak pernah menggunakan jaringan.
- Blok perintah: ⌘↑ dan ⌘↓ melompat antar-prompt, menu **Edit** memilih, menyalin, dan menyimpan keluaran sebuah perintah, dan markah menandai baris. Saat **Tandai perintah sebagai blok** aktif, sebuah bilah menunjukkan setiap blok, perintah yang gagal ditandai, dan tombol di atas blok menampilkan status dan durasinya.
- **Kirim Blok ke Agen** (menu Edit dan menu konteks terminal) membuka tab AI dengan baris perintah dan keluaran blok saat ini terlampir sebagai Konten Eksternal.
- **Temukan** (⌘F saat terminal memiliki fokus) mencari di seluruh riwayat, dengan ekspresi reguler, huruf besar-kecil, dan kata utuh; bilah gulir menandai hasilnya.
- Notifikasi program (OSC 9 dan OSC 777) dan belnya memasang titik merah di tab terminal selama Anda tidak melihatnya, dan notifikasi dikirim ke macOS saat app lain berada di depan. Kemajuan yang dilaporkan program (OSC 9;4) tampil sebagai cincin di tabnya dan sebagai bilah di ikon Dock.
- Klik ganda memilih URL, jalur, atau hash secara utuh, seret dengan ⌥ memilih sebuah persegi panjang, **Salin dengan Format** mempertahankan warna dan gaya, dan **Pilih dengan Papan Ketik** menggerakkan pilihan dengan tombol Vim.
- Tempelan multibaris yang akan langsung berjalan tampil lebih dulu, dan **Dango › Entri Papan Ketik Aman** mencegah app lain membaca apa yang Anda ketik di terminal.
- Tautan jenis selain alamat web, `mailto:`, dan file tidak lagi dibuka dari terminal.

## Pengaturan
- **Pengaturan › Terminal** mengaktifkan dan menonaktifkan bidang input untuk terminal baru, tombol Vim-nya, saran, pewarnaan perintah, garis bawah untuk perintah yang tidak dikenal, blok perintah, penyalinan pilihan seketika, penampilan tempelan multibaris lebih dulu, dan ⌥ sebagai tombol Meta, yang dipindahkan ke sini dari Umum.
