<!-- DRAFT for the owner's review. Check each point against the release build, adjust what differs, then delete this
comment; `cargo xtask release --notarize` refuses notes that still carry it. Keep this file in step with
RELEASE_NOTES.md. -->
Ini adalah rilis pertama Dango, editor macOS native untuk bekerja dengan agen coding. Editor, terminal, browser, Git,
CI, debugger, dan thread AI berbagi satu jendela per folder. App ini ditulis dengan Rust di atas AppKit; WebKit hanya
muncul di tab browser. Antarmukanya tersedia dalam 35 bahasa.

## Persyaratan
- macOS 14 atau lebih baru, di Apple silicon atau Intel.
- Untuk thread AI, agen yang Anda pilih: Claude memerlukan perintah `claude` yang sudah masuk; Codex memerlukan Node.js,
  dan app menginstal paketnya dengan versi tetap saat Anda memintanya; Antigravity dan OpenCode menggunakan `agy` atau
  `opencode` milik Anda sendiri, atau unduhan dengan versi tetap; agen bawaan memerlukan kunci API, atau Ollama maupun
  LM Studio di Mac ini.
- Server bahasa dan adaptor debug diinstal saat Anda memintanya. gopls dan `dlv` memerlukan Go; server TypeScript,
  Python, JSON, YAML, CSS, HTML, dan shell serta debugger JavaScript memerlukan Node.js; clangd, sourcekit-lsp, dan
  `lldb-dap` tersedia bersama Xcode Command Line Tools.
- Prediksi edit memerlukan langganan GitHub Copilot (yang gratis pun cukup), atau model fill-in-the-middle di Ollama
  atau LM Studio.

## Jendela
- Setiap file, terminal, halaman browser, thread AI, tampilan perbedaan, dan pratinjau adalah tab macOS di jendela
  folder tersebut. Tombol “+” di ujung bar tab dan ⌘T menawarkan file baru, terminal, browser, dan setiap agen yang
  terinstal, juga di worktree Git tersendiri.
- Bar samping beralih antara File, Pencarian, Agen, Git, Debug, dan CI (⌘1–⌘6); klik kanan menyembunyikan yang tidak
  Anda gunakan. Bar alat berisi kembali dan maju melalui tempat yang Anda edit (⌃- dan ⌃⇧-), branch, Buka Cepat, dan
  ikhtisar Aktivitas.
- “Buka di Samping” menampilkan dokumen kedua di samping yang pertama. Setelah mulai ulang atau crash, setiap jendela
  kembali dengan tab-tabnya, dan editor dengan teks yang belum disimpan kembali dengan teks tersebut.
- Gambar, audio, video, font, dan dokumen Office dibuka di tab melalui Quick Look, PDF melalui PDFKit.

## Editor
- Warna sintaks, nomor baris, tanda perubahan terhadap commit terakhir, temukan dan ganti, Buka Cepat (⌘P), dan
  pencarian proyek (⇧⌘F).
- Indentasi otomatis, tab atau spasi sesuai `.editorconfig`, pasangan kurung, mengaktifkan dan menonaktifkan komentar,
  pelipatan, buka baris, spasi kosong yang terlihat, dan pembungkusan baris.
- Beberapa kursor: ⌥-klik, ⌥-seret untuk kolom, ⌘D untuk kemunculan berikutnya.
- File mempertahankan enkode, tanda urutan byte, dan akhir barisnya. File yang berubah di disk dimuat ulang jika editor
  tidak memiliki teks yang belum disimpan; jika ada, sebuah bar menawarkan perbedaan dan penggabungan.
- **Prediksi edit:** saat Anda mengetik, saran muncul dalam warna abu-abu; Tab menerimanya. Saran berasal dari GitHub
  Copilot atau dari model lokal di Ollama atau LM Studio, sesuai pilihan Anda di pengaturan; model lokal menjaga kode
  Anda tetap di Mac ini.

