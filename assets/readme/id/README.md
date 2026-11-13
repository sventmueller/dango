<p><img src="../../readme-icon.png" alt="Ikon Dango" width="168" height="168"></p>

# Dango

**Editor macOS asli untuk bekerja dengan agen pemrograman.**

Dango menyatukan kode, agen yang mengubahnya, dan semua hal di sekitarnya dalam satu jendela per folder: editor dengan server bahasa dan prediksi saat mengetik, terminal, browser, Git, CI, debugger, dan utas AI dengan Claude, Codex, Antigravity, OpenCode, atau agen yang dibangun ke dalam app. Setiap agen bekerja di lini masa yang sama, bertanya sebelum mengubah file atau menjalankan perintah, dan meninggalkan checkpoint sebelum setiap prompt, sehingga Anda dapat meninjau dan membatalkan apa yang dilakukannya, pesan demi pesan. Tidak ada yang dikonfigurasi oleh sebuah folder yang berjalan sebelum Anda memercayainya, dan teks dari luar sampai ke agen dengan tanda tidak tepercaya. Diktekan ke editor dan kolom pesan; pengenalan suara bisa tetap di Mac Anda. Dango ditulis dalam Rust di atas kontrol AppKit asli, tanpa Electron, berbicara dalam 35 bahasa, dan tidak memiliki telemetri.

