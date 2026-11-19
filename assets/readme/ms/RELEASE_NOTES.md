## Memori
- Sesi ejen yang panjang kini memerlukan jauh kurang memori:
  - Membuka sesi lalu membaca transkripnya satu baris pada satu masa; sesi dengan transkrip 100 MB kini memuncak pada
    kira-kira 20 MB dan bukannya 65 MB.
  - Panggilan alat yang telah selesai hanya menyimpan apa yang ditunjukkan oleh kadnya: 400 baris pertama outputnya, dan
    tidak lagi input mentah sebaik sahaja kad menunjukkan medannya. “N baris lagi” masih mengira setiap baris.
  - Tab AI yang tidak kelihatan selama 30 saat melepaskan kadnya, dan apabila ditunjukkan semula ia membina kad yang
    kelihatan, di tempat ia berada.
  - Selepas tab ditutup atau sesi lalu dibuka, Dango memulangkan memori yang dibebaskannya kepada macOS.
- Senarai tidak lagi membesar dalam memori apabila dimuat semula. Pada macOS 27, setiap muat semula senarai dengan baris
  yang boleh dibuka meninggalkan segi tiga pendedahan secara kekal, dan setiap muat semula meninggalkan sedikit daripada
  setiap baris:
  - Senarai CI, Container, Software Factory, Ujian, Nyahpepijat, Git dan Carian serta hierarki panggilan melukis anak
    panahnya sendiri.
  - Senarai yang tidak kelihatan tidak lagi dimuat semula; senarai itu menyusul apabila ditunjukkan.
  - Senarai Sesi, CI, Software Factory dan Container mengekalkan baris yang tidak berubah, dan Git hanya memuat semula
    baris fail yang keadaannya berubah.
- Panel tetingkap yang ditutup dilepaskan bersamanya.

## Tab AI
- “Tunjukkan Output” bagi perintah latar belakang yang menulis outputnya sendiri ke fail, seperti
  `make > build.log 2>&1`, menunjukkan fail itu apabila CLI tidak mempunyai sebarang output.
