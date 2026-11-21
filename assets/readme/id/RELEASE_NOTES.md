## Keamanan
- Tautan dalam jawaban, rencana, dan laporan agen, dalam pratinjau Markdown, dan dalam PDF tidak lagi membuka aplikasi
  apa pun dengan satu klik. Alamat web terbuka di tab browser, file di dalam proyek di tab editor; selain itu, seperti
  file di tempat lain, berbagi jaringan, atau tautan aplikasi lain, akan bertanya terlebih dahulu dan menunjukkan ke
  mana tautan itu mengarah.
- Alamat masuk dan Remote Control yang diserahkan oleh agen atau server MCP hanya terbuka jika berupa alamat web.
- Folder yang berisi jauh lebih banyak daripada satu proyek, seperti folder pengguna, Desktop, Dokumen, Unduhan, atau
  akar volume, tidak lagi dapat dipercayai secara keseluruhan; percayai proyek di dalamnya satu per satu. Pertanyaan
  kepercayaan kini menyebutkan bahwa kepercayaan itu juga berlaku untuk folder di dalamnya.
- Token CI dan data masuk tidak pernah dikirim melalui http biasa ke server selain Mac ini; repositori yang remote-nya
  diawali `http://` ditanyai melalui https.
- Tanpa gitleaks, prediksi edit menunggu sebelum kode dikirim ke penyedia di luar Mac ini hingga Anda memilih untuk
  mengirimnya tanpa diperiksa. Pengaturan gitleaks milik proyek tidak lagi mematikan pemeriksaan.
- Agen bawaan tidak lagi dapat diarahkan untuk menulis di luar proyek melalui tautan yang muncul setelah penulisannya
  disetujui, dan file yang memiliki hard link lain mendapatkan file sendiri alih-alih ditulis melalui tautan itu.
- Penyelesaian kode dari server bahasa tidak lagi dapat membuat editor macet, dan penggantian nama serta penghapusan
  file bertautan olehnya berlaku pada tautannya, bukan pada apa yang ditunjuknya.
- Gambar yang Anda lampirkan ke prompt didekode oleh layanan dekode yang berjalan di sandbox, seperti di tempat lain.
- Program di terminal hanya dapat mengganti papan klip selama terminalnya berada di depan, dan terminal
  memberitahukannya.
- rust-analyzer diinstal dari rilis yang dipatok dengan checksum-nya, bukan dari versi terbaru apa pun saat itu.

## Pencarian dan navigasi
- Buka Cepat memberi tahu jika tidak ada file yang cocok dan menunjukkan apakah ia mencari file atau simbol proyek.
- Simbol proyek mencakup TypeScript dan JavaScript tanpa perlu membuka salah satu filenya. Kecocokan persis dari proyek
  dan dalam bahasa yang sedang Anda gunakan muncul lebih dulu; simbol dari pustaka dan paket yang terinstal ditandai.
- Lihat › Pencarian menempatkan fokus di kolom pencarian.

## Editor
- Editor tetap memegang fokus saat file Markdown terbuka dengan pratinjau di sampingnya, sehingga menu Format langsung
  berfungsi. Perintah editor juga berfungsi selama fokus berada di bar samping.
- Saran di akhir baris atau di kotak di bawahnya memberi tahu bahwa Tab menerimanya.
- Tab yang bernama sama dalam ikhtisar tab kini dapat dibedakan, misalnya sebuah file dan pratinjaunya di browser.
- Menu Lihat tidak lagi menawarkan “Sembunyikan Bar Tab” milik macOS, yang tidak digunakan oleh tab Dango.

## Bahasa dan debugging
- Server JSON kini memeriksa file, dan server yang mendaftarkan diagnostiknya setelah dimulai, seperti basedpyright
  dan sourcekit-lsp, menampilkannya.
- Jika pip menolak menginstal ke Python, seperti Python dari Homebrew, debugger menawarkan untuk menginstal debugpy ke
  lingkungan virtual proyek.