## Server bahasa
Go, Rust, TypeScript dan JavaScript, Python, C, C++ dan Objective-C, Swift, skrip shell, JSON, YAML, CSS, HTML, TOML,
Markdown, dan Lua, serta server Anda sendiri dari pengaturan. Diagnostik, info saat penunjuk diarahkan, pelengkapan,
bantuan tanda tangan, buka definisi, definisi tipe, dan implementasi, referensi, hierarki panggilan dan hierarki tipe,
ganti nama, pemformatan, warna semantik, pelipatan, dan kerangka; tipe dan nama parameter di dalam baris; perbaikan
cepat dan refaktor di balik bola lampu (⌥⌘A); simbol proyek dengan ⌥⌘O atau `#` di Buka Cepat. `tsc` milik proyek
TypeScript sendiri digunakan jika dapat berfungsi sebagai server bahasa.

## Debugger
Breakpoint, juga dengan kondisi, eksekusi langkah demi langkah, thread dengan tumpukan panggilannya, variabel dan
ekspresi pantauan di Inspektur (⌥⌘I), nilai di samping kode selama program dijeda, dan konsol debug yang mengevaluasi
ekspresi di frame yang dijeda. Rust, C, C++, dan Swift melalui `lldb-dap`, Go melalui `dlv`, JavaScript dan TypeScript
melalui debugger JavaScript, Python melalui debugpy. `.vscode/launch.json` milik proyek dibaca. Menu Debug menggunakan
pintasan Xcode.

## Terminal
Terminal dalam tab dengan lingkungan shell login, digambar dengan Metal. Menempel atau melepas gambar akan
menyerahkannya ke agen coding yang berjalan di terminal, atau menempelkan jalur salinan yang disimpan. ⌘-klik membuka
tautan dan nama file. Server pengembangan yang dimulai dari terminal dapat dibuka di tab browser.

## Markdown
File Markdown dibuka dengan pratinjaunya di samping (⇧⌘V), lengkap dengan rumus, diagram Mermaid, Graphviz, dan
PlantUML, serta gambar dari folder. Menu Format untuk tebal, miring, tautan, judul, daftar, kutipan, dan blok kode; teks
yang ditempel dari Word atau halaman web masuk sebagai Markdown. **File › Ekspor** menulis PDF, satu file HTML, atau
dokumen Word. Jika Splat terinstal, **File › Buka di Splat** membuka dokumen di sana.

## Browser
Tab browser dengan bidang alamat, kembali dan maju, pencarian, unduhan dengan pertanyaan terlebih dahulu, dan Web
Inspector.

## Git
Panel Git dengan stage dan pembuangan perubahan file atau hunk tunggal, commit, commit lalu push atau sinkronisasi,
branch, stash, konflik penggabungan, dan grafik commit terbaru. Blame menunjukkan siapa yang terakhir mengubah baris di
bawah kursor, atau kolom untuk seluruh file. Sebuah tombol menulis pesan commit dengan AI; perbedaan baru dikirim
setelah gitleaks tidak menemukan rahasia di dalamnya, dan setiap commit juga diperiksa dari rahasia. Thread AI dapat
bekerja di worktree Git tersendiri; panel menunjukkan apa yang diubahnya dan mengambil hasil kerjanya melalui
penggabungan atau cherry-pick.

## CI
GitHub Actions dan GitLab CI, juga GitHub Enterprise Server dan GitLab yang dikelola sendiri: run, job, dan log dari
branch saat ini; menjalankan ulang job yang gagal, satu job, atau (di GitHub) seluruh run, memulai job manual atau
menyetujui deployment yang menunggu, memulai pipeline baru atau workflow yang boleh dimulai secara manual, dan
membatalkan. Merge request atau pull request yang terbuka serta issue tercantum di panel yang sama dan dibuka di tab
browser. Log job yang gagal dapat diberikan ke agen, ditandai sebagai teks dari luar. Token berasal dari Rantai Kunci,
`gh` atau `glab`, lingkungan shell login, atau token yang Anda masukkan.

