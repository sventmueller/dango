## Bellek
- Uzun ajan oturumları çok daha az bellek kullanıyor:
  - Geçmiş bir oturum açılırken dökümü satır satır okunur; 100 MB’lık dökümü olan bir oturum artık en fazla 65 MB yerine
    yaklaşık 20 MB’a çıkar.
  - Tamamlanan bir araç çağrısı yalnızca kartının gösterdiğini tutar: çıktısının ilk 400 satırını; kart alanlarını
    gösterdikten sonra ham girdiyi artık tutmaz. “N satır daha” yine her satırı sayar.
  - 30 saniye boyunca görünmeyen bir AI sekmesi kartlarını bırakır ve yeniden göründüğünde görünür olanları kaldığı
    yerde oluşturur.
  - Bir sekme kapandıktan veya geçmiş bir oturum açıldıktan sonra Dango boşalttığı belleği macOS’e geri verir.
- Listeler yenilendikçe artık bellekte büyümüyor. macOS 27’de açılabilen satırları olan bir listenin her yenilenmesi
  geride kalıcı olarak bir açma üçgeni bırakıyordu ve her yenileme her satırdan biraz geride bırakıyordu:
  - CI, Container’lar, Software Factory, Testler, Hata Ayıkla, Git ve Arama listeleri ile çağrı hiyerarşisi kendi
    oklarını çiziyor.
  - Görünmeyen listeler artık yenilenmiyor; göründüklerinde aradaki farkı kapatıyorlar.
  - Oturumlar, CI, Software Factory ve Container’lar listeleri değişmeyen satırları koruyor; Git ise yalnızca durumu
    değişen dosyaların satırlarını yeniliyor.
- Kapanan bir pencerenin panelleri onunla birlikte serbest bırakılır.

## AI sekmeleri
- Çıktısını kendisi bir dosyaya yazan bir arka plan komutunda, örneğin `make > build.log 2>&1`, “Çıktıyı Göster” CLI’da
  çıktıdan hiçbir şey olmadığında o dosyayı gösterir.
