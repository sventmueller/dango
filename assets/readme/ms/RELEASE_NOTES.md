## Keselamatan
- Pautan dalam jawapan, rancangan dan laporan ejen, dalam pratonton Markdown dan dalam PDF tidak lagi membuka apa-apa
  apl dengan satu klik. Alamat web dibuka dalam tab pelayar, fail di dalam projek dalam tab editor; selain itu, seperti
  fail di tempat lain, perkongsian rangkaian atau pautan apl lain, akan bertanya dahulu dan menunjukkan ke mana pautan
  itu menuju.
- Alamat daftar masuk dan Remote Control yang diserahkan oleh ejen atau pelayan MCP hanya dibuka jika ia alamat web.
- Folder yang mengandungi jauh lebih banyak daripada satu projek, seperti folder pengguna, Desktop, Dokumen, Muat Turun
  atau akar volum, tidak lagi boleh dipercayai secara keseluruhan; percayai projek di dalamnya satu demi satu. Soalan
  kepercayaan kini menyatakan bahawa kepercayaan itu juga terpakai pada folder di dalamnya.
- Token CI dan daftar masuk tidak sekali-kali dihantar melalui http biasa kepada pelayan selain Mac ini; repositori
  yang remotenya bermula dengan `http://` ditanya melalui https.
- Tanpa gitleaks, ramalan suntingan menunggu sebelum kod dihantar kepada penyedia di luar Mac ini sehingga anda memilih
  untuk menghantarnya tanpa disemak. Seting gitleaks projek itu sendiri tidak lagi mematikan semakan.
- Ejen terbina dalam tidak lagi boleh diarahkan untuk menulis di luar projek melalui pautan yang muncul selepas
  penulisannya diluluskan, dan fail yang mempunyai hard link lain mendapat fail tersendiri dan bukannya ditulis melalui
  pautan itu.
- Pelengkapan daripada pelayan bahasa tidak lagi boleh menyebabkan editor tergantung, dan penamaan semula serta
  pemadaman fail berpautan olehnya bertindak pada pautan itu, bukan pada sasaran yang ditunjuknya.
- Gambar yang anda lampirkan pada prompt dinyahkod oleh perkhidmatan penyahkodan dalam sandbox, seperti di tempat lain.
- Program dalam terminal hanya boleh menggantikan papan klip selagi terminalnya berada di hadapan, dan terminal
  memaklumkannya.
- rust-analyzer dipasang daripada keluaran yang ditetapkan dengan checksumnya, dan bukannya versi terkini pada masa itu.

## Carian dan navigasi
- Buka Dengan Pantas memaklumkan apabila tiada fail yang sepadan dan menunjukkan sama ada ia mencari fail atau simbol
  projek.
- Simbol projek merangkumi TypeScript dan JavaScript tanpa perlu membuka mana-mana failnya. Padanan tepat daripada
  projek dan dalam bahasa yang sedang anda gunakan muncul dahulu; simbol daripada pustaka dan pakej yang dipasang
  ditandakan.
- Paparan › Carian meletakkan fokus dalam medan carian.

## Editor
- Editor mengekalkan fokus apabila fail Markdown dibuka dengan pratontonnya di sebelah, jadi menu Format terus
  berfungsi. Perintah editor juga berfungsi selagi fokus berada dalam bar sisi.
- Cadangan di hujung baris atau dalam kotak di bawahnya memaklumkan bahawa Tab menerimanya.
- Tab yang sama nama dalam gambaran keseluruhan tab kini dapat dibezakan, contohnya fail dan pratontonnya dalam
  pelayar.
- Menu Paparan tidak lagi menawarkan “Sembunyikan Bar Tab” milik macOS, yang tidak digunakan oleh tab Dango.

## Bahasa dan nyahpepijat
- Pelayan JSON kini menyemak fail, dan pelayan yang mendaftarkan diagnostiknya selepas dimulakan, seperti basedpyright
  dan sourcekit-lsp, menunjukkannya.
- Jika pip enggan memasang ke dalam Python, seperti Python daripada Homebrew, penyahpepijat menawarkan untuk memasang
  debugpy ke dalam persekitaran maya projek.
