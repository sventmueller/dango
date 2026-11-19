## AI sekmeleri
- Üç dakikadır görünmeyen bir AI sekmesi, oturum boştayken Claude işlemini bırakır. Konuşma ve yazdıklarınız sekmede
  kalır; bir sonraki istem oturumu sürdürür. Claude çalışırken, bir istem beklerken, açık bir soru varken, bir alt ajan,
  arka plan görevi ya da komut hâlâ çalışırken veya Remote Control açıkken sekme işlemini korur.
- Bir ajan sonlandırıldığında, kendi işlem grubunda ya da kendi oturumunda başlattığı komutlar da sonlandırılır; bu
  komutlar daha önce ajandan sonra da çalışmaya devam edebiliyordu.
- Zaman çizelgesi, önceki tüm satırları yeniden dizinlemek yerine yeni girdileri sonuna ekler.

## Hız
- Geri yüklenen pencereler daha hızlı açılır: kaydedilmemiş değişikliği olmayan bir dosya sekmesi, dosyasını ancak öne
  geldiğinde yükler.
- Paneli gizli olan bir proje araması duraklar ve panel göründüğünde devam eder.
- Düzenleyicinin Git değişiklik işaretleri her dosya için aynı anda en fazla bir karşılaştırma çalıştırır; bu sırada
  yapılan düzenlemeler tek bir sonraki karşılaştırmada birleştirilir ve dosyanın commit’lenmiş metni yalnızca commit
  değiştiğinde yeniden okunur.
- Katlanmış kod içeren bir dosyanın Mini Haritası, satır başına bir girdi yerine katlanmamış her bölüm için bir girdi
  tutar.

## Arama
- Filtrelerin oku, üstündeki Değiştir okuyla aynı hizada duruyor.
