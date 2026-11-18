Ini ialah keluaran pertama Dango, editor macOS asli untuk bekerja dengan ejen pengekodan. Editor, terminal, pelayar,
Git, CI, penyahpepijat, thread AI dan Software Factory untuk pesanan kerja berkongsi satu tetingkap bagi setiap folder.
Apl ini ditulis dalam Rust di atas AppKit; WebKit hanya digunakan dalam tab pelayar. Antara muka tersedia dalam 35
bahasa.

## Keperluan
- macOS 14 atau lebih baharu, pada Apple silicon atau Intel.
- Untuk thread AI, ejen yang anda pilih: Claude memerlukan perintah `claude` yang telah didaftar masuk; Codex memerlukan
  Node.js, dan apl memasang pakejnya dalam versi yang ditetapkan apabila anda memintanya; Antigravity dan OpenCode
  menggunakan `agy` atau `opencode` anda sendiri, atau muat turun dalam versi yang ditetapkan; ejen terbina dalam
  memerlukan kunci API, atau Ollama atau LM Studio pada Mac ini.
- Pelayan bahasa dan penyesuai nyahpepijat dipasang apabila anda memintanya. gopls dan `dlv` memerlukan Go; pelayan
  untuk TypeScript, Python, JSON, YAML, CSS, HTML, ESLint dan shell serta penyahpepijat JavaScript memerlukan Node.js;
  clangd, sourcekit-lsp dan `lldb-dap` disertakan bersama Xcode Command Line Tools.
- Ramalan semasa menaip memerlukan langganan GitHub Copilot (langganan percuma pun memadai), atau model
  fill-in-the-middle dalam Ollama atau LM Studio.

## Tetingkap
- Setiap fail, terminal, halaman pelayar, thread AI, paparan perbezaan dan pratonton ialah tab macOS dalam tetingkap
  folder. “+” di hujung bar tab dan ⌘T menawarkan fail baharu, terminal, pelayar dan setiap ejen yang dipasang, juga
  dalam worktree Git tersendiri.
- Bar sisi bertukar antara Fail, Carian, Ejen, Git, Nyahpepijat, CI, Ujian, Container dan Software Factory; klik kanan
  menyembunyikan yang tidak anda gunakan. Bar alat memuatkan undur dan maju melalui tempat yang anda sunting (⌃- dan
  ⌃⇧-), branch, Buka Dengan Pantas dan gambaran keseluruhan Aktiviti.
- Tetingkap boleh dipecahkan kepada kawasan yang bersebelahan atau satu di atas yang lain, bersarang sedalam mana yang
  anda mahu, seperti dalam Xcode; setiap kawasan menyimpan dokumennya di belakang satu sama lain dengan bar tab
  tersendiri. “Buka di Sebelah” menunjukkan dokumen kedua di sebelah dokumen pertama.
- Selepas mula semula, atau ranap, setiap tetingkap kembali dengan tab dan kawasannya, dan editor yang mempunyai teks
  belum disimpan kembali bersama teks itu.
- Imej, audio, video, fon dan dokumen Office dibuka dalam tab melalui Quick Look, PDF melalui PDFKit.

## Editor
- Warna sintaks, nombor baris, tanda perubahan berbanding commit terakhir, cari dan ganti, Buka Dengan Pantas (⌘P) dan
  carian projek (⇧⌘F). Warna dan gaya Markdown tetap mengikut walaupun dalam fail beribu-ribu baris tanpa melambatkan
  penaipan.
- Inden automatik, tab atau ruang mengikut `.editorconfig`, pasangan kurungan, togol ulasan, lipatan, pergi ke baris,
  ruang kosong yang kelihatan, balut baris, minimap dan pengepala melekit bagi skop yang melingkungi.
- Beberapa kursor: ⌥-klik, ⌥-seret untuk satu lajur, ⌘D untuk kemunculan seterusnya.
- **Mod Vim:** dihidupkan dalam menu Editor, dengan mod normal, sisip dan visual, gerakan dan operator, register, tanda,
  makro, carian dan perintah Ex; mesejnya muncul dalam bahasa antara muka.
- Singkatan Emmet dalam pelengkapan HTML, CSS dan JSX, dan masalah ESLint dibetulkan semasa menyimpan jika anda mahu.
- Fail mengekalkan pengekodan, tanda tertib bait dan penghujung barisnya. Fail yang berubah pada cakera dimuat semula
  apabila editor tiada teks yang belum disimpan; jika ada, satu bar menawarkan perbezaan dan penggabungan.
