## Kelajuan dan memori
- Semua yang dimulakan oleh ejen, dalam apa-apa bahasa, seperti build, ujian, shell dan pelayan MCP, berjalan dalam
  keadaan latar belakang macOS: keutamaan paling rendah, dengan akses cakera dan rangkaian dihadkan. Ejen itu sendiri,
  bersama subejen Claude, berjalan pada keutamaan sederhana. Ejen yang bekerja dengan beban penuh tidak lagi
  memperlahankan Dango; ini juga terpakai pada ejen Software Factory.
- Tetingkap yang dipulihkan hanya memulakan apa yang kelihatan: tab AI yang berada di belakang tab lain memulakan
  CLI-nya apabila ia dibawa ke hadapan atau menerima prompt, dan tab pelayar, pratonton laman dan terminal memuatkan
  halamannya atau memulakan shellnya apabila tab itu mula-mula ditunjukkan.
- Carian projek yang panjang tidak lagi melukis semula semua hasilnya selagi carian itu berjalan. Software Factory, log
  CI dan panel Ujian kurang bekerja selagi tidak kelihatan; ujian dan konfigurasi nyahpepijat dibaca apabila panelnya
  mula-mula ditunjukkan, dan panel CI dan Container yang tersembunyi tidak membaca apa-apa sehingga panel itu
  ditunjukkan.
- Sesi yang panjang memerlukan kurang memori: diff yang selesai menyimpan hanya baris yang ditunjukkan oleh kadnya,
  permintaan keizinan yang telah dijawab melepaskan inputnya, cache glif terminal tidak lagi membesar tanpa had, dan
  panel Fail dan Git melepaskan baris yang sudah tiada.
- Program itu sendiri kira-kira 40% lebih kecil.

## Tetingkap dan menu
- Menutup kawasan memindahkan tabnya ke kawasan sebelumnya, atau jika tiada, ke kawasan selepasnya, dan bukannya menutup
  tab itu. Hanya kawasan pratonton dan kawasan terakhir ditutup bersama tabnya.
- Menu Paparan menyenaraikan panel mengikut susunan simbol pada bar sisi.
- “Urus Ejen…” tidak lagi berada dalam menu +; ia kekal di bawah “+ Sesi Baharu” dalam panel Ejen.

## Pembetulan
- “Sertakan dan Kecualikan Fail” dalam panel carian tidak lagi menunjukkan huruf tersasar di sebelah anak panahnya.
- Menutup popover ulasan bagi spesifikasi tidak lagi menyebabkan ranap.