**Belum dirilis.** Rilis pertama akan berupa image disk yang ditandatangani dengan Developer ID dan dinotarisasi oleh Apple, untuk macOS 14 atau lebih baru di Apple Silicon dan Intel. Hingga saat itu, [bangun Dango dari kode sumber](#membangun-dari-kode-sumber).

**Menemukan bug?** [Buka issue](https://github.com/sventmueller/dango/issues/new) dan sebutkan dengan agen apa dan di jenis proyek apa bug itu terjadi.

[![Rust edisi 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 atau lebih baru](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon dan Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit melalui objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 bahasa antarmuka](https://img.shields.io/badge/languages-35-4C9A2A)
![Tanpa telemetri](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Ditandatangani dengan Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lisensi: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lisensi)

**Bahasa Indonesia** ·
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
[Tiếng Việt](../vi/README.md) ·
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

## Daftar isi

- [Fitur sekilas](#fitur-sekilas)
- [Persyaratan](#persyaratan)
- [Instalasi](#instalasi)
- [Memulai](#memulai)
- [Agen AI](#agen-ai)
  - [Lini masa](#lini-masa)
  - [Izin dan mode](#izin-dan-mode)
  - [Checkpoint dan worktree](#checkpoint-dan-worktree)
- [Editor](#editor)
- [Server bahasa](#server-bahasa)
- [Tes](#tes)
- [Debugger](#debugger)
- [Terminal](#terminal)
- [Browser](#browser)
- [Git](#git)
- [CI](#ci)
- [Container](#container)
- [Markdown](#markdown)
- [Dikte](#dikte)
- [Kepercayaan](#kepercayaan)
- [Sandbox](#sandbox)
- [Tampilan dan aksesibilitas](#tampilan-dan-aksesibilitas)
- [Bahasa](#bahasa)
- [Pintasan papan ketik](#pintasan-papan-ketik)
- [Privasi dan keamanan](#privasi-dan-keamanan)
- [Lokasi data](#lokasi-data)
- [Batasan yang diketahui](#batasan-yang-diketahui)
- [Membangun dari kode sumber](#membangun-dari-kode-sumber)
- [Lisensi](#lisensi)

## Fitur sekilas

- **Semua agen dalam satu lini masa.** Claude melalui perintah `claude` miliknya sendiri, Codex, Antigravity, dan OpenCode melalui Agent Client Protocol, lebih banyak lagi dari registri ACP, serta agen yang dibangun ke dalam app untuk Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, dan LM Studio. Jawaban, panggilan alat, permintaan izin, rencana, daftar tugas, subagen, dan tugas latar belakang tampak sama, agen mana pun yang berjalan.
- **Tinjau dan batalkan.** Sebelum setiap prompt, Dango menyimpan checkpoint. Apa yang diubah agen dapat ditinjau dan dibatalkan per pesan, dan sebuah utas dapat bekerja di worktree Git miliknya sendiri lalu menyerahkan hasilnya melalui gabungkan atau cherry-pick.
- **Satu jendela per folder.** File, terminal, halaman web, utas AI, diff, dan pratinjau adalah tab macOS di jendela itu. Bar samping berisi File, Pencarian, Agen, Git, Tes, Debug, CI, dan Container.
- **Editor kode sungguhan.** Warna sintaksis, beberapa titik sisip, pelipatan, `.editorconfig`, server bahasa untuk 17 bahasa dengan pelengkapan, buka definisi, referensi, ganti nama, perbaikan cepat, dan hierarki, serta prediksi saat mengetik dari GitHub Copilot atau model lokal.
- **Debugger, Git, dan CI bawaan.** Breakpoint dan eksekusi langkah demi langkah untuk Rust, C, C++, Swift, Go, JavaScript, TypeScript, dan Python; stage hunk satu per satu, branch, stash, konflik, dan grafik commit; run, job, dan log dari GitHub Actions dan GitLab CI.
- **Berhati-hati dengan folder Anda.** Folder yang tidak Anda percayai tidak memulai apa pun: tidak ada proses Git, server bahasa, hook, atau agen. Perintah Git yang merusak tidak pernah diizinkan selamanya, dan commit diperiksa dari rahasia.
- **Dikte.** Berbicaralah ke editor dan kolom pesan, secara online melalui OpenAI atau Groq, atau secara lokal dengan Whisper dan Parakeet.
- **Asli sepenuhnya.** Jendela, tab, menu, dan bar alat AppKit, TextKit 2 untuk editor, Metal untuk terminal, WebKit hanya di tab browser. Mode gelap, Tingkatkan Kontras, dan VoiceOver bekerja seperti di setiap app Mac.
- **Dalam bahasa Anda.** Antarmuka tersedia dalam 35 bahasa: Arab, Katala, Tionghoa (Sederhana), Tionghoa (Tradisional), Kroasia, Ceko, Denmark, Belanda, Inggris, Finlandia, Prancis, Jerman, Yunani, Ibrani, Hindi, Hungaria, Indonesia, Italia, Jepang, Korea, Melayu, Norwegia, Polandia, Portugis (Brasil), Portugis (Portugal), Rumania, Rusia, Slowakia, Slovenia, Spanyol, Swedia, Thai, Turki, Ukraina, dan Vietnam.

## Persyaratan

- macOS 14 Sonoma atau lebih baru, di Apple Silicon atau Intel. Dango dikembangkan dan diuji di macOS 27.
- Untuk utas AI, agen yang ingin Anda gunakan; lihat [Agen AI](#agen-ai).
- Untuk server bahasa dan adaptor debug, yang diinstal Dango saat Anda memintanya: Go untuk gopls dan `dlv`; Node.js untuk server TypeScript, Python, JSON, YAML, CSS, HTML, dan shell serta debugger JavaScript; Xcode Command Line Tools untuk clangd, sourcekit-lsp, dan `lldb-dap`. rust-analyzer diunduh dari rilis GitHub-nya.
- Untuk prediksi saat mengetik, paket GitHub Copilot (yang gratis pun cukup), atau model fill-in-the-middle di Ollama atau LM Studio.

## Instalasi

**Dari image disk.** Setiap rilis adalah image disk, `Dango-<version>.dmg`, dengan satu app universal untuk Apple Silicon dan Intel, ditandatangani dengan Developer ID dan dinotarisasi oleh Apple. Buka, lalu seret Dango ke folder Aplikasi. Homebrew Cask menginstal image yang sama. Belum ada rilis yang diterbitkan.

**Pembaruan.** Dango dari image disk memperbarui dirinya sendiri dengan [Sparkle](https://sparkle-project.org). Pada pembukaan kedua, app menanyakan apakah boleh memeriksa pembaruan sendiri; hingga Anda setuju, app tidak mengirim permintaan apa pun. **Dango › Periksa Pembaruan…** langsung memeriksa. Pembaruan ditandatangani dengan kunci milik app sendiri, yang diperiksa Sparkle sebelum menginstal apa pun. Versi yang Anda bangun sendiri tidak memiliki pembaru.

**Dari kode sumber.** Lihat [Membangun dari kode sumber](#membangun-dari-kode-sumber), lalu pindahkan `target/release/Dango.app` ke `/Applications`.

**Perintah terminal.** **Dango › Instal Perintah “dango”…** menulis skrip kecil ke `~/.local/bin/dango`, tanpa hak administrator; `~/.local/bin` harus ada di `PATH` Anda. Setelah itu, `dango .` membuka folder saat ini dan `dango file.rs` membuka sebuah file. Dango tidak pernah mengganti file di sana yang tidak ditulisnya sendiri.

## Memulai

1. **Buka folder** dengan ⌘O atau `dango <folder>`. Pertama kali, Dango menanyakan apakah Anda memercayainya (lihat [Kepercayaan](#kepercayaan)).
2. **Bar samping** beralih antara File (⌘1), Pencarian (⌘2), Agen (⌘3), Git (⌘4), Tes (⌘5), Debug (⌘6), CI (⌘7), dan Container (⌘8); klik kanan menyembunyikan yang tidak Anda gunakan, dan ⌃⌘S menutup bar samping.
3. **Setiap dokumen adalah tab** di jendela folder, tab bawaan macOS. “+” di ujung bar tab dan ⌘T menawarkan file baru, terminal, browser, dan setiap agen yang terinstal, juga di worktree Git tersendiri; menahan “+” menampilkan hal yang sama sebagai menu.
4. **Buka Cepat** dengan ⌘P menemukan file, dan dengan `#` simbol proyek; ⇧⌘F mencari dan mengganti di proyek.
5. **Berbicaralah dengan agen** di tab AI. ⌘Esc memindahkan fokus papan ketik ke kolom pesannya dan kembali.
6. **Pengaturan** dengan ⌘, berisi tab Umum, CI, Agen, Agen Bawaan, Server bahasa, Dikte, dan Prediksi.

Bar alat berisi mundur dan maju melalui tempat yang Anda edit (⌃- dan ⌃⇧-), branch, Buka Cepat, dan ikhtisar aktivitas, yang mencantumkan semua yang sedang berjalan. **Buka di Samping** di menu konteks file menampilkan dokumen kedua di samping yang pertama. Setelah mulai ulang atau crash, setiap jendela kembali dengan tab-tabnya, dan editor yang berisi teks belum disimpan kembali dengan teks tersebut. Gambar, audio, video, font, dan dokumen Office terbuka di tab melalui Lihat Cepat, PDF melalui PDFKit.

## Agen AI

| Agen | Cara berjalan | Yang dibutuhkan |
|---|---|---|
| Claude | perintah `claude`, yang diajak bicara langsung melalui protokol stream-json-nya | `claude` terinstal dan sudah masuk; Dango mencari di `PATH` shell login, di `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin`, dan `/usr/local/bin`. Masuk hanya dilakukan di `claude` sendiri |
| Codex | `codex-acp` melalui Agent Client Protocol (ACP) | Node.js dan npm; Dango menginstal paket berversi tetap saat Anda memintanya |
| Antigravity | `agy acp` | `agy` milik Anda di `PATH`, atau unduhan berversi tetap, diinstal saat Anda memintanya |
| OpenCode | `opencode acp` | `opencode` milik Anda di `PATH`, atau unduhan berversi tetap, diinstal saat Anda memintanya |
| Agen ACP lainnya | dari registri ACP, yang diambil hanya saat Anda memintanya | apa pun yang disebutkan entri registri; entri tanpa checksum hanya diinstal setelah Anda mengonfirmasi |
| Agen bawaan | dibangun ke dalam app, dengan alat untuk file, pencarian, perintah, diagnostik, dan permintaan web | kunci API untuk Anthropic, OpenAI, Google Gemini, atau OpenRouter, disimpan di rantai kunci; Ollama atau LM Studio di Mac ini tanpa kunci; server apa pun yang mendukung API OpenAI atau Anthropic |

**Pengaturan › Agen** menampilkan agen yang dimiliki Mac ini, menginstal yang lain beserta pembaruannya, dan membuka registri ACP. Agen yang diinstal Dango dikunci ke versi yang telah diuji dan diperiksa terhadap checksum yang dipublikasikan (npm integrity atau SHA-256); Dango tidak menjalankan paket `npx` tanpa versi.

### Lini masa

Setiap agen bekerja di tab AI dengan lini masa yang sama: jawaban, panggilan alat beserta keluarannya, permintaan izin, pertanyaan, rencana untuk disetujui dan dikomentari (⌥⌘K), daftar tugas, subagen, dan tugas latar belakang. Dua tombol di kepala tab mencantumkan subagen dan proses latar belakang yang sedang berjalan. Sebut sebuah file, `@terminal`, atau `@browser` untuk memberikan isinya kepada agen. Titik hijau di tab menunjukkan sesi yang sedang bekerja, titik merah sesi yang menunggu Anda, dan notifikasi memberi tahu saat sesi selesai atau menunggu sementara Anda melihat ke tempat lain. Untuk Claude, baris status menampilkan konteks dan penggunaan.

### Izin dan mode

Utas baru dimulai dalam mode “Manual”: agen bertanya sebelum mengubah file atau menjalankan perintah. Untuk perintah Git yang merusak (`push --force`, `reset --hard`, `clean -fd`), Dango tidak menawarkan “Selalu Izinkan”. Agen bawaan menyimpan aturan “Selalu Izinkan” per proyek, mencantumkannya di **Pengaturan › Agen Bawaan**, dan bertanya setiap kali untuk perintah Git yang merusak, apa pun yang tersimpan; Claude dan agen ACP menyimpan aturan seperti itu di konfigurasi mereka sendiri. Dengan “Izinkan Semua” dan tanpa sandbox, utas menampilkan peringatan permanen.

### Checkpoint dan worktree

Sebelum setiap prompt, Dango menyimpan checkpoint, sebagai commit di bawah `refs/ide-checkpoints/` di repositori proyek, atau di folder tersendiri untuk proyek tanpa Git; push atau fetch biasa tidak pernah membawanya. Apa yang diubah agen dapat ditinjau dan dibatalkan per pesan. Sebuah utas dapat bekerja di worktree Git tersendiri: panel Git menampilkan apa yang diubahnya dan mengambil alih hasilnya melalui gabungkan atau cherry-pick.

## Editor

- **Warna sintaksis, nomor baris, dan tanda perubahan** terhadap commit terakhir, cari dan ganti, Buka Cepat (⌘P), dan pencarian proyek (⇧⌘F).
- **Indentasi** mengikuti file dan `.editorconfig`: tab atau spasi, indentasi otomatis, pasangan kurung.
- **Beberapa titik sisip:** ⌥-klik, ⌥-seret untuk kolom, ⌃⇧↑ dan ⌃⇧↓, ⌘D untuk kemunculan berikutnya.
- **Pelipatan** (⌥⌘← dan ⌥⌘→, dengan ⇧ untuk semuanya), alihkan komentar (⌘/), buka baris (⌃G), spasi kosong yang terlihat, dan bungkus baris (⌥Z).
- **File mempertahankan pengodeannya**, tanda urutan byte, dan akhir barisnya. File yang berubah di disk dimuat ulang jika editor tidak berisi teks yang belum disimpan; jika ada, sebuah bar menawarkan perbedaannya dan penggabungan.
- **Prediksi saat mengetik.** Saat Anda mengetik, sebuah saran muncul dalam warna abu-abu, dan Tab menerimanya. Saran berasal dari GitHub Copilot atau dari model lokal di Ollama atau LM Studio, sesuai pilihan di **Pengaturan › Prediksi**; model lokal menjaga kode Anda tetap di Mac ini.
- **Minimap dan Sticky Scroll** (menu Editor): gambar sempit dari seluruh file di samping teks, dan baris judul dari blok-blok tempat baris teratas berada, ditahan di atasnya.
- **Diagnostik di akhir baris** (menu Lihat): pesan paling serius di setiap baris setelah jeda satu detik, dengan simbol untuk tingkat keparahannya.
- **Pratinjau langsung halaman statis:** Editor › Buka di Tab Browser menyajikan halaman dari folder tepercaya langsung dari Dango sendiri dan memuat ulang tab saat file disimpan.
- **Prettier** memformat JavaScript, TypeScript, CSS, HTML, Markdown, dan lainnya jika proyek mengonfigurasinya, dengan `node_modules/.bin/prettier` milik proyek itu sendiri; pengaturan `formatter` memilih pemformat per bahasa.
- **Mode Vim** (Editor › Mode Vim): mode Normal, Insert, Visual, dan Replace, hitungan, operator dengan gerakan dan objek teks, `.`, register, tanda, dan makro. `u` dan ⌃R menggunakan urungkan milik editor, dan satu putaran penyisipan adalah satu langkah. `"+` dan `"*` adalah papan klip; pengaturan `vimClipboard` juga menjadikan register tanpa nama sebagai papan klip. `/`, `?`, `n`, `N`, `*`, `:s`, dan `:%s` mencari dengan ekspresi reguler Rust, bukan pola Vim: tidak ada `\v`, dan `\b` sebagai pengganti `\<` dan `\>`. Perintah Ex adalah `:w`, `:q`, `:wq`, `:x`, `:noh`, dan `:<line>`. Dalam mode Normal, ⌃G dipakai Vim, bukan untuk buka baris.
- **Sinkronisasi pengaturan** melalui folder di iCloud Drive (**Pengaturan › Umum**): pengaturan dan pintasan Anda mengikuti Anda ke Mac Anda yang lain; jalur, alamat lokal, dan persetujuan tetap di masing-masing Mac.

## Server bahasa

Go, Rust, TypeScript dan JavaScript, Python, C, C++ dan Objective-C, Swift, skrip shell, JSON, YAML, CSS, HTML, TOML, Markdown, dan Lua, serta server milik Anda sendiri dari **Pengaturan › Server bahasa**. Dango menginstal server saat Anda memintanya, ke folder alatnya sendiri, dan menggunakan `tsc` milik proyek TypeScript jika dapat berfungsi sebagai server bahasa.

Yang mereka berikan: diagnostik (⌘' dan ⇧⌘' berpindah di antaranya), info saat menunjuk (⌃⌘I), pelengkapan (⌥Esc), bantuan tanda tangan, buka definisi (⌃⌘J), definisi tipe dan implementasi, referensi (⌃⇧⌘F), hierarki panggilan dan tipe (⌃⇧⌘H), ganti nama (⌃⌘E), pemformatan (⌥⇧F), warna semantik, pelipatan, dan kerangka (⇧⌘O); tipe dan nama parameter di dalam baris; perbaikan cepat dan refaktor di balik bola lampu (⌥⌘A); simbol proyek dengan ⌥⌘O.

Selain server milik file itu sendiri, ESLint memeriksa JavaScript dan TypeScript pada proyek yang menggunakannya, dengan perbaikan cepatnya, dan singkatan Emmet muncul dalam pelengkapan untuk HTML, CSS, dan JSX. Dango menginstal keduanya saat Anda memintanya; **Pengaturan › Umum** memperbaiki masalah ESLint saat menyimpan jika Anda mau.

## Tes

Panel Tes mencantumkan tes sebuah folder per paket dan file beserta statusnya: `cargo test` atau cargo-nextest, Go, pytest, Vitest, dan Jest. Simbol di samping setiap tes pada margin menjalankannya, ⌥-klik men-debug-nya, dan assert yang gagal menandai barisnya. Menu Debug menjalankan tes di titik sisip (⌃⌥⌘U), tes terakhir sekali lagi (⌃⌥⌘G), atau semuanya, dan men-debug tes di titik sisip. Tes hanya berjalan di folder tepercaya, karena tes menjalankan kode proyek; `cargo test` mencantumkan tesnya saat Anda memintanya, karena pencantuman membangun setiap program tes.

## Debugger

Breakpoint, juga dengan kondisi dan sebagai logpoint, eksekusi langkah demi langkah, thread dengan call stack-nya, variabel dan ekspresi yang dipantau di inspektur (⌥⌘I), nilai di samping kode selama program dijeda, dan konsol debug yang mengevaluasi di frame yang dijeda.

| Bahasa | Adaptor debug |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` dari Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | debugger JavaScript, di Node.js |
| Python | debugpy |

`.vscode/launch.json` proyek dibaca. Menu Debug menggunakan pintasan Xcode: ⌘\\ memasang breakpoint, ⌘Y mengaktifkan atau menonaktifkan semuanya, ⌃⌘Y melanjutkan, F6, F7, dan F8 melangkahi, masuk, dan keluar, ⌘. menghentikan.

## Terminal

Terminal di tab (⌃\`) dengan lingkungan shell login, digambar dengan Metal. ⌘K membersihkan layar. Gambar yang ditempel atau diseret diberikan ke agen pemrograman yang berjalan di terminal, atau menempelkan jalur salinan yang disimpan. ⌘-klik membuka tautan dan nama file. Server pengembangan yang dimulai terminal dapat dibuka di tab browser.

## Browser

Tab browser dengan kolom alamat (⌘L), mundur dan maju (⌘[ dan ⌘]), muat ulang (⌘R), pencarian, unduhan yang bertanya lebih dulu, dan Web Inspector. `@browser` memberikan halaman ke agen, ditandai sebagai teks dari luar.

## Git

Panel Git melakukan stage dan membuang file atau hunk satu per satu, membuat commit, melakukan commit lalu push atau sinkronisasi, berpindah dan membuat branch, menyimpan stash, menyelesaikan konflik gabungan, dan menggambar grafik commit terbaru. Blame menampilkan siapa yang terakhir mengubah baris di bawah kursor, atau kolom untuk seluruh file. Sebuah tombol menulis pesan commit dengan AI; diff hanya dikirim setelah gitleaks tidak menemukan rahasia di dalamnya, dan setiap commit juga diperiksa dari rahasia.

## CI

GitHub Actions dan GitLab CI, juga GitHub Enterprise Server dan GitLab yang dikelola sendiri:

- run, job, dan log dari branch saat ini;
- menjalankan ulang job yang gagal, satu job, atau, di GitHub, seluruh run; memulai job manual atau menyetujui deployment yang menunggu; memulai pipeline baru atau workflow yang boleh dimulai secara manual; membatalkan;
- pull request atau merge request yang terbuka dan issue, yang terbuka di tab browser;
- log job yang gagal diberikan ke agen, ditandai sebagai teks dari luar.

Token berasal dari rantai kunci, `gh` atau `glab`, lingkungan shell login, atau token yang Anda masukkan di **Pengaturan › CI**.

## Container

Panel Container menampilkan proyek Compose milik Docker beserta layanannya, container tunggal, image, dan volume, serta mengikuti event Docker saat terjadi. File Compose di folder muncul sebagai proyek bahkan sebelum container-nya ada. Container dapat dimulai, dihentikan, dan dimulai ulang; log-nya dan sebuah shell terbuka di tab terminal, dan port yang dipublikasikan terbuka di tab browser. Menghapus container, image, atau volume, serta membersihkan data yang tidak digunakan, akan meminta konfirmasi terlebih dahulu. Perintah up, down, dan pull dari Compose hanya berjalan di folder tepercaya, karena Compose membangun dan menjalankan apa yang dijelaskan proyek. Menu panel memilih konteks Docker untuk jendela tanpa mengubah konteks yang digunakan shell Anda; perintah yang sama ada di menu Container. Panel ini memerlukan baris perintah `docker`, Docker 24.0.6 atau yang lebih baru.

## Markdown

- **Pratinjau.** File Markdown terbuka dengan pratinjau di sampingnya; **Lihat › Tampilkan/Sembunyikan Pratinjau di Samping** (⇧⌘V) menutup dan membukanya, dan **Pengaturan › Umum** menonaktifkan pratinjau otomatis. Pratinjau menggambar rumus, diagram Mermaid, Graphviz, dan PlantUML, serta gambar di folder; gambar dari web hanya dimuat di folder tepercaya atau setelah Anda memintanya di pratinjau.
- **Menu Format:** tebal (⌘B), miring (⌘I), coret (⇧⌘X), kode (⌥⇧⌘C), tautan (⇧⌘K), judul 1–3 (⌥⌘1–⌥⌘3), daftar berpoin dan bernomor, tugas, kutipan, dan blok kode. Setiap perintah beralih, pada pilihan atau baris, dan merupakan satu langkah urungkan.
- **Tempel.** Teks dari Word atau halaman web masuk sebagai Markdown, gambarnya sebagai file di samping dokumen; ⌥⇧⌘V menempelkan teks biasa.
- **Ekspor.** **File › Ekspor** menulis PDF, satu file HTML dengan gambar di dalamnya, atau dokumen Word. Gambar dari web hanya disertakan dari folder tepercaya.
- **Buka di Splat.** Jika [Splat](https://github.com/sventmueller/splat) terinstal, **File › Buka di Splat** dan menu konteks daftar file serta tab membuka dokumen di sana, setelah menyimpannya.

## Dikte

Diktekan ke kolom pesan tab AI atau ke editor: dengan tombol mikrofon di sana, **Edit › Mulai Dikte** (⌃⌥D), atau tombol fn (tahan untuk merekam, ketuk dua kali untuk terus merekam). Pengenalan suaranya berasal dari [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak sendiri tidak perlu diinstal.

- **Pengaturan › Dikte** mengatur mode dan model (online dengan OpenAI atau Groq, lokal dengan Whisper atau Parakeet), bahasa, pascapemrosesan, mikrofon, dan kosakata Anda sendiri.
- **Model lokal** diunduh dari Hugging Face saat Anda memilihnya, dengan kemajuan dan opsi untuk membatalkan. Model disimpan di folder yang digunakan Lowspeak, sehingga setiap app menggunakan apa yang diunduh app lainnya. Model lokal menjaga rekaman tetap di Mac ini.
- **Transkripsi online** memerlukan kunci OpenAI atau Groq, yang disimpan Dango di item rantai kunci miliknya sendiri.
- **Dari Lowspeak.** Jika pengaturan Lowspeak berbeda dari default, tab Dikte menawarkan sekali untuk mengambil alih pengaturannya, dan kuncinya jika Anda minta; macOS bertanya sebelum Dango membaca salah satunya (pilih “Izinkan”, bukan “Selalu Izinkan”).
- **Izin.** macOS meminta akses mikrofon pada dikte pertama. Tombol fn berfungsi selama Dango adalah app yang aktif dan Lowspeak tidak berjalan; tombol ini memerlukan izin Aksesibilitas, yang diminta Dango hanya setelah dikte pertama, tidak pernah saat dibuka.

## Kepercayaan

Sebuah folder dapat membuat program berjalan begitu sesuatu dimulai di dalamnya: hook Git dan pengaturan repositori seperti `core.fsmonitor`, server bahasa dari `node_modules`, hook agen, `.mcp.json`. Saat pertama kali Anda membuka folder, Dango menanyakan apakah Anda memercayainya.

- **Tepercaya:** Git, server bahasa, debugger, agen, dan CI berjalan untuk folder ini dan semua yang ada di bawahnya.
- **Terbatas:** Anda dapat membaca dan mengedit; tanda perubahan di editor berasal dari membaca `.git` secara langsung. Tidak ada yang dikonfigurasi folder yang dimulai. Panel Git, CI, dan AI menyatakannya dan menawarkan untuk memercayai folder.

Teks yang tidak berasal dari Anda dapat membawa instruksi yang dimaksudkan untuk mengarahkan agen (prompt injection). Karena itu keluaran terminal (`@terminal`), halaman web (`@browser`), log CI yang diberikan ke agen, file di luar proyek, dan teks panjang yang ditempel dari halaman web dikirim dengan tanda: chip bertuliskan “Konten Eksternal”, dan agen menerima teks dengan pemberitahuan bahwa itu adalah data tidak tepercaya yang instruksinya tidak boleh diikuti. Halaman web agen bawaan datang dengan pemberitahuan yang sama. Teks yang ditempel dianggap berasal dari halaman web jika browser menyatakannya di papan klip, seperti yang dilakukan Safari, Chrome, dan Firefox; teks dari tempat lain, dan frasa pendek beberapa kata, tetap menjadi teks Anda sendiri.

## Sandbox

Dango sendiri berjalan tanpa App Sandbox, karena setiap program yang dimulainya akan mewarisinya: terminal, Git, server bahasa, dan agen tidak akan dapat bekerja. Sebagai gantinya, setiap utas AI memiliki sakelar sandbox, nonaktif secara default. Apa yang dibatasinya bergantung pada agen, dan tooltip sakelar menyebutkannya:

| Agen | Dengan sandbox aktif |
|---|---|
| Claude | perintah shell berjalan di sandbox milik Claude sendiri; alat file dan web-nya tidak |
| Codex | perintah dan perubahan hanya menulis di proyek, tanpa jaringan |
| Agen bawaan | file, permintaan web, dan perintah tetap di dalam proyek dan folder yang diizinkan, tanpa jaringan (`sandbox-exec`) |
| Antigravity, OpenCode | tidak ada sandbox yang dapat diaktifkan Dango |

## Tampilan dan aksesibilitas

- **Mode gelap** mengikuti sistem; **Tingkatkan Kontras** dan **Kurangi Gerakan** dihormati.
- **VoiceOver** menamai bar samping, tab, kartu lini masa AI, baris Git dan CI, debugger, dan teks terminal. Breakpoint tidak pernah dibedakan hanya dengan warna.
- **Papan ketik.** Setiap perintah memiliki pintasan, dan semuanya dapat diubah di `keybindings.json`; ⌥⌘\` berpindah ke panel berikutnya.
- **Kanan ke kiri.** Dalam bahasa Arab dan Ibrani, jendela dicerminkan, sementara tampilan kode tetap dari kiri ke kanan.

Apa yang ditambahkan Dango di atas AppKit, area demi area, ada di [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Bahasa

Antarmuka diterjemahkan ke dalam 35 bahasa: Arab, Katala, Tionghoa (Sederhana), Tionghoa (Tradisional), Kroasia, Ceko, Denmark, Belanda, Inggris, Finlandia, Prancis, Jerman, Yunani, Ibrani, Hindi, Hungaria, Indonesia, Italia, Jepang, Korea, Melayu, Norwegia, Polandia, Portugis (Brasil), Portugis (Portugal), Rumania, Rusia, Slowakia, Slovenia, Spanyol, Swedia, Thai, Turki, Ukraina, dan Vietnam.

Dango mengikuti urutan bahasa di **Pengaturan Sistem › Umum › Bahasa & Wilayah**; di sana Anda juga dapat memilih bahasa khusus untuk Dango di bawah **Aplikasi**. Jika tidak ada terjemahan untuk bahasa Anda, Dango menggunakan bahasa Inggris. Istilah mengikuti satu glosarium, [docs/glossary.md](../../../docs/glossary.md), dan terjemahan Apple sendiri, sehingga Dango terbaca seperti bagian lain Mac Anda.

## Pintasan papan ketik

| Tindakan | Pintasan |
|---|---|
| Buka folder, jendela baru, tutup jendela | ⌘O, ⇧⌘N, ⇧⌘W |
| File baru, simpan, simpan sebagai | ⌘N, ⌘S, ⇧⌘S |
| Tab baru, tutup tab, tab berikutnya dan sebelumnya | ⌘T, ⌘W, ⇧⌘] dan ⇧⌘[ |
| Buka Cepat, cari di proyek | ⌘P, ⇧⌘F |
| File, Pencarian, Agen, Git, Tes, Debug, CI, Container | ⌘1 hingga ⌘8 |
| Tampilkan atau sembunyikan bar samping, panel berikutnya | ⌃⌘S, ⌥⌘\` |
| Mundur dan maju melalui tempat yang diedit | ⌃- dan ⌃⇧- |
| Alihkan komentar, buka baris | ⌘/, ⌃G |
| Kemunculan berikutnya, titik sisip di atas dan di bawah | ⌘D, ⌃⇧↑ dan ⌃⇧↓ |
| Lipat, buka lipatan, lipat semua, buka semua lipatan | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Bungkus baris, inspektur | ⌥Z, ⌥⌘I |
| Pelengkapan, info, buka definisi | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Referensi, hierarki panggilan, ganti nama | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simbol di file, di proyek | ⇧⌘O, ⌥⌘O |
| Perbaikan cepat, format | ⌥⌘A, ⌥⇧F |
| Masalah berikutnya dan sebelumnya | ⌘' dan ⇧⌘' |
| Pratinjau Markdown, tempel sebagai teks biasa | ⇧⌘V, ⌥⇧⌘V |
| Tebal, miring, coret, kode, tautan | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Judul 1 hingga 3 | ⌥⌘1 hingga ⌥⌘3 |
| Terminal baru, bersihkan terminal | ⌃\`, ⌘K |
| Fokus ke kolom pesan agen, komentari rencana | ⌘Esc, ⌥⌘K |
| Kolom alamat, muat ulang, mundur, maju (browser) | ⌘L, ⌘R, ⌘[, ⌘] |
| Mulai atau hentikan dikte | ⌃⌥D |
| Breakpoint, aktifkan atau nonaktifkan semua breakpoint | ⌘\\, ⌘Y |
| Jalankan tes di titik sisip, jalankan ulang tes terakhir | ⌃⌥⌘U, ⌃⌥⌘G |
| Lanjutkan, langkahi, masuk, keluar, hentikan | ⌃⌘Y, F6, F7, F8, ⌘. |
| Pengaturan | ⌘, |

`keybindings.json` di folder data app (lihat [Lokasi data](#lokasi-data)) mengubah salah satu dari pintasan tersebut.

## Privasi dan keamanan

- **Tanpa telemetri,** tanpa analitik, tanpa laporan crash yang dikirim ke mana pun. Kesalahan masuk ke log sistem (Konsol, di bawah nama app).
- **Jaringan hanya untuk yang Anda mulai:** agen yang Anda gunakan, masing-masing ke layanannya sendiri; registri ACP dan unduhan agen, server bahasa, adaptor debug, serta model dikte saat Anda memintanya; host CI tempat Anda masuk; prediksi dari Copilot jika Anda memilihnya; penyedia agen bawaan; transkripsi online jika Anda memilihnya; halaman yang Anda buka di tab browser; dan pemeriksaan pembaruan setelah Anda mengizinkannya.
- **Kode meninggalkan Mac** hanya ke tempat Anda memilih layanan untuknya: agen, prediksi Copilot, pesan commit yang ditulis dengan AI. Diff untuk pesan commit hanya dikirim setelah gitleaks tidak menemukan rahasia di dalamnya.
- **Kunci dan token** tetap di rantai kunci macOS, tidak pernah di file pengaturan. Kunci app lain, seperti milik Lowspeak, hanya dibaca jika Anda setuju, dan hanya sekali.
- **Unduhan berversi tetap.** Agen, server bahasa, dan adaptor debug yang diinstal Dango dikunci ke versi yang telah diuji dan diperiksa terhadap checksum-nya.
- **Penandatanganan kode** dengan Hardened Runtime; pembaruan hanya diinstal dengan tanda tangan milik app sendiri.

## Lokasi data

`<id>` adalah pengenal bundel, `local.agentide` untuk rilis (`local.agentide.debug` untuk build debug). Pengenal ini tidak memuat nama tampilan, sehingga penggantian nama mempertahankan semua data.

| Tempat | Isi |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (daftar panel AI), `agent-threads/` (percakapan agen bawaan), `layouts/` (tab per proyek), `recovery/` (editor yang belum disimpan setelah crash), `checkpoints/` (untuk folder tanpa Git), `worktrees/`, `agents/` dan `tools/` (agen, server bahasa, dan adaptor debug yang terinstal), `task-output/` |
| `~/Library/Caches/<id>/` | gambar yang ditempel, skrip integrasi shell, registri ACP; boleh dihapus |
| `~/Library/Logs/<id>/dictation/` | file log dikte |
| `~/Library/Application Support/Lowspeak` dan `FluidAudio` | model dikte lokal, dibagikan dengan Lowspeak |
| `~/Library/WebKit/<id>/` | cookie dan data situs web tab browser |
| `<project>/.dango/settings.json` | pengaturan milik proyek |
| repositori Git proyek | checkpoint, sebagai commit di bawah `refs/ide-checkpoints/`; push atau fetch biasa tidak pernah membawanya |
| rantai kunci, layanan `<id>` | kunci API agen bawaan dan dikte, token CI |
| `~/.local/bin/dango` | perintah terminal, jika Anda menginstalnya |

Claude menyimpan sesinya sendiri di `~/.claude/projects`; Dango membacanya untuk melanjutkan sesi tetapi tidak mengubahnya.

## Batasan yang diketahui

- **Tanpa App Sandbox** untuk app itu sendiri, karena terminal, Git, server bahasa, dan agen akan mewarisinya; folder harus tepercaya sebelum apa pun yang dikonfigurasinya berjalan. Antigravity dan OpenCode tidak memiliki sandbox yang dapat diaktifkan Dango.
- **Angka penggunaan dan rincian konteks Claude** bergantung pada bagian protokolnya yang ditandai eksperimental, dan disembunyikan jika bagian itu tidak ada.
- **Memutar mundur percakapan** hanya berfungsi untuk Claude; untuk agen lain hanya kode yang dikembalikan.
- **Masuk ke CI melalui browser** dinonaktifkan; gunakan `gh`, `glab`, token, atau lingkungan.
- **Gemini CLI tidak ditawarkan:** Google mengakhiri masuk dengan akun pribadi dan mengarahkan ke Antigravity. Utas Gemini sebelumnya tetap dapat dibaca.

## Membangun dari kode sumber

Anda memerlukan:

- macOS 14 atau lebih baru dengan Xcode Command Line Tools (`xcode-select --install`); Xcode sendiri tidak diperlukan.
- Rust sesuai yang disebutkan `rust-toolchain.toml`. `rust` dari Homebrew membangun untuk prosesor Mac ini; build universal sebuah rilis memerlukan rustup dengan target `aarch64-apple-darwin` dan `x86_64-apple-darwin`.
- Salinan [splat](https://github.com/sventmueller/splat) dan [lowspeak](https://github.com/sventmueller/lowspeak) di samping yang ini (`../splat`, `../lowspeak`), keduanya di branch `main`; workspace mengambil crate dari keduanya melalui jalur.
- cmake, untuk setiap build dengan dikte (`make run`, `make build`, `make release`), yang membangun whisper.cpp. `make check` membangun tanpanya.

| Perintah | Yang dilakukan |
|---|---|
| `make run` | membangun app debug (`target/debug/Dango Debug.app`) dan membukanya |
| `make build` | membangun app rilis (`target/release/Dango.app`), ditandatangani dengan sertifikat lokal yang dibuat build pertama di rantai kunci login; menaikkan nomor build di `VERSION` |
| `make lint` | pemformatan dan clippy; peringatan membuatnya gagal |
| `make check` | `lint`, lalu semua tes; tidak memerlukan rantai kunci, jendela, atau jaringan |
| `make test` | `check`, lalu swauji `probe` dan audit aksesibilitas, kiri ke kanan dan kanan ke kiri, di app sungguhan |
| `make perf` | anggaran waktu, dalam build rilis; hanya bermakna di Mac yang sedang tidak sibuk |
| `make release` | uji coba rilis ke `target/release-dry-run/`, ditandatangani dengan sertifikat lokal; tidak mengirim apa pun ke mana pun |
| `make release-notarized` | `check`, lalu rilis itu sendiri ke `dist/`: tanda tangan Developer ID, notarisasi, feed pembaruan yang ditandatangani dengan kunci milik app sendiri; menaikkan nomor build |

Debug dan rilis adalah app terpisah dengan pengaturan dan data masing-masing. Pembangunan, pengujian, swauji, rilis, dan aturan yang diikuti kode dijelaskan di [docs/development.md](../../../docs/development.md), keputusan di baliknya di [docs/plan.md](../../../docs/plan.md). Catatan rilis adalah [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), diterjemahkan di samping setiap README di `assets/readme/<language>/`.

## Lisensi

Dango dilisensikan di bawah salah satu dari

- Lisensi Apache, Versi 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Lisensi MIT ([LICENSE-MIT](../../../LICENSE-MIT))

sesuai pilihan Anda.

Kecuali Anda menyatakan sebaliknya secara eksplisit, setiap kontribusi yang dengan sengaja Anda kirimkan untuk disertakan dalam Dango, sebagaimana didefinisikan dalam lisensi Apache-2.0, akan dilisensikan ganda seperti di atas, tanpa syarat atau ketentuan tambahan.