- **Ramalan semasa menaip:** semasa anda menaip, cadangan muncul dalam warna kelabu; Tab menerimanya. Cadangan datang
  daripada GitHub Copilot atau daripada model setempat dalam Ollama atau LM Studio, mengikut pilihan anda dalam Seting;
  model setempat menyimpan kod anda pada Mac ini.

## Pelayan Bahasa
Go, Rust, TypeScript dan JavaScript, Python, C, C++ dan Objective-C, Swift, skrip shell, JSON, YAML, CSS, HTML, TOML,
Markdown dan Lua, ESLint di sebelah pelayan TypeScript, serta pelayan anda sendiri daripada Seting. Diagnostik, juga di
hujung baris, hover, pelengkapan, bantuan tandatangan, pergi ke definisi, definisi jenis dan pelaksanaan, rujukan,
hierarki panggilan dan hierarki jenis, nama semula, pemformatan, warna semantik, lipatan dan kerangka; jenis dan nama
parameter di dalam baris; pembetulan pantas dan pemfaktoran semula di sebalik mentol lampu (⌥⌘A); simbol projek dengan
⌥⌘O atau `#` dalam Buka Dengan Pantas. `tsc` milik projek TypeScript sendiri digunakan apabila ia boleh berfungsi
sebagai pelayan bahasa.

## Penyahpepijat
Titik henti, juga dengan syarat, langkah demi langkah, thread dengan tindanan panggilannya, pemboleh ubah dan ungkapan
pantauan dalam Pemeriksa (⌥⌘I), nilai di sebelah kod semasa program dijeda, dan konsol nyahpepijat yang menilai ungkapan
dalam bingkai yang dijeda. Rust, C, C++ dan Swift melalui `lldb-dap`, Go melalui `dlv`, JavaScript dan TypeScript
melalui penyahpepijat JavaScript, Python melalui debugpy. `.vscode/launch.json` projek akan dibaca. Menu Nyahpepijat
menggunakan pintasan Xcode.

## Ujian dan Container
- Panel Ujian menyenaraikan ujian projek Rust, Go, Python, Jest, Vitest dan Bun sebelum larian pertama, menjalankan satu
  ujian, satu fail atau semuanya, menunjukkan setiap keputusan secara langsung dan menandakannya di jidar editor.
- Panel Container menyenaraikan container, imej, volum dan projek Compose Docker; ia memulakan, menghentikan dan
  mengalih keluar semuanya, mengikuti log masing-masing dan membuka shell dalam tab terminal.

## Terminal
Terminal dalam tab dengan persekitaran shell log masuk, dilukis dengan Metal. Imej yang ditampal atau dilepaskan
diserahkan kepada ejen pengekodan yang berjalan dalam terminal, atau laluan salinan yang disimpan ditampal. ⌘-klik
membuka pautan dan nama fail. Pelayan pembangunan yang dimulakan oleh terminal boleh dibuka dalam tab pelayar.

## Markdown
Fail Markdown dibuka dengan pratontonnya di sebelah (⇧⌘V), dengan formula, rajah Mermaid, Graphviz dan PlantUML serta
gambar dalam folder. Menu Format untuk tebal, condong, pautan, tajuk, senarai, petikan dan blok kod; teks yang ditampal
daripada Word atau halaman web tiba sebagai Markdown. **Fail › Eksport** menulis PDF, satu fail HTML atau dokumen Word.
Jika Splat dipasang, **Fail › Buka dalam Splat** membuka dokumen itu di sana.

## Pelayar
Tab pelayar dengan medan alamat, undur dan maju, carian, muat turun yang bertanya dahulu, dan Pemeriksa Web.

## Git
Panel Git dengan stage dan buang fail atau hunk tunggal, commit, commit dan push atau segerak, branch, stash, konflik
gabung, dan graf commit terkini. Blame menunjukkan siapa yang terakhir mengubah baris di bawah kursor, atau satu lajur
untuk keseluruhan fail. Satu butang menulis mesej commit dengan AI; diff hanya dihantar selepas gitleaks tidak menemui
sebarang rahsia di dalamnya, dan setiap commit juga disemak untuk rahsia. Thread AI boleh bekerja dalam worktree Git
tersendiri; panel menunjukkan apa yang diubahnya dan mengambil alih kerja itu melalui gabung atau cherry-pick.

