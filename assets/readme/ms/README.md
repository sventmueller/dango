<p><img src="../../readme-icon.png" alt="Ikon apl Dango" width="135" height="135"></p>

# Dango

**Editor macOS asli untuk bekerja dengan ejen pengekodan.**

Dango meletakkan kod, ejen yang mengubahnya dan segala-galanya di sekelilingnya dalam satu tetingkap bagi setiap folder: editor dengan pelayan bahasa dan ramalan semasa menaip, terminal, pelayar, Git, CI, penyahpepijat dan thread AI dengan Claude, Codex, Antigravity, OpenCode atau ejen yang terbina dalam apl. Setiap ejen bekerja dalam garis masa yang sama, bertanya sebelum mengubah fail atau menjalankan perintah, dan meninggalkan titik semak sebelum setiap prompt, supaya anda boleh menyemak dan memutar balik apa yang dilakukannya, mesej demi mesej. Tiada apa-apa yang dikonfigurasikan oleh folder akan dijalankan sebelum anda mempercayainya, dan teks dari luar sampai kepada ejen dengan tanda tidak dipercayai. Rencanakan ke dalam editor dan medan mesej, dan pengecaman pertuturan boleh kekal pada Mac anda. Dango ditulis dalam Rust di atas kawalan AppKit yang tulen, tanpa Electron, menggunakan 35 bahasa dan tiada telemetri.

