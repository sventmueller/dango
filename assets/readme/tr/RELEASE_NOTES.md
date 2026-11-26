## Terminal
- Terminal artık Dango’nun kardeş terminal uygulaması Tapa’nın terminalidir ve onun getirdiği her şeyi sunar; Dango ise terminalinin önceden yaptıklarını korur: ajanlara görüntü yapıştırma, tarayıcı sekmesinde geliştirme sunucuları, ⌘-tıklamayla düzenleyiciye geçme.
- **Görüntü › Giriş Alanını Göster/Gizle** (bir terminal odaktayken ⌘I) komut satırını, üstünde klasör ve Git dalı bulunan bir metin alanına dönüştürür. Komutlar parçalarına göre renklendirilir ve kabuğunuzun çalıştıramadığı bir komutun altı kesik kırmızı çizgiyle çizilir.
- Öneriler, alanda da onsuz da, siz yazarken gri olarak görünür: geçmişinizden, 700’den fazla komut satırı aracının tanımlarından ve git dalları, `package.json` betikleri ile make, just ve cargo hedefleri gibi projeden. → birini kabul eder; Tab tamamlar ve birden çok aday varsa yazdıkça süzülen bir menü açar. Öneriler asla ağı kullanmaz.
- Komut blokları: ⌘↑ ve ⌘↓ istemler arasında atlar, **Düzen** menüsü bir komutun çıktısını seçer, kopyalar ve kaydeder, yer işaretleri de satırları işaretler. **Komutları blok olarak işaretle** açıkken bir çubuk her bloğu gösterir, başarısız komutlar işaretlenir ve bir bloğun üzerindeki düğmeler bloğun durumunu ve süresini gösterir.
- **Bloğu Ajana Gönder** (Düzen menüsü ve terminalin bağlam menüsü), geçerli bloğun komut satırı ve çıktısı Dış İçerik olarak eklenmiş bir AI sekmesi açar.
- **Bul** (bir terminal odaktayken ⌘F) tüm geçmişte düzenli ifadelerle, büyük/küçük harf ve tam sözcük seçenekleriyle arar; kaydırma çubuğu sonuçları işaretler.
- Bir programın istediği bildirim (OSC 9 ve OSC 777) ve programın zili, siz terminali görmezken terminalin sekmesine kırmızı noktayı koyar ve önde başka bir uygulama varken bildirim macOS’e gider. Bir programın bildirdiği ilerleme (OSC 9;4) sekmesinde bir halka ve Dock simgesinde bir çubuk olarak görünür.
- Çift tıklama bir URL’yi, bir yolu ya da bir hash’i bütün olarak seçer, ⌥ ile sürükleme bir dikdörtgen seçer, **Biçimlendirmeyle Kopyala** renkleri ve stilleri korur ve **Klavyeyle Seç** bir seçimi Vim tuşlarıyla taşır.
- Hemen çalışacak çok satırlı yapıştırmalar önce gösterilir ve **Dango › Güvenli Klavye Girişi** bir terminale yazdıklarınızı başka uygulamaların okumasını engeller.
- Web adresleri, `mailto:` ve dosyalar dışındaki türden bağlantılar artık terminalden açılmaz.

## Ayarlar
- **Ayarlar › Terminal** yeni terminaller için giriş alanını, onun Vim tuşlarını, önerileri, komutların renklerini, bilinmeyenlerin alt çizgisini, komut bloklarını, seçimin hemen kopyalanmasını, çok satırlı yapıştırmaların önce gösterilmesini ve Genel’den buraya taşınan, ⌥ tuşunun Meta tuşu olarak kullanılmasını açıp kapatır.