## CI
GitHub Actions dan GitLab CI, juga GitHub Enterprise Server dan GitLab yang diurus sendiri: larian, job dan log bagi
branch semasa; menjalankan semula job yang gagal, satu job atau (di GitHub) seluruh larian, memulakan job manual atau
meluluskan deployment yang menunggu, memulakan pipeline baharu atau workflow yang boleh dimulakan secara manual, dan
membatalkan. Merge request atau pull request yang terbuka dan issue disenaraikan dalam panel yang sama dan dibuka dalam
tab pelayar. Log job yang gagal boleh dihantar kepada ejen, ditanda sebagai kandungan luaran. Token diambil daripada
Rantai Kunci, `gh` atau `glab`, persekitaran shell log masuk, atau token yang anda masukkan.

## AI
Setiap ejen bekerja dalam tab AI dengan garis masa yang sama: jawapan, panggilan alat dengan outputnya, permintaan
keizinan, soalan, pelan untuk diluluskan dan diulas, senarai tugasan, subejen dan tugas latar belakang; dua butang dalam
pengepala tab menyenaraikan subejen dan proses latar belakang yang sedang berjalan. Sebelum setiap prompt, apl menyimpan
titik semak; perubahan boleh disemak dan diputar balik bagi setiap mesej. Thread baharu bermula dalam mod “Manual”, dan
perintah Git yang merosakkan tidak pernah dibenarkan secara kekal. Gambaran keseluruhan Aktiviti menyenaraikan semua
yang sedang berjalan. Titik hijau pada tab menunjukkan sesi yang sedang bekerja, titik merah sesi yang menunggu anda,
dan pemberitahuan memaklumkan apabila sesi selesai atau menunggu semasa anda melihat di tempat lain. Garis masa, medan
mesej dan baris status mempunyai saiz teks tersendiri dalam Seting.
- **Claude** melalui perintah `claude` tanpa diubah, dengan sesi, subejen, tugas latar belakang, konteks, penggunaan dan
  Remote Control daripada telefon.
- **Codex** melalui pelayan aplnya dan **OpenCode** melalui pelayannya, masing-masing dalam protokolnya sendiri, dengan
  thread mereka, putar balik dalam ejen itu sendiri, dan Remote Control untuk Codex.
- **Ejen ACP:** Antigravity, serta lebih banyak lagi daripada registry ACP apabila anda memintanya; ejen yang dipasang
  ditetapkan pada versi yang telah diuji dan disemak berbanding checksum masing-masing.
- **Ejen terbina dalam:** Anthropic, OpenAI, Google Gemini, OpenRouter, Ollama, LM Studio dan pelayan yang menggunakan
  API OpenAI atau Anthropic, dengan alat untuk fail, carian, perintah, diagnostik dan permintaan web; kunci kekal dalam
  Rantai Kunci.
- Perintah ejen itu sendiri dibuka sebagai paparan tindih pada tab, sama bagi setiap ejen: status, eksport, pelayan MCP,
  keizinan, hook, memori, konfigurasi, skill, plugin, model, akaun dan maklum balas.
- Suis sandbox bagi setiap thread mengehadkan apa yang boleh dilakukan oleh perintah ejen, setakat yang disokong oleh
  ejen itu.
- Kandungan luaran, seperti output terminal, halaman web atau log CI, dihantar kepada ejen dengan tanda tidak
  dipercayai.

## Software Factory
Pesanan kerja dan bukan prompt tunggal: matlamat dengan kriteria penerimaan, ditulis sendiri atau diambil daripada
issue. Setiap pesanan dirancang, dibina dan disemak dalam worktree Git tersendiri, oleh ejen daripada vendor yang
berbeza dalam peranan tetap: perancang menulis spesifikasi, pembina melakukan kerja, dan penyemak daripada vendor lain
menyemak setiap kriteria. Perintah gerbang projek, seperti binaan dan ujian, berjalan dalam sandbox, dan pembina
membetulkan apa yang ditemuinya.
- Anda membuat keputusan pada dua titik: anda meluluskan spesifikasi, dan anda menerima hasilnya bersama buktinya,
  secara keseluruhan atau bahagian demi bahagian. Tiada apa-apa yang diambil alih tanpa penerimaan anda.
