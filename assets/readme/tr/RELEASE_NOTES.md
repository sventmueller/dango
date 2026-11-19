## AI sekmeleri
- Bir arka plan komutunun “Çıktıyı Göster” düğmesi, o anki çıktının bir kopyası yerine, komut çalıştığı sürece çıktıyı
  izleyen bir sekme açar. Sekme çıktının sonunda açılır ve siz yukarı kaydırana kadar yeni satırlar geldikçe orada
  kalır; yeniden sona kaydırdığınızda çıktıyı yine izler. Zaten açık olan bir sekme için “Çıktıyı Göster”, sekmeyi öne
  getirir ve çıktının sonunu gösterir.
  - Çıktısını kendisi bir dosyaya yazan bir komut, örneğin `make > build.log 2>&1`, bu dosyayı büyüdükçe gösterir.
  - Komut sona erdiğinde sekme tüm çıktısını gösterir; “Düzenleyicide Aç”, sekmenin okuduğu dosyayı açar.
  - Bir sekme çıktıyı yalnızca öndeyken okur.