## AI
Setiap agen bekerja di tab AI dengan linimasa yang sama: jawaban, panggilan alat beserta output-nya, permintaan izin,
pertanyaan, rencana untuk disetujui dan dikomentari, daftar tugas, subagen, dan tugas latar belakang; dua tombol di
kepala tab mencantumkan subagen dan proses latar belakang yang sedang berjalan. Sebelum setiap prompt, app menyimpan
titik pemulihan; perubahan dapat ditinjau dan diputar balik per pesan. Thread baru dimulai dalam mode “Manual”, dan
perintah Git yang merusak tidak pernah diizinkan secara permanen. Ikhtisar Aktivitas mencantumkan semua yang berjalan.
Titik hijau di tab menunjukkan sesi yang sedang bekerja, titik merah menunjukkan sesi yang menunggu Anda, dan notifikasi
memberi tahu saat sesi selesai atau menunggu ketika Anda sedang melihat di tempat lain.
- **Claude** melalui perintah `claude` tanpa diubah, dengan sesi, subagen, tugas latar belakang, konteks, dan
  penggunaan.
- **Agen ACP:** Codex, Antigravity, dan OpenCode, serta lainnya dari registri ACP saat Anda memintanya; agen yang
  diinstal dikunci ke versi yang telah diuji dan diperiksa terhadap checksum-nya.
- **Agen bawaan:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio, dan server yang menggunakan API
  OpenAI atau Anthropic, dengan alat untuk file, pencarian, perintah, diagnostik, dan permintaan web; kunci tetap di
  Rantai Kunci.
- Sakelar sandbox per thread membatasi apa yang boleh dilakukan perintah agen, sejauh agen mendukungnya.
- Teks dari luar, seperti output terminal, halaman web, atau log CI, dikirim ke agen dengan tanda tidak tepercaya.

## Dikte
Dikte ke bidang pesan AI dan ke editor melalui pengenalan ucapan Lowspeak: tombol mikrofon, ⌃⌥D, atau tombol fn selama
Dango adalah app yang aktif. Dikte menggunakan pengaturan dan model Lowspeak; transkripsi online memerlukan kunci OpenAI
atau Groq.

## Pembaruan
App memperbarui dirinya melalui Sparkle. Pada peluncuran kedua, app bertanya apakah boleh memeriksa pembaruan sendiri;
sampai Anda setuju, app tidak mengirim permintaan apa pun. **Dango › Periksa Pembaruan…** langsung memeriksa.

## Aksesibilitas
Nama VoiceOver untuk bar samping, tab, kartu di linimasa AI, baris Git dan CI, debugger, serta teks terminal; kontrol
penuh dengan papan ketik menggunakan pintasan yang dapat Anda ubah di `keybindings.json`; Tingkatkan Kontras dan Kurangi
Gerakan; tampilan kode tetap dari kiri ke kanan di antarmuka kanan ke kiri. Detailnya ada di ACCESSIBILITY.md.

## Privasi
Tanpa telemetri, tanpa analitik, dan tanpa laporan crash. App hanya terhubung untuk hal yang Anda mulai: agen yang Anda
gunakan, unduhan yang Anda minta, host CI tempat Anda masuk, prediksi dari Copilot jika Anda memilihnya, transkripsi
online jika Anda memilihnya, halaman yang Anda buka, dan pemeriksaan pembaruan setelah Anda mengizinkannya. Kunci dan
token tetap di Rantai Kunci macOS.

## Batasan yang diketahui
- Masuk ke CI melalui browser dinonaktifkan; gunakan `gh`, `glab`, token, atau lingkungan.
- App berjalan tanpa App Sandbox, karena terminal, Git, server bahasa, dan agen akan mewarisinya; folder harus
  dipercayai sebelum apa pun yang dikonfigurasinya dijalankan. Antigravity dan OpenCode tidak memiliki sandbox yang
  dapat dinyalakan oleh app.
- Angka penggunaan dan rincian konteks Claude bergantung pada bagian protokolnya yang ditandai eksperimental, dan
  disembunyikan jika bagian tersebut tidak ada. Memutar balik percakapan hanya berfungsi untuk Claude; untuk agen lain,
  hanya kode yang diputar balik.
- Gemini CLI tidak ditawarkan: Google mengakhiri proses masuknya dengan akun pribadi dan mengarahkan ke Antigravity.
  Thread Gemini sebelumnya tetap dapat dibaca.