[![Muat turun Dango.dmg](https://img.shields.io/badge/Muat_turun_Dango-0A84FF?style=for-the-badge&logo=apple&logoColor=white)](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)

**[Muat turun Dango.dmg](https://github.com/sventmueller/dango/releases/latest/download/Dango.dmg)**: ditandatangani dan dinotari oleh Apple, untuk macOS 14 atau lebih baharu pada Apple Silicon dan Intel. Pautan tersedia mulai keluaran pertama.

**Menemui pepijat?** [Buka issue](https://github.com/sventmueller/dango/issues/new) dan nyatakan dengan ejen mana dan dalam jenis projek apa ia berlaku.

[![Rust edisi 2024](https://img.shields.io/badge/Rust-2024_edition-000000?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![macOS 14 atau lebih baharu](https://img.shields.io/badge/macOS-14%2B-000000?logo=apple&logoColor=white)](https://www.apple.com/macos/)
![Universal: Apple Silicon dan Intel](https://img.shields.io/badge/Universal-Apple_Silicon_%7C_Intel-333333?logo=apple&logoColor=white)
[![AppKit melalui objc2](https://img.shields.io/badge/AppKit-objc2_0.6-147EFB?logo=apple&logoColor=white)](https://github.com/madsmtm/objc2)
[![Agent Client Protocol](https://img.shields.io/badge/ACP-Agent_Client_Protocol-6E56CF)](https://agentclientprotocol.com)
[![Language Server Protocol](https://img.shields.io/badge/LSP-3.17-0078D4)](https://microsoft.github.io/language-server-protocol/)
[![Debug Adapter Protocol](https://img.shields.io/badge/DAP-debugger-0078D4)](https://microsoft.github.io/debug-adapter-protocol/)
[![Terminal: alacritty_terminal](https://img.shields.io/badge/terminal-alacritty__terminal_0.26-F46D01)](https://github.com/alacritty/alacritty)
![35 bahasa antara muka](https://img.shields.io/badge/languages-35-4C9A2A)
![Tiada telemetri](https://img.shields.io/badge/telemetry-none-4C9A2A)
![Ditandatangani dengan Hardened Runtime](https://img.shields.io/badge/Hardened_Runtime-signed-4C9A2A)
[![Lesen: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT_OR_Apache--2.0-4C9A2A)](#lesen)

[Bahasa Indonesia](../id/README.md) ·
**Bahasa Melayu** ·
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

## Kandungan

- [Ciri sepintas lalu](#ciri-sepintas-lalu)
- [Keperluan](#keperluan)
- [Pemasangan](#pemasangan)
- [Bermula](#bermula)
- [Ejen AI](#ejen-ai)
  - [Garis masa](#garis-masa)
  - [Keizinan dan mod](#keizinan-dan-mod)
  - [Titik semak dan worktree](#titik-semak-dan-worktree)
- [Editor](#editor)
- [Pelayan bahasa](#pelayan-bahasa)
- [Ujian](#ujian)
- [Penyahpepijat](#penyahpepijat)
- [Terminal](#terminal)
- [Pelayar](#pelayar)
- [Git](#git)
- [CI](#ci)
- [Container](#container)
- [Markdown](#markdown)
- [Perencanaan](#perencanaan)
- [Kepercayaan](#kepercayaan)
- [Sandbox](#sandbox)
- [Penampilan dan kebolehcapaian](#penampilan-dan-kebolehcapaian)
- [Bahasa](#bahasa)
- [Pintasan papan kekunci](#pintasan-papan-kekunci)
- [Privasi dan keselamatan](#privasi-dan-keselamatan)
- [Lokasi data](#lokasi-data)
- [Had yang diketahui](#had-yang-diketahui)
- [Membina daripada sumber](#membina-daripada-sumber)
- [Lesen](#lesen)

## Ciri sepintas lalu

- **Setiap ejen dalam satu garis masa.** Claude melalui perintah `claude` miliknya sendiri, Codex, Antigravity dan OpenCode melalui Agent Client Protocol, lebih banyak lagi daripada registry ACP, dan ejen yang terbina dalam apl untuk Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama dan LM Studio. Jawapan, panggilan alat, permintaan keizinan, pelan, senarai tugasan, subejen dan tugas latar belakang kelihatan sama, tidak kira ejen mana yang berjalan.
- **Semak dan putar balik.** Sebelum setiap prompt, Dango menyimpan titik semak. Apa yang diubah oleh ejen boleh disemak dan diputar balik bagi setiap mesej, dan thread boleh bekerja dalam worktree Git tersendiri dan menyerahkan kerjanya melalui gabung atau cherry-pick.
- **Satu tetingkap bagi setiap folder.** Fail, terminal, halaman pelayar, thread AI, paparan perbezaan dan pratonton ialah tab macOS dalam tetingkap itu. Bar sisi memuatkan Fail, Carian, Ejen, Git, Ujian, Nyahpepijat, CI dan Container.
- **Editor kod sebenar.** Warna sintaks, beberapa kursor, lipatan, `.editorconfig`, pelayan bahasa untuk 17 bahasa dengan pelengkapan, pergi ke definisi, rujukan, nama semula, pembetulan pantas dan hierarki, serta ramalan semasa menaip daripada GitHub Copilot atau model setempat.
- **Penyahpepijat, Git dan CI terbina dalam.** Titik henti dan langkah demi langkah untuk Rust, C, C++, Swift, Go, JavaScript, TypeScript dan Python; stage hunk tunggal, branch, stash, konflik dan graf commit; larian, job dan log daripada GitHub Actions dan GitLab CI.
- **Berhati-hati dengan folder anda.** Folder yang belum anda percayai tidak memulakan apa-apa: tiada proses Git, pelayan bahasa, hook atau ejen. Perintah Git yang merosakkan tidak pernah dibenarkan secara kekal, dan commit disemak untuk rahsia.
- **Perencanaan.** Bercakap ke dalam editor dan medan mesej, dalam talian melalui OpenAI atau Groq atau secara setempat dengan Whisper dan Parakeet.
- **Asli hingga ke teras.** Tetingkap, tab, menu dan bar alat AppKit, TextKit 2 untuk editor, Metal untuk terminal, WebKit hanya dalam tab pelayar. Mod Gelap, Tingkatkan Kontras dan VoiceOver berfungsi seperti dalam setiap apl Mac.
- **Dalam bahasa anda.** Antara muka tersedia dalam 35 bahasa: Arab, Katalan, Cina (Ringkas), Cina (Tradisional), Croatia, Czech, Denmark, Belanda, Inggeris, Finland, Perancis, Jerman, Yunani, Ibrani, Hindi, Hungary, Indonesia, Itali, Jepun, Korea, Melayu, Norway, Poland, Portugis (Brazil), Portugis (Portugal), Romania, Rusia, Slovak, Slovenia, Sepanyol, Sweden, Thai, Turki, Ukraine dan Vietnam.

## Keperluan

- macOS 14 Sonoma atau lebih baharu, pada Apple Silicon atau Intel. Dango dibangunkan dan diuji pada macOS 27.
- Untuk thread AI, ejen yang anda mahu gunakan; lihat [Ejen AI](#ejen-ai).
- Untuk pelayan bahasa dan penyesuai nyahpepijat, yang dipasang oleh Dango apabila anda memintanya: Go untuk gopls dan `dlv`; Node.js untuk pelayan TypeScript, Python, JSON, YAML, CSS, HTML dan shell serta penyahpepijat JavaScript; Xcode Command Line Tools untuk clangd, sourcekit-lsp dan `lldb-dap`. rust-analyzer dimuat turun daripada keluaran GitHubnya.
- Untuk ramalan semasa menaip, langganan GitHub Copilot (langganan percuma pun memadai), atau model fill-in-the-middle dalam Ollama atau LM Studio.

## Pemasangan

**Daripada imej cakera.** Setiap keluaran ialah imej cakera, `Dango-<version>.dmg`, dengan satu apl universal untuk Apple Silicon dan Intel, ditandatangani dengan Developer ID dan dinotari oleh Apple. Bukanya dan seret Dango ke folder Aplikasi. Homebrew Cask memasang imej yang sama. Belum ada keluaran yang diterbitkan.

**Kemas kini.** Dango daripada imej cakera mengemas kini dirinya sendiri dengan [Sparkle](https://sparkle-project.org). Pada pelancaran kedua, ia bertanya sama ada ia boleh mencari kemas kini sendiri; selagi anda belum bersetuju, ia tidak membuat sebarang permintaan. **Dango › Semak Kemas Kini…** menyemak serta-merta. Kemas kini ditandatangani dengan kunci apl itu sendiri, yang disemak oleh Sparkle sebelum memasang apa-apa. Binaan anda sendiri tiada pengemas kini.

**Daripada sumber.** Lihat [Membina daripada sumber](#membina-daripada-sumber), kemudian alihkan `target/release/Dango.app` ke dalam `/Applications`.

**Perintah terminal.** **Dango › Pasang Perintah “dango”…** menulis skrip kecil ke `~/.local/bin/dango`, tanpa hak pentadbir; `~/.local/bin` mesti berada dalam `PATH` anda. Kemudian `dango .` membuka folder semasa dan `dango file.rs` membuka fail. Dango tidak pernah menggantikan fail di situ yang tidak ditulisnya sendiri.

## Bermula

1. **Buka folder** dengan ⌘O atau `dango <folder>`. Kali pertama, Dango bertanya sama ada anda mempercayainya (lihat [Kepercayaan](#kepercayaan)).
2. **Bar sisi** bertukar antara Fail (⌘1), Carian (⌘2), Ejen (⌘3), Git (⌘4), Ujian (⌘5), Nyahpepijat (⌘6), CI (⌘7) dan Container (⌘8); klik kanan menyembunyikan yang tidak anda gunakan, dan ⌃⌘S melipat bar sisi.
3. **Setiap dokumen ialah tab** dalam tetingkap folder, iaitu tab macOS sendiri. “+” di hujung bar tab dan ⌘T menawarkan fail baharu, terminal, pelayar dan setiap ejen yang dipasang, juga dalam worktree Git tersendiri; menahan “+” menunjukkan perkara yang sama sebagai menu.
4. **Buka Dengan Pantas** dengan ⌘P mencari fail, dan dengan `#` mencari simbol projek; ⇧⌘F mencari dan menggantikan dalam projek.
5. **Bercakap dengan ejen** dalam tab AI. ⌘Esc mengalihkan fokus papan kekunci ke medan mesejnya dan kembali.
6. **Seting** dengan ⌘, memuatkan tab Umum, CI, Ejen, Ejen Terbina Dalam, Pelayan bahasa, Perencanaan dan Ramalan.

Bar alat memuatkan undur dan maju melalui tempat yang anda sunting (⌃- dan ⌃⇧-), branch, Buka Dengan Pantas dan gambaran keseluruhan Aktiviti, yang menyenaraikan semua yang sedang berjalan. **Buka di Sebelah** dalam menu konteks fail menunjukkan dokumen kedua di sebelah dokumen pertama. Selepas mula semula, atau ranap, setiap tetingkap kembali dengan tabnya, dan editor yang mempunyai teks belum disimpan kembali bersama teks itu. Imej, audio, video, fon dan dokumen Office dibuka dalam tab melalui Quick Look, PDF melalui PDFKit.

## Ejen AI

| Ejen | Cara ia berjalan | Apa yang diperlukan |
|---|---|---|
| Claude | perintah `claude`, yang dihubungi terus melalui protokol stream-json miliknya | `claude` dipasang dan telah didaftar masuk; Dango mencari dalam `PATH` shell log masuk, dalam `~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin` dan `/usr/local/bin`. Daftar masuk hanya berlaku dalam `claude` itu sendiri |
| Codex | `codex-acp` melalui Agent Client Protocol (ACP) | Node.js dan npm; Dango memasang pakej dalam versi yang ditetapkan apabila anda memintanya |
| Antigravity | `agy acp` | `agy` anda sendiri dalam `PATH`, atau muat turun dalam versi yang ditetapkan, dipasang apabila anda memintanya |
| OpenCode | `opencode acp` | `opencode` anda sendiri dalam `PATH`, atau muat turun dalam versi yang ditetapkan, dipasang apabila anda memintanya |
| Ejen ACP lain | daripada registry ACP, yang diambil hanya apabila anda memintanya | apa sahaja yang dinyatakan oleh entri registry; entri tanpa checksum hanya dipasang selepas anda mengesahkan |
| Ejen terbina dalam | terbina dalam apl, dengan alat untuk fail, carian, perintah, diagnostik dan permintaan web | kunci API untuk Anthropic, OpenAI, Google Gemini atau OpenRouter, disimpan dalam Rantai Kunci; Ollama atau LM Studio pada Mac ini tanpa kunci; mana-mana pelayan yang menggunakan API OpenAI atau Anthropic |

**Seting › Ejen** menunjukkan ejen yang ada pada Mac ini, memasang yang lain serta kemas kininya, dan membuka registry ACP. Ejen yang dipasang oleh Dango ditetapkan pada versi yang telah diuji dan disemak berbanding checksum yang diterbitkan (npm integrity atau SHA-256); Dango tidak menjalankan pakej `npx` tanpa versi.

### Garis masa

Setiap ejen bekerja dalam tab AI dengan garis masa yang sama: jawapan, panggilan alat dengan outputnya, permintaan keizinan, soalan, pelan untuk diluluskan dan diulas (⌥⌘K), senarai tugasan, subejen dan tugas latar belakang. Dua butang dalam pengepala tab menyenaraikan subejen dan proses latar belakang yang sedang berjalan. Sebut fail, `@terminal` atau `@browser` untuk menyerahkan kandungannya kepada ejen. Titik hijau pada tab menunjukkan sesi yang sedang bekerja, titik merah sesi yang menunggu anda, dan pemberitahuan memaklumkan apabila sesi selesai atau menunggu semasa anda melihat di tempat lain. Untuk Claude, baris status menunjukkan konteks dan penggunaan.

### Keizinan dan mod

Thread baharu bermula dalam mod “Manual”: ejen bertanya sebelum mengubah fail atau menjalankan perintah. Untuk perintah Git yang merosakkan (`push --force`, `reset --hard`, `clean -fd`), Dango tidak menawarkan “Sentiasa Benarkan”. Ejen terbina dalam menyimpan peraturan “Sentiasa Benarkan” bagi setiap projek, menyenaraikannya dalam **Seting › Ejen Terbina Dalam** dan bertanya setiap kali untuk perintah Git yang merosakkan, tidak kira apa yang disimpan; Claude dan ejen ACP menyimpan peraturan sedemikian dalam konfigurasi mereka sendiri. Dengan “Benarkan Semua” dan tanpa sandbox, thread menunjukkan amaran kekal.

### Titik semak dan worktree

Sebelum setiap prompt, Dango menyimpan titik semak, sebagai commit di bawah `refs/ide-checkpoints/` dalam repositori projek, atau dalam foldernya sendiri untuk projek tanpa Git; push atau fetch biasa tidak pernah membawanya. Apa yang diubah oleh ejen boleh disemak dan diputar balik bagi setiap mesej. Thread boleh bekerja dalam worktree Git tersendiri: panel Git menunjukkan apa yang diubahnya dan mengambil alih kerja itu melalui gabung atau cherry-pick.

## Editor

- **Warna sintaks, nombor baris dan tanda perubahan** berbanding commit terakhir, cari dan ganti, Buka Dengan Pantas (⌘P) dan carian projek (⇧⌘F).
- **Inden** mengikut fail dan `.editorconfig`: tab atau ruang, inden automatik, pasangan kurungan.
- **Beberapa kursor:** ⌥-klik, ⌥-seret untuk satu lajur, ⌃⇧↑ dan ⌃⇧↓, ⌘D untuk kemunculan seterusnya.
- **Lipatan** (⌥⌘← dan ⌥⌘→, dengan ⇧ untuk semua), togol ulasan (⌘/), pergi ke baris (⌃G), ruang kosong yang kelihatan dan balut baris (⌥Z).
- **Fail mengekalkan pengekodannya**, tanda tertib bait dan penghujung baris. Fail yang berubah pada cakera dimuat semula apabila editor tiada teks yang belum disimpan; jika ada, satu bar menawarkan perbezaan dan penggabungan.
- **Ramalan semasa menaip.** Semasa anda menaip, cadangan muncul dalam warna kelabu, dan Tab menerimanya. Cadangan datang daripada GitHub Copilot atau daripada model setempat dalam Ollama atau LM Studio, seperti yang dipilih dalam **Seting › Ramalan**; model setempat menyimpan kod anda pada Mac ini.
- **Minimap dan Sticky Scroll** (menu Editor): gambar sempit seluruh fail di sebelah teks, dan baris pengepala blok yang mengandungi baris teratas, disematkan di atasnya.
- **Diagnostik di Hujung Baris** (menu Paparan): mesej paling serius bagi setiap baris selepas berhenti sesaat, dengan simbol bagi tahap keterukannya.
- **Pratonton langsung halaman statik:** Editor › Buka dalam Tab Pelayar menyajikan halaman folder yang dipercayai daripada Dango sendiri dan memuat semula tab apabila fail disimpan.
- **Prettier** memformat JavaScript, TypeScript, CSS, HTML, Markdown dan banyak lagi jika projek menyediakannya, dengan `node_modules/.bin/prettier` milik projek itu sendiri; seting `formatter` memilih pemformat bagi setiap bahasa.
- **Mod Vim** (Editor › Mod Vim): mod Normal, Insert, Visual dan Replace, kiraan, operator dengan gerakan dan objek teks, `.`, register, tanda dan makro. `u` dan ⌃R menggunakan buat asal editor, dan satu sesi mod Insert dikira satu langkah. `"+` dan `"*` ialah papan klip; seting `vimClipboard` menjadikan register tanpa nama juga papan klip. `/`, `?`, `n`, `N`, `*`, `:s` dan `:%s` mencari dengan ungkapan nalar Rust, bukan corak Vim: tiada `\v`, dan `\b` sebagai ganti `\<` dan `\>`. Arahan Ex ialah `:w`, `:q`, `:wq`, `:x`, `:noh` dan `:<line>`. Dalam mod Normal, ⌃G menjadi milik Vim dan bukan lagi pergi ke baris.
- **Penyelarasan seting** melalui folder dalam iCloud Drive (**Seting › Umum**): seting dan pintasan anda mengikut anda ke Mac anda yang lain; laluan, alamat setempat dan persetujuan kekal pada setiap satu.

## Pelayan bahasa

Go, Rust, TypeScript dan JavaScript, Python, C, C++ dan Objective-C, Swift, skrip shell, JSON, YAML, CSS, HTML, TOML, Markdown dan Lua, serta pelayan anda sendiri daripada **Seting › Pelayan bahasa**. Dango memasang pelayan apabila anda memintanya, ke dalam folder alatnya sendiri, dan menggunakan `tsc` milik projek TypeScript sendiri apabila ia boleh berfungsi sebagai pelayan bahasa.

Apa yang dibawanya: diagnostik (⌘' dan ⇧⌘' bergerak antaranya), hover (⌃⌘I), pelengkapan (⌥Esc), bantuan tandatangan, pergi ke definisi (⌃⌘J), definisi jenis dan pelaksanaan, rujukan (⌃⇧⌘F), hierarki panggilan dan hierarki jenis (⌃⇧⌘H), nama semula (⌃⌘E), pemformatan (⌥⇧F), warna semantik, lipatan dan kerangka (⇧⌘O); jenis dan nama parameter di dalam baris; pembetulan pantas dan pemfaktoran semula di sebalik mentol lampu (⌥⌘A); simbol projek dengan ⌥⌘O.

Selain pelayan fail itu sendiri, ESLint menyemak JavaScript dan TypeScript dalam projek yang menggunakannya, dengan pembetulan pantasnya, dan singkatan Emmet muncul dalam pelengkapan untuk HTML, CSS dan JSX. Dango memasang kedua-duanya apabila anda memintanya; **Seting › Umum** membetulkan masalah ESLint semasa menyimpan jika anda mahu.

## Ujian

Panel Ujian menyenaraikan ujian sesebuah folder mengikut pakej dan fail beserta keadaannya: `cargo test` atau cargo-nextest, Go, pytest, Vitest dan Jest. Simbol di sebelah setiap ujian pada jidar menjalankannya, ⌥-klik menyahpepijatnya, dan assert yang gagal menandakan barisnya. Menu Nyahpepijat menjalankan ujian pada kursor (⌃⌥⌘U), ujian terakhir sekali lagi (⌃⌥⌘G) atau kesemuanya, dan menyahpepijat ujian pada kursor. Ujian hanya berjalan dalam folder yang dipercayai, kerana ujian menjalankan kod projek; `cargo test` menyenaraikan ujiannya apabila anda memintanya, kerana penyenaraian membina setiap program ujian.

## Penyahpepijat

Titik henti, juga dengan syarat dan sebagai titik log, langkah demi langkah, thread dengan tindanan panggilannya, pemboleh ubah dan ungkapan pantauan dalam Pemeriksa (⌥⌘I), nilai di sebelah kod semasa program dijeda, dan konsol nyahpepijat yang menilai ungkapan dalam bingkai yang dijeda.

| Bahasa | Penyesuai nyahpepijat |
|---|---|
| Rust, C, C++, Swift | `lldb-dap` daripada Xcode Command Line Tools |
| Go | `dlv` |
| JavaScript, TypeScript | penyahpepijat JavaScript, pada Node.js |
| Python | debugpy |

`.vscode/launch.json` projek akan dibaca. Menu Nyahpepijat menggunakan pintasan Xcode: ⌘\\ menetapkan titik henti, ⌘Y menghidupkan atau mematikan semuanya, ⌃⌘Y meneruskan, F6, F7 dan F8 melakukan langkah lalu, langkah masuk dan langkah keluar, ⌘. menghentikan.

## Terminal

Terminal dalam tab (⌃\`) dengan persekitaran shell log masuk, dilukis dengan Metal. ⌘K mengosongkan skrin. Imej yang ditampal atau dilepaskan diserahkan kepada ejen pengekodan yang berjalan dalam terminal, atau laluan salinan yang disimpan ditampal. ⌘-klik membuka pautan dan nama fail. Pelayan pembangunan yang dimulakan oleh terminal boleh dibuka dalam tab pelayar.

## Pelayar

Tab pelayar dengan medan alamat (⌘L), undur dan maju (⌘[ dan ⌘]), muat semula (⌘R), carian, muat turun yang bertanya dahulu, dan Pemeriksa Web. `@browser` menyerahkan halaman kepada ejen, ditanda sebagai teks dari luar.

## Git

Panel Git melakukan stage dan membuang fail atau hunk tunggal, membuat commit, membuat commit dan push atau segerak, menukar dan mencipta branch, menyimpan stash, menyelesaikan konflik gabung dan melukis graf commit terkini. Blame menunjukkan siapa yang terakhir mengubah baris di bawah kursor, atau satu lajur untuk keseluruhan fail. Satu butang menulis mesej commit dengan AI; diff hanya dihantar selepas gitleaks tidak menemui sebarang rahsia di dalamnya, dan setiap commit juga disemak untuk rahsia.

## CI

GitHub Actions dan GitLab CI, juga GitHub Enterprise Server dan GitLab yang diurus sendiri:

- larian, job dan log bagi branch semasa;
- menjalankan semula job yang gagal, satu job atau, di GitHub, seluruh larian; memulakan job manual atau meluluskan deployment yang menunggu; memulakan pipeline baharu atau workflow yang boleh dimulakan secara manual; membatalkan;
- pull request atau merge request yang terbuka dan issue, yang dibuka dalam tab pelayar;
- log job yang gagal diserahkan kepada ejen, ditanda sebagai teks dari luar.

Token diambil daripada Rantai Kunci, `gh` atau `glab`, persekitaran shell log masuk, atau token yang anda masukkan dalam **Seting › CI**.

## Container

Panel Container menunjukkan projek Compose Docker beserta perkhidmatannya, container tunggal, imej dan volum, serta mengikuti peristiwa Docker sebaik sahaja ia berlaku. Fail Compose dalam folder muncul sebagai projek walaupun sebelum mana-mana containernya wujud. Container boleh dimulakan, dihentikan dan dimulakan semula; lognya dan sebuah shell dibuka dalam tab terminal, dan port yang diterbitkan dibuka dalam tab pelayar. Memadam container, imej atau volum, dan membersihkan data yang tidak digunakan, akan bertanya dahulu. Perintah up, down dan pull Compose hanya berjalan dalam folder yang dipercayai, kerana Compose membina dan menjalankan apa yang diterangkan oleh projek. Menu panel memilih konteks Docker untuk tetingkap tanpa mengubah konteks yang digunakan oleh shell anda; perintah yang sama ada dalam menu Container. Panel ini memerlukan baris perintah `docker`, Docker 24.0.6 atau lebih baharu.

## Markdown

- **Pratonton.** Fail Markdown dibuka dengan pratontonnya di sebelah; **Paparan › Tunjukkan/Sembunyikan Pratonton di Sebelah** (⇧⌘V) menutup dan membukanya, dan **Seting › Umum** mematikan pratonton automatik. Pratonton melukis formula, rajah Mermaid, Graphviz dan PlantUML serta gambar dalam folder; gambar dari web hanya dimuatkan dalam folder yang dipercayai atau selepas anda memintanya dalam pratonton.
- **Menu Format:** tebal (⌘B), condong (⌘I), garis lorek (⇧⌘X), kod (⌥⇧⌘C), pautan (⇧⌘K), tajuk 1–3 (⌥⌘1–⌥⌘3), senarai berbulet dan bernombor, tugasan, petikan dan blok kod. Setiap perintah menogol, pada pilihan atau baris, dan merupakan satu langkah buat asal.
- **Tampal.** Teks daripada Word atau halaman web tiba sebagai Markdown, gambarnya sebagai fail di sebelah dokumen; ⌥⇧⌘V menampal teks biasa.
- **Eksport.** **Fail › Eksport** menulis PDF, satu fail HTML dengan gambarnya di dalam, atau dokumen Word. Gambar dari web hanya disertakan daripada folder yang dipercayai.
- **Buka dalam Splat.** Jika [Splat](https://github.com/sventmueller/splat) dipasang, **Fail › Buka dalam Splat** dan menu konteks pepohon fail serta tab membuka dokumen di sana, selepas menyimpannya.

## Perencanaan

Rencanakan ke dalam medan mesej tab AI atau editor: butang mikrofon di situ, **Edit › Mulakan Perencanaan** (⌃⌥D), atau kekunci fn (tahan untuk merakam, ketik dua kali untuk terus merakam). Pengecaman pertuturan ialah milik [Lowspeak](https://github.com/sventmueller/lowspeak); Lowspeak sendiri tidak perlu dipasang.

- **Seting › Perencanaan** menetapkan mod dan model (dalam talian dengan OpenAI atau Groq, secara setempat dengan Whisper atau Parakeet), bahasa, pascapemprosesan, mikrofon dan perbendaharaan kata anda sendiri.
- **Model setempat** dimuat turun daripada Hugging Face apabila anda memilihnya, dengan kemajuan dan cara untuk membatalkan. Model itu disimpan dalam folder yang digunakan oleh Lowspeak, jadi setiap apl menggunakan apa yang dimuat turun oleh apl yang satu lagi. Model setempat menyimpan rakaman pada Mac ini.
- **Transkripsi dalam talian** memerlukan kunci OpenAI atau Groq, yang disimpan oleh Dango dalam item Rantai Kunci miliknya sendiri.
- **Daripada Lowspeak.** Jika Lowspeak disediakan secara berbeza daripada lalai, tab Perencanaan menawarkan sekali untuk mengambil alih setingnya, dan kuncinya apabila diminta; macOS bertanya sebelum Dango membaca kunci (pilih “Benarkan”, bukan “Sentiasa Benarkan”).
- **Keizinan.** macOS meminta mikrofon pada perencanaan pertama. Kekunci fn berfungsi semasa Dango ialah apl yang aktif dan Lowspeak tidak berjalan; ia memerlukan keizinan Kebolehcapaian, yang diminta oleh Dango selepas perencanaan pertama, tidak pernah semasa pelancaran.

## Kepercayaan

Folder boleh menyebabkan program berjalan sebaik sahaja sesuatu dimulakan di dalamnya: hook Git dan seting repositori seperti `core.fsmonitor`, pelayan bahasa daripada `node_modules`, hook ejen, `.mcp.json`. Kali pertama anda membuka folder, Dango bertanya sama ada anda mempercayainya.

- **Dipercayai:** Git, pelayan bahasa, penyahpepijat, ejen dan CI berjalan untuk folder ini dan semua yang berada di bawahnya.
- **Terhad:** anda boleh membaca dan menyunting; tanda perubahan dalam editor datang daripada membaca `.git` secara terus. Tiada apa-apa yang dikonfigurasikan oleh folder dimulakan. Panel Git, CI dan AI memaklumkan hal ini dan menawarkan untuk mempercayai folder itu.

Teks yang tidak datang daripada anda boleh membawa arahan yang bertujuan untuk mengemudi ejen (prompt injection). Oleh itu, output terminal (`@terminal`), halaman pelayar (`@browser`), log CI yang diserahkan kepada ejen, fail di luar projek dan teks yang lebih panjang yang ditampal daripada halaman web dihantar dengan tanda: cip menyatakan “Kandungan Luaran”, dan ejen menerima teks itu dengan notis bahawa ia ialah data tidak dipercayai yang arahannya tidak boleh diikuti. Halaman web ejen terbina dalam datang dengan notis yang sama. Teks yang ditampal dikira sebagai datang daripada halaman web apabila pelayar menyatakannya pada papan klip, seperti yang dilakukan oleh Safari, Chrome dan Firefox; teks dari tempat lain, dan frasa pendek beberapa perkataan, kekal sebagai milik anda.

## Sandbox

Dango sendiri berjalan tanpa App Sandbox, kerana setiap program yang dimulakannya akan mewarisinya: terminal, Git, pelayan bahasa dan ejen tidak akan dapat berfungsi. Sebaliknya, setiap thread AI mempunyai suis sandbox, mati secara lalai. Apa yang dihadkannya bergantung pada ejen, dan petua alat suis itu menyatakannya:

| Ejen | Dengan sandbox dihidupkan |
|---|---|
| Claude | perintah shell berjalan dalam sandbox Claude sendiri; alat fail dan webnya tidak |
| Codex | perintah dan perubahan hanya menulis dalam projek, tanpa rangkaian |
| Ejen terbina dalam | fail, permintaan web dan perintah kekal dalam projek dan folder yang dibenarkan, tanpa rangkaian (`sandbox-exec`) |
| Antigravity, OpenCode | tiada sandbox yang boleh dihidupkan oleh Dango |

## Penampilan dan kebolehcapaian

- **Mod Gelap** mengikut sistem; **Tingkatkan Kontras** dan **Kurangkan Gerakan** dipatuhi.
- **VoiceOver** menamakan bar sisi, tab, kad dalam garis masa AI, baris Git dan CI, penyahpepijat dan teks terminal. Titik henti tidak pernah dibezakan dengan warna sahaja.
- **Papan kekunci.** Setiap perintah mempunyai pintasan, dan semuanya boleh diubah dalam `keybindings.json`; ⌥⌘\` beralih ke panel seterusnya.
- **Kanan ke kiri.** Dalam bahasa Arab dan Ibrani, tetingkap dicerminkan, manakala paparan kod kekal dari kiri ke kanan.

Apa yang ditambah oleh Dango di atas AppKit, mengikut bidang, terdapat dalam [ACCESSIBILITY.md](../../../ACCESSIBILITY.md).

## Bahasa

Antara muka diterjemahkan ke dalam 35 bahasa: Arab, Katalan, Cina (Ringkas), Cina (Tradisional), Croatia, Czech, Denmark, Belanda, Inggeris, Finland, Perancis, Jerman, Yunani, Ibrani, Hindi, Hungary, Indonesia, Itali, Jepun, Korea, Melayu, Norway, Poland, Portugis (Brazil), Portugis (Portugal), Romania, Rusia, Slovak, Slovenia, Sepanyol, Sweden, Thai, Turki, Ukraine dan Vietnam.

Dango mengikut susunan bahasa dalam **Seting Sistem › Umum › Bahasa & Rantau**; anda juga boleh memilih bahasa untuk Dango sahaja di situ di bawah **Aplikasi**. Tanpa terjemahan untuk bahasa anda, ia menggunakan bahasa Inggeris. Istilahnya mengikut satu glosari, [docs/glossary.md](../../../docs/glossary.md), dan terjemahan Apple sendiri, jadi Dango terasa seperti bahagian lain Mac anda.

## Pintasan papan kekunci

| Tindakan | Pintasan |
|---|---|
| Buka folder, tetingkap baharu, tutup tetingkap | ⌘O, ⇧⌘N, ⇧⌘W |
| Fail baharu, simpan, simpan sebagai | ⌘N, ⌘S, ⇧⌘S |
| Tab baharu, tutup tab, tab seterusnya dan sebelumnya | ⌘T, ⌘W, ⇧⌘] dan ⇧⌘[ |
| Buka Dengan Pantas, cari dalam projek | ⌘P, ⇧⌘F |
| Fail, Carian, Ejen, Git, Ujian, Nyahpepijat, CI, Container | ⌘1 hingga ⌘8 |
| Tunjukkan atau sembunyikan bar sisi, panel seterusnya | ⌃⌘S, ⌥⌘\` |
| Undur dan maju melalui tempat yang disunting | ⌃- dan ⌃⇧- |
| Togol ulasan, pergi ke baris | ⌘/, ⌃G |
| Kemunculan seterusnya, kursor di atas dan di bawah | ⌘D, ⌃⇧↑ dan ⌃⇧↓ |
| Lipat, buka lipatan, lipat semua, buka semua lipatan | ⌥⌘←, ⌥⌘→, ⇧⌥⌘←, ⇧⌥⌘→ |
| Balut baris, Pemeriksa | ⌥Z, ⌥⌘I |
| Pelengkapan, hover, pergi ke definisi | ⌥Esc, ⌃⌘I, ⌃⌘J |
| Rujukan, hierarki panggilan, nama semula | ⌃⇧⌘F, ⌃⇧⌘H, ⌃⌘E |
| Simbol dalam fail, dalam projek | ⇧⌘O, ⌥⌘O |
| Pembetulan pantas, format | ⌥⌘A, ⌥⇧F |
| Masalah seterusnya dan sebelumnya | ⌘' dan ⇧⌘' |
| Pratonton Markdown, tampal sebagai teks biasa | ⇧⌘V, ⌥⇧⌘V |
| Tebal, condong, garis lorek, kod, pautan | ⌘B, ⌘I, ⇧⌘X, ⌥⇧⌘C, ⇧⌘K |
| Tajuk 1 hingga 3 | ⌥⌘1 hingga ⌥⌘3 |
| Terminal baharu, kosongkan terminal | ⌃\`, ⌘K |
| Fokus pada medan mesej ejen, ulas pelan | ⌘Esc, ⌥⌘K |
| Medan alamat, muat semula, undur, maju (pelayar) | ⌘L, ⌘R, ⌘[, ⌘] |
| Mulakan atau hentikan perencanaan | ⌃⌥D |
| Titik henti, hidupkan atau matikan semua titik henti | ⌘\\, ⌘Y |
| Jalankan ujian pada kursor, jalankan semula ujian terakhir | ⌃⌥⌘U, ⌃⌥⌘G |
| Teruskan, langkah lalu, masuk, keluar, henti | ⌃⌘Y, F6, F7, F8, ⌘. |
| Seting | ⌘, |

`keybindings.json` dalam folder data apl (lihat [Lokasi data](#lokasi-data)) mengubah mana-mana pintasan itu.

## Privasi dan keselamatan

- **Tiada telemetri,** tiada analitik, tiada laporan ranap yang dihantar ke mana-mana. Ralat pergi ke log sistem (Console.app, di bawah nama apl).
- **Rangkaian hanya untuk perkara yang anda mulakan:** ejen yang anda gunakan, setiap satu bercakap dengan perkhidmatannya sendiri; registry ACP dan muat turun ejen, pelayan bahasa, penyesuai nyahpepijat dan model perencanaan apabila anda memintanya; hos CI yang anda daftar masuk; ramalan semasa menaip daripada Copilot jika anda memilihnya; penyedia ejen terbina dalam; transkripsi dalam talian jika anda memilihnya; halaman yang anda buka dalam tab pelayar; dan semakan kemas kini sebaik sahaja anda membenarkannya.
- **Kod yang meninggalkan Mac** hanya berbuat demikian ke tempat anda memilih perkhidmatan untuknya: ejen, ramalan Copilot, mesej commit yang ditulis oleh AI. Diff untuk mesej commit hanya dihantar selepas gitleaks tidak menemui sebarang rahsia di dalamnya.
- **Kunci dan token** kekal dalam Rantai Kunci macOS, tidak pernah dalam fail seting. Kunci apl lain, seperti kunci Lowspeak, hanya dibaca apabila anda bersetuju, dan hanya sekali.
- **Muat turun dengan versi yang ditetapkan.** Ejen, pelayan bahasa dan penyesuai nyahpepijat yang dipasang oleh Dango ditetapkan pada versi yang telah diuji dan disemak berbanding checksum masing-masing.
- **Tandatangan kod** dengan Hardened Runtime; kemas kini hanya dipasang dengan tandatangan apl itu sendiri.

## Lokasi data

`<id>` ialah pengecam bundle, `local.agentide` untuk keluaran (`local.agentide.debug` untuk binaan nyahpepijat). Ia tidak mengandungi nama paparan, jadi penukaran nama mengekalkan semua data.

| Tempat | Kandungan |
|---|---|
| `~/Library/Application Support/<id>/` | `settings.json`, `state.json`, `keybindings.json`, `trust.json`; `threads/` (senarai panel AI), `agent-threads/` (perbualan ejen terbina dalam), `layouts/` (tab bagi setiap projek), `recovery/` (editor yang belum disimpan selepas ranap), `checkpoints/` (untuk folder tanpa Git), `worktrees/`, `agents/` dan `tools/` (ejen, pelayan bahasa dan penyesuai nyahpepijat yang dipasang), `task-output/` |
| `~/Library/Caches/<id>/` | imej yang ditampal, skrip integrasi shell, registry ACP; boleh dipadam |
| `~/Library/Logs/<id>/dictation/` | fail log perencanaan |
| `~/Library/Application Support/Lowspeak` dan `FluidAudio` | model perencanaan setempat, dikongsi dengan Lowspeak |
| `~/Library/WebKit/<id>/` | kuki dan data laman web tab pelayar |
| `<project>/.dango/settings.json` | seting projek itu sendiri |
| repositori Git projek | titik semak, sebagai commit di bawah `refs/ide-checkpoints/`; push atau fetch biasa tidak pernah membawanya |
| Rantai Kunci, perkhidmatan `<id>` | kunci API ejen terbina dalam dan perencanaan, token CI |
| `~/.local/bin/dango` | perintah terminal, jika anda memasangnya |

Claude menyimpan sesinya sendiri di bawah `~/.claude/projects`; Dango membacanya untuk menyambung semula sesi tetapi tidak mengubahnya.

## Had yang diketahui

- **Tiada App Sandbox** untuk apl itu sendiri, kerana terminal, Git, pelayan bahasa dan ejen akan mewarisinya; folder mesti dipercayai sebelum apa-apa yang dikonfigurasikannya dijalankan. Antigravity dan OpenCode tiada sandbox yang boleh dihidupkan oleh Dango.
- **Angka penggunaan dan pecahan konteks Claude** bergantung pada bahagian protokolnya yang ditanda sebagai eksperimen, dan disembunyikan apabila bahagian itu tiada.
- **Memutar balik perbualan** hanya berfungsi untuk Claude; bagi ejen lain, hanya kod yang diputar balik.
- **Daftar masuk ke CI melalui pelayar** dimatikan; gunakan `gh`, `glab`, token atau persekitaran.
- **Gemini CLI tidak ditawarkan:** Google telah menamatkan daftar masuk dengan akaun peribadi dan mengarahkan pengguna ke Antigravity. Thread Gemini yang terdahulu kekal boleh dibaca.

## Membina daripada sumber

Anda memerlukan:

- macOS 14 atau lebih baharu dengan Xcode Command Line Tools (`xcode-select --install`); Xcode sendiri tidak diperlukan.
- Rust seperti yang dinyatakan oleh `rust-toolchain.toml`. `rust` daripada Homebrew membina untuk pemproses Mac ini; binaan universal untuk keluaran memerlukan rustup dengan sasaran `aarch64-apple-darwin` dan `x86_64-apple-darwin`.
- Salinan [splat](https://github.com/sventmueller/splat) dan [lowspeak](https://github.com/sventmueller/lowspeak) di sebelah salinan ini (`../splat`, `../lowspeak`), kedua-duanya pada branch `main`; ruang kerja mengambil crate daripada kedua-duanya melalui laluan.
- cmake, untuk setiap binaan dengan perencanaan (`make run`, `make build`, `make release`), yang membina whisper.cpp. `make check` membina tanpanya.

| Perintah | Fungsinya |
|---|---|
| `make run` | membina apl nyahpepijat (`target/debug/Dango Debug.app`) dan membukanya |
| `make build` | membina apl keluaran (`target/release/Dango.app`), ditandatangani dengan sijil setempat yang dicipta oleh binaan pertama dalam rantai kunci log masuk; membiarkan `VERSION` tidak berubah |
| `make lint` | pemformatan dan clippy; amaran menyebabkan kegagalan |
| `make check` | `lint`, kemudian semua ujian; tidak memerlukan rantai kunci, tetingkap atau rangkaian |
| `make test` | `check`, kemudian ujian kendiri `probe` dan audit kebolehcapaian, kiri ke kanan dan kanan ke kiri, dalam apl sebenar |
| `make perf` | bajet masa, dalam binaan keluaran; hanya bermakna pada Mac yang tidak melakukan apa-apa lagi |
| `make release` | larian percubaan keluaran ke dalam `target/release-dry-run/`, ditandatangani dengan sijil setempat; tidak menghantar apa-apa ke mana-mana |
| `make release-notarized` | `check`, kemudian keluaran itu sendiri ke dalam `dist/`: tandatangan Developer ID, notarisasi, suapan kemas kini yang ditandatangani dengan kunci apl itu sendiri; membiarkan `VERSION` tidak berubah |
| `make deploy` | pada `main` yang bersih dengan meterai CI hijau: menotari versi seterusnya, mengemas kini dan melakukan commit `VERSION`, memuat naik fail keluaran dan melakukan push kod, meterai CI serta tag versi; GitHub menerbitkan keluaran secara automatik |

Nyahpepijat dan keluaran ialah apl berasingan dengan seting dan data masing-masing. Pembinaan, pengujian, ujian kendiri, keluaran dan peraturan yang diikuti oleh kod diterangkan dalam [docs/development.md](../../../docs/development.md), keputusan di sebaliknya dalam [docs/plan.md](../../../docs/plan.md). Nota keluaran ialah [RELEASE_NOTES.md](../../../RELEASE_NOTES.md), diterjemahkan di sebelah setiap README dalam `assets/readme/<language>/`.

## Lesen

Dango dilesenkan di bawah mana-mana satu daripada

- Lesen Apache, Versi 2.0 ([LICENSE-APACHE](../../../LICENSE-APACHE))
- Lesen MIT ([LICENSE-MIT](../../../LICENSE-MIT))

mengikut pilihan anda.

Melainkan anda menyatakan sebaliknya secara jelas, sebarang sumbangan yang sengaja dikemukakan untuk dimasukkan ke dalam Dango oleh anda, seperti yang ditakrifkan dalam lesen Apache-2.0, akan dilesenkan dua kali seperti di atas, tanpa sebarang terma atau syarat tambahan.