- Panel dan tab Software Factory menunjukkan setiap pesanan bersama peringkat, kos dan apa yang ditunggunya; tab sesuatu
  pesanan menunjukkan garis masa, bukti dan langkah seterusnya, serta membuka sesinya yang sedang berjalan sebagai tab
  AI.
- Bajet setiap pesanan dan setiap hari, had giliran, jeda semasa keluar, tidur dan bateri lemah, serta log yang ditulis
  sebelum setiap kesan, supaya pesanan disambung semula selepas mula semula.
- Issue dengan label yang dipilih boleh masuk sebagai pesanan; pesanan yang diterima boleh keluar sebagai merge request
  dengan ulasan pada issuenya, apabila anda mengklik. Pesanan dengan skop laluan yang berasingan berjalan serentak dan
  diambil alih satu demi satu.
- Metrik setiap ejen dan vendor, penghalaan mengikut kadar lulus pertama, cadangan peraturan untuk `AGENTS.md` dan
  `CLAUDE.md` daripada dapatan yang berulang, dan pesanan kerja berulang daripada fail aliran kerja projek.
- Fail aliran kerja folder, `.dango/factory.toml`, memuatkan gerbang, kelas risiko dan skop laluannya; perintah gerbang
  yang diubah dan apa-apa yang melonggarkan batasan menunggu pengesahan anda.

## Perencanaan
Perencanaan ke dalam medan mesej AI dan editor melalui pengecaman pertuturan Lowspeak: butang mikrofon, ⌃⌥D, atau
kekunci fn semasa Dango ialah apl yang aktif. Ia menggunakan seting dan model Lowspeak; transkripsi dalam talian
memerlukan kunci OpenAI atau Groq.

## Kemas Kini
Apl mengemas kini dirinya melalui Sparkle. Pada pelancaran kedua, apl bertanya sama ada ia boleh menyemak kemas kini
sendiri; selagi anda belum bersetuju, ia tidak membuat sebarang permintaan. **Dango › Semak Kemas Kini…** menyemak
serta-merta.

## Kebolehcapaian
Nama VoiceOver untuk bar sisi, tab, kad dalam garis masa AI, baris Git dan CI, Software Factory, penyahpepijat dan teks
terminal; kawalan papan kekunci penuh dengan pintasan yang boleh anda ubah dalam `keybindings.json`; Tingkatkan Kontras
dan Kurangkan Gerakan; paparan kod kekal dari kiri ke kanan dalam antara muka dari kanan ke kiri. Butiran ada dalam
ACCESSIBILITY.md.

## Privasi
Tiada telemetri, tiada analitik dan tiada laporan ranap. Apl hanya menyambung untuk perkara yang anda mulakan: ejen yang
anda gunakan, muat turun yang anda minta, hos CI yang anda daftar masuk, ramalan daripada Copilot jika anda memilihnya,
transkripsi dalam talian jika anda memilihnya, halaman yang anda buka, dan semakan kemas kini sebaik sahaja anda
membenarkannya. Kunci dan token kekal dalam Rantai Kunci macOS, dan ejen Software Factory tidak pernah melihatnya.

## Had Yang Diketahui
- Daftar masuk ke CI melalui pelayar dimatikan; gunakan `gh`, `glab`, token atau persekitaran.
- Apl berjalan tanpa App Sandbox, kerana terminal, Git, pelayan bahasa dan ejen akan mewarisinya; folder mesti
  dipercayai sebelum apa-apa yang dikonfigurasikannya dijalankan. Antigravity dan OpenCode tiada sandbox yang boleh
  dihidupkan oleh apl.
- Dalam Software Factory, pembina atau penyemak Codex boleh membaca di luar worktreenya, kerana protokol Codex tiada
  batasan bacaan; penulisannya kekal di dalam.
- Angka penggunaan dan pecahan konteks Claude bergantung pada bahagian protokolnya yang ditanda sebagai eksperimen, dan
  disembunyikan apabila bahagian itu tiada. Memutar balik perbualan berfungsi untuk Claude, Codex dan OpenCode; bagi
  ejen lain, hanya kod yang diputar balik.
- Gemini CLI tidak ditawarkan: Google telah menamatkan daftar masuk dengan akaun peribadi dan mengarahkan pengguna ke
  Antigravity. Thread Gemini yang terdahulu kekal boleh dibaca.
