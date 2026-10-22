# OctopusOn GEO-SEO Platform kılavuzu

GEO-SEO Platform, OctopusOn SEO'nun hepsi bir arada SEO ve GEO (yapay zekâ yanıtlarında görünürlük) programıdır. Tek yerde şunları sunar:
- alan adı genel bakış
- anahtar kelime araştırması
- backlink'ler
- boşluk analizi
- sıralama takibi
- site denetimi
- AI görünürlüğü
- Search Console
- beyaz etiketli raporlar

İki türlü kullanılır:
- **Masaüstü programı:** verileriniz bilgisayarınızdadır. OctopusOn SEO hesabınızı bağlarsanız projeleriniz web sürümüyle kendiliğinden eşitlenir (bkz. 13. bölüm).
- **Web sürümü (app.octopuson.com):** OctopusOn SEO hesabınızla girersiniz; verileriniz hesabınıza özel alanda saklanır.

Teknik ayrıntılar [platform.md](platform.md) dosyasındadır.

> Bu sürümde **Pano**, **Alan Adı Genel Bakış**, **Anahtar Kelimeler**, **Backlink'ler**, **Boşluk Analizi**, **Sıralama Takibi**, **Site Denetimi**, **AI Görünürlüğü**, **Search Console**, **Raporlar**, **Kaynaklar** ve **Ayarlar** hazırdır. Diğer modüller kenar çubuğunda "Yakında" olarak görünür ve ilk sürümde eklenir.

## 1. Proje oluşturun

**Pano**'da **Yeni proje**'ye basın:
- **Alan adı:** `ornek.com` gibi yazın. Adres yapıştırırsanız (`https://www.ornek.com/tr/`) alan adı otomatik alınır.
- **Ülke ve dil:** verilerin hangi pazar için getirileceğini belirler.
- **Rakipler:** her satıra bir alan adı yazın, en fazla 10.
- **Marka adları:** markanızın yazılışlarını virgülle ayırarak girin. AI görünürlüğü modülü bunları yapay zekâ yanıtlarında arar.

## 2. Veri kaynağını seçin

Ücretli veriler (DataForSEO) iki yoldan biriyle kullanılır. Bu seçim **Kaynaklar** sayfasındadır.

- **Kendi anahtarım:**
  1. DataForSEO hesabınızın API girişini ve şifresini `login:password` biçiminde girin.
  2. İstekler doğrudan DataForSEO'ya gider ve ücreti DataForSEO hesabınızdan düşer.
- **OctopusOn kredisi:**
  1. Masaüstünde **Tek tıkla bağlan**'a basın. Tarayıcıda my.octopuson.com'a girmişseniz bağlantı hemen kurulur. Web sürümü zaten hesabınızla açılır.
  2. Kredi my.octopuson.com'dan yüklenir.
- **Otomatik (varsayılan):** anahtarınız varsa onu, yoksa krediyi kullanır.

**Yapay zekâ asistanlarının anahtarları:** Kaynaklar sayfasında OpenAI (ChatGPT), Anthropic (Claude), Google Gemini ve Perplexity kartları da vardır. Bunlar yalnızca **AI Görünürlüğü** içindir ve kredisi yoktur:
- Anahtarını girdiğiniz asistan doğrudan sizin hesabınızdan sorulur; ücreti o sağlayıcı hesabınızdan düşer.
- Anahtar girmezseniz AI Görünürlüğü o asistanı DataForSEO üzerinden sorar (kendi DataForSEO anahtarınız ya da kredi).

Kaynak kartında şunlar da var:
- **Aylık bütçe:** bütçeyi aşacak istek yapılmaz.
- **Bu ay:** o ayki harcamanız.
- **Aylık harcama:** sayfanın altındaki tablo; her kaynak ve ödeme şekli için ay ay istek sayısını ve tutarı gösterir.

## 3. Alan adına genel bakış

**Alan Adı Genel Bakış** sayfası projenin alan adını gösterir. Arama kutusuna bir rakibi yazıp **İncele**'ye basarak onun verilerine de bakabilirsiniz.

- **Verileri getir:** düğmenin yanında tahmini maliyet görünür (krediyle "≈ 16 kredi" gibi, kendi anahtarınızla dolar karşılığı).
- **Saklanan veriler:**
  - Getirilen veriler saklanır; sayfayı tekrar açmak ücretsizdir.
  - Alan adı verileri 7 gün güncel sayılır. Süre dolunca **Eski** etiketi çıkar, **Yenile** ile yeniden getirebilirsiniz.
- **Gösterilenler:**
  - organik anahtar kelime sayısı
  - aylık organik trafik ve trafik değeri
  - ücretli anahtar kelimeler
  - sıra dağılımı (1., 2–3, 4–10, 11–20, 21–100)
  - son güncellemeden bu yana yeni, yükselen, düşen ve kaybedilen kelimeler

Kaynak yoksa sayfa bunu söyler ve **Kaynaklara git** düğmesini gösterir.

Sayfanın sekmeleri:
- **Özet:**
  - Sıralama özeti.
  - **Trafik geçmişi:** son 24 ayın aylık organik trafiği ve kelime sayısı, grafik olarak.
- **Anahtar kelimeler:**
  - Alan adının sıralandığı kelimeler, en çok trafik getirenler önce. Her kelimede sıra ve değişimi, hacim, KD (zorluk), TBM, trafik, niyet ve sıralanan adres görünür.
  - Üstte, yüklenen kelimelerde en sık görülen sonuç sayfası öğeleri çıkar (AI Overview, diğer sorular, görseller…).
  - Kelime ya da adres arayabilir; sıra (ilk 3, ilk 10…), niyet ve en az hacme göre süzebilirsiniz. Arama büyük-küçük harf ve Türkçe karakter farkını gözetmez: "ayakkabi" yazınca "ayakkabı" da bulunur.
  - **CSV indir** süzülmüş satırları Excel'in açtığı biçimde kaydeder.
- **Sayfalar:** organik trafiği en yüksek sayfalar; ilk 3 ve ilk 10'daki kelime sayılarıyla.
- **Rakipler:**
  - **Organik rakipler:** aynı kelimelerde sıralanan alan adları, en çok ortak kelimesi olan önce. Wikipedia gibi her sitenin karşılaştığı büyük siteler çıkarılır. Her satırda **İncele** (o alan adına bakmak için) ve **Projeye ekle** vardır.
  - **Trafik karşılaştırması:** projede rakip varsa, alan adınızla rakiplerin aylık trafiğini yan yana gösterir.
- **Alt alan adları:** alt alan adlarının organik trafiği.

Tablolu sekmelerde **100 / 500 / 1000** satır seçilir; maliyet satır sayısıyla artar. Süzme, sıralama ve CSV indirme, getirilmiş satırlar üzerinde yapılır ve ücretsizdir.

## 4. Anahtar kelimeler

Üstteki kutuya bir kelime yazıp **Enter**'a basın. Çok kelimeyi birden ölçmek için her satıra bir kelime yazın (en fazla 700); satır eklemek için **Shift+Enter**.

- **Genel bakış:**
  - **Tek kelimede:** aylık arama, zorluk (KD), TBM, reklam rekabeti, niyet, son 12 ayın grafiği ve değişimi.
  - **Sonuç sayfası öğeleri:** AI Overview, öne çıkan snippet, diğer sorular… İlk 10'daki sayfaların ortalama kaç atıf yapan alan adı olduğu da gösterilir.
  - **Çok kelimede:** bir tablo.
  - **Otomatik tamamlama:** altta Google ve Yandex önerileri ücretsiz gelir; birine tıklayınca o kelime incelenir.
- **Fikirler:**
  - **Eşleşen:** kelimeyi içeren aramalar.
  - **Sorular:** bunların soru biçimlileri ("nasıl", "nedir", "mı"…).
  - **İlgili:** Google'ın ilgili aramaları.
  - **Geniş fikirler:** aynı konudaki kelimeler.
  - Arama, niyet, en az hacim ve en fazla KD ile süzülür. Kelimeleri seçip bir listeye ekleyebilir, CSV indirebilirsiniz.
- **Sonuç sayfası:** Google'ın şu anki ilk 10'u ve sayfadaki öğeler; masaüstü ya da mobil.
- **Yandex Wordstat:** Türkiye ve Yandex'in güçlü olduğu ülkelerde görünür. Yandex'te kelimeyle yapılan en çok aramaları gösterir. Yandex Cloud anahtarınız (API anahtarı ve klasör kimliği) ya da krediyle çalışır; anahtar **Kaynaklar** sayfasından girilir.
- **Listeler:**
  - Kelimeler eklendikleri andaki sayılarıyla saklanır.
  - **Konulara ayır**, listeyi tek bir sayfanın hedefleyebileceği kelime gruplarına böler. Ücretsizdir; Türkçe ekleri tanır ("kahvesi", "kahve" aynı köktür). Soru biçimli kelimeler genelde ayrı bir konu olur; ayrı bir rehber sayfası için iyi bir işarettir.

## 5. Backlink'ler

Üstte projenin alan adı vardır. Bir rakibin alan adını ya da tek bir sayfanın adresini yazıp **İncele**'ye basabilirsiniz.

- **Özet:**
  - **OA (Octopus Authority, 0–100):** sitenin aldığı bağlantıların gücü.
  - Backlink ve atıf yapan alan adı sayıları, dofollow oranı, kırık bağlantılar.
  - Bağlantıların ülke, alan adı uzantısı, site türü ve bağlantı türüne göre dağılımı.
  - Son 12 ayda kazanılan ve kaybedilen alan adları.
- **Backlink'ler:**
  - Bağlantı veren sayfalar, en güçlüsü önce.
  - **Canlı / Yeni / Kaybedilen / Kırık** seçilebilir. **Kırık**, artık olmayan sayfalarınıza giden bağlantılardır; bu sayfaları yönlendirerek bağlantıları geri kazanabilirsiniz.
  - "Yalnızca dofollow" ve "Her siteden bir bağlantı" ile süzülür.
- **Atıf yapan alan adları:** size bağlantı veren siteler, OA'larıyla.
- **Anchor'lar:** bağlantılarda kullanılan metinler.
- **En çok bağlantı alan sayfalar:** sitenizin bağlantı alan sayfaları. 404 veren sayfalar adresin yanında görünür.
- **Toplu OA:** yazdığınız alan adlarının (ya da projenin ve rakiplerinin) otoritesini yan yana gösterir; en fazla 1.000 alan adı.

Backlink verileri 3 gün güncel sayılır.

## 6. Boşluk Analizi

Üstte projenin rakipleri seçili gelir. Karşılaştırmaya almak ya da çıkarmak için rakibin adına tıklayın; en fazla 5 rakip seçilir. Projede rakip yoksa önce **Alan Adı Genel Bakış**'ın **Rakipler** sekmesinden ekleyin.

- **Eksik kelimeler:** rakiplerin sıralandığı, sizin sıralanmadığınız kelimeler. Daha çok rakibin sıralandığı kelimeler önce gelir; bunlar ilk yazmanız gereken konulardır. Her rakibin sırası ayrı sütundadır. "En az … rakip" ile süzülür.
- **Ortak ve zayıf:** hem sizin hem rakiplerin sıralandığı kelimeler. Bir rakip sizden üstteyse kelime **Zayıf** işaretlenir; bu sayfaları güçlendirmek en hızlı kazançtır. "Yalnızca zayıflar" ile süzülür.
- **Backlink boşluğu:** seçtiğiniz rakiplerin hepsine bağlantı veren ama size vermeyen siteler, OA'larıyla. Bağlantı istemek için iyi adaylardır.
- **Toplu karşılaştırma ve pazar payı:**
  - Yazdığınız alan adlarının (boş bırakırsanız projenin ve rakiplerinin) aylık organik trafiği, kelime sayısı, ücretli trafiği ve OA'sı yan yana; en fazla 100 alan adı.
  - **Pazar payı:** sitelerin trafiği toplanır, her sitenin payı bu toplamdaki yüzdesidir. Pasta grafikte görünür.

Kelimeleri seçip bir listeye ekleyebilir, her tabloyu CSV olarak indirebilirsiniz.

**Maliyet:** Kelime boşlukları her rakip için ayrı bir sorgudur; tahmin hepsinin toplamını gösterir ve bütçe toplama göre denetlenir. Kelime boşlukları 7 gün, backlink boşluğu 3 gün güncel sayılır.

## 7. Sıralama Takibi

Kelimelerinizin Google, Bing ve Yandex'te kaçıncı sırada olduğunu gün gün izler.

**Başlarken:**
1. **Kelime ekle**'ye basın. Kelimeleri her satıra bir tane yazın ya da bir anahtar kelime listesinden alın. İsterseniz etiket verin (ör. "marka", "kış kampanyası").
2. İlk eklemede **ne sıklıkla denetleneceğini** seçin: **Günlük**, **Haftalık** ya da **Elle**. Elle seçerseniz yalnızca siz **Şimdi denetle**'ye bastığınızda ödeme yapılır.
3. Anahtar Kelime Gezgini'nde kelimeleri seçip **Sıralamayı takip et**'e basarak da ekleyebilirsiniz; aylık aramalarıyla birlikte gelirler.

**Takip yerleri:** İlk yer, projenin ülkesinde Google masaüstüdür. **Yerleri yönet**'ten başka yer ekleyebilirsiniz: Bing ya da Yandex, mobil, başka bir ülke ya da bir şehir (ör. `Istanbul,Istanbul,Turkey`). Her kelime her yerde ayrı denetlenir. En fazla 10 yer ve 5.000 kelime takip edilir.

**Maliyet:**
- Bir denetim, bakılan her 10 sonuç için $0.002'dir. Varsayılan ilk 20 sonuçtur.
- "Sonradan yüklenen AI Overview'ları da al" işaretliyse Google'da kelime başına $0.002 eklenir. Sayfada AI Overview yoksa bu ücret iade edilir.
- Üstteki satır bir denetimin ve seçtiğiniz sıklıkla bir ayın tahmini tutarını gösterir; kredi kullanıyorsanız kredi olarak.
- Denetim, kaynak yoksa ya da aylık bütçe aşılacaksa hiç başlamaz.

**Ne görürsünüz:**
- **Görünürlük:** Kelimeleriniz 1. sırada olsaydı alacağınız tıklamalara göre bugünkü tahmini tıklama payınız. Aylık aramaya göre ağırlıklıdır: çok aranan bir kelimedeki yükseliş daha çok etkiler.
- **Ortalama sıra**, ilk 3 ve ilk 10'daki kelime sayısı.
- **AI Overview:** kaç kelimede AI Overview çıktığı ve kaçında sitenizin kaynak gösterildiği.
- **Hareket:** yükselen, düşen, sıralamadan çıkan kelimeler; öne çıkan snippet'i sizde olanlar; birden çok sayfanızın yarıştığı kelimeler.
- **Rakip payları:** aynı kelimelerde sizin ve rakiplerinizin görünürlüğü, bugün ve gün gün. Projede rakip yoksa en görünür beş site gösterilir.
- **Kelimeler tablosu:** sıra ve değişim, en iyi sıra, aylık arama, sıralanan sayfa, sonuç sayfasındaki işaretler (AI Overview, snippet, yarışan sayfalar) ve geçmiş. Arama, etiket, sıra aralığı ve "Yalnızca AI Overview olanlar" ile süzülür. Seçili kelimeleri etiketleyebilir, takipten çıkarabilir, CSV indirebilirsiniz.
- Bir kelimeye tıklayınca sıra geçmişi, her denetim ve son denetimdeki ilk sonuçlar açılır; sizin sonucunuz vurgulanır.

**Aylık aramalar:** Aramasını bilmediğimiz kelimeler görünürlükte 1 arama sayılır. **Aramaları getir** bunları DataForSEO'dan alır (bir istekte 700 kelimeye kadar; 30 gün saklanır).

**Planlı denetimler:**
- **Masaüstünde** program açıkken çalışır. Program kapalıyken kaçan denetim, program açıldıktan kısa süre sonra yapılır.
- **Webde** kimse oturum açık tutmasa da sunucuda çalışır.
- Son denetimler listesi her denetimin planlı mı elle mi yapıldığını, kaç kelimenin denetlendiğini, hataları ve tutarını gösterir.

## 8. Site Denetimi

Sitenizi tarar, SEO Crawler'ın 118 teknik kontrolünü çalıştırır ve sitenin yapay zekâ yanıtlarına ne kadar hazır olduğunu (GEO) ölçer.

**Başlatmak:**
1. **Sıklık** seçin: **Haftalık**, **Aylık** ya da **Elle**. Varsayılan Elle'dir.
2. **En çok taranacak adres** sayısını seçin. Görseller ve yönlendirmeler de adres sayılır.
3. **Denetimi başlat**'a basın. İlerleme sayfada ve kenar çubuğunda görünür; istediğiniz an durdurabilirsiniz.

**Nerede çalışır:**
- **Masaüstünde** tarama bilgisayarınızda yapılır, ücretsizdir; 100.000 adrese kadar.
- **Webde** tarama sunucuda yapılır, 5.000 adrese kadar.
  - Her ay ilk 1.000 adres ücretsizdir. Sonrası kredinizden düşer; sayfada 1.000 adresin kaç kredi tuttuğu yazar.
  - Krediniz biterse tarama durur ve o ana kadar taranan sayfalar raporlanır.
  - Sunucu yalnızca herkese açık sitelere bağlanır. Yerel ağ, `localhost` ya da özel adresler taranmaz; bunlar için masaüstü programını kullanın.

**Tarayıcı kimliği:** Denetim, OctopusOn tarayıcısı olarak (`octopusSEO-Spider`) gelir ve robots.txt'e uyar. Başka sitelere, CSS ve JavaScript dosyalarına gitmez. JavaScript çalıştırmaz.

**Raporda neler var:**
- **Sağlık** (0–100): teknik SEO sağlığı.
- **GEO hazırlığı** (0–100): sitenin AI yanıtlarına hazırlığı. Her ikisinde de bir önceki denetime göre değişim gösterilir.
- **GEO işaretleri:**
  - AI arama botları (OAI-SearchBot, PerplexityBot, Claude-SearchBot …) ve AI asistanları (ChatGPT-User …) robots.txt'te engelli mi?
  - llms.txt var mı?
  - Sayfalar alıntılanabiliyor mu (nosnippet, kısa max-snippet)?
  - Yapılandırılmış veri ne kadar sayfada var? Organization ve WebSite şeması var mı?
  - İçerikli sayfalarda soru biçiminde başlık var mı? Sayfalarda yeterli içerik var mı?

  Her işaretin örnek sayfaları açılabilir. Yanında tüm AI botları robots.txt kararıyla ve bulunan şema türleri listelenir.
- **Sorunlar:**
  - Sorun, uyarı ve fırsat olarak ayrılır; kategoriye göre gruplanır.
  - Bir kontrolün yanında bir önceki denetime göre artış ya da azalış görünür. "Düzeltilenleri de göster" ile artık bulunmayanları da görürsünüz.
  - Bir kontrole tıklayınca ne anlama geldiği, nasıl düzeltileceği ve bulunduğu adresler (100'erli sayfalar) açılır.
- **Eski denetimler:** "Denetim" listesinden açılabilir ya da silinebilir. Silinen denetimin taraması da silinir.

Kontrollerin açıklamaları Türkçe ve İngilizce'dir; Almanca ve İspanyolca arayüzde İngilizce görünür.

## 9. AI Görünürlüğü

Müşterilerinizin yapay zekâ asistanlarına sorabileceği soruları ChatGPT, Claude, Gemini, Perplexity ve Google AI Mode'a sorar. Her yanıtta markanızın anılıp anılmadığını, sitenizin kaynak gösterilip gösterilmediğini ve rakiplere karşı payınızı ölçer.

**Başlamak:**
1. **Soru ekle**'ye basın ve her satıra bir soru yazın (en çok 500 karakter; projede en çok 200 soru). İsterseniz etiket ekleyin.
   - **Öneriler** markanızdan ve Sıralama Takibi'ndeki en çok aranan kelimelerinizden hazırlanır; basınca listeye eklenir. Öneriler ücretsizdir.
   - İlk sorularda ne sıklıkla sorulacağını seçersiniz: **Haftalık**, **Aylık** ya da **Elle** (varsayılan).
2. Sorulacak **asistanları** seçin. Her asistanın yanında nasıl sorulacağı yazar: **kendi anahtarınız**, **DataForSEO**, **kredi** ya da **kullanılamıyor**.
3. **Markalar ve modeller** ile rakip markaları girin: alan adı ve yanıtlarda geçebilecek adları (virgülle; en çok 10 rakip). Boş bırakırsanız projenin rakipleri kullanılır. Aynı pencerede her asistanın modelini değiştirebilirsiniz.
4. **Şimdi sor**'a basın. Her soru seçili her asistana bir kez sorulur; asistanlar aynı anda sorulur. İlerleme sayfada ve kenar çubuğunda görünür.

**Ücret:** Sayfa bir ölçümün ve bir aylık planın tahmini tutarını gösterir (kredide kredi olarak). Yanıtlar her seferinde yeniden sorulur; aylık bütçe aşılmaz.
- Bir asistanın yanıtı alınamazsa hata kaydedilir ve ölçüm sürer.
- Anahtar reddedilir, kredi ya da bütçe biterse yalnızca o asistan durur.

**Raporda neler var:**
- **Anılma oranı:** markanızın geçtiği yanıtların payı.
- **Kaynak gösterilme:** sitenizi kaynak gösteren yanıtların payı.
- **Ses payı:** yanıtlardaki tüm marka anılmaları içinde sizin payınız.
- **Ortalama sıra:** yanıtta adı geçen markalar arasında kaçıncı geçtiğiniz.
- **Ton:** markanızın geçtiği cümlelerin tonu (olumlu, nötr, olumsuz; sözlük tabanlı).
- **Asistanlara göre** anılma oranı ve ölçümden ölçüme değişim grafiği (30, 90 ya da 180 gün).
- **Ses payı ve kaynaklar:** markaların payları, yanıtların en çok kaynak gösterdiği alan adları ve kaynak gösterilen sayfalarınız.
- **Sorular:** her sorunun her asistandaki son yanıtı. Hücreler sitenizin kaynak gösterildiğini, yalnızca anıldığınızı (sıranızla), anılmadığınızı ya da yanıt alınamadığını gösterir. Bir hücreye basınca yanıtın tamamı açılır: markalar renkli işaretlenir, adı geçen markalar ve kaynaklar listelenir, önceki yanıtlar tarihe göre seçilebilir. Tablo CSV olarak indirilebilir.
- **Son ölçümler:** planlı mı elle mi yapıldığı, yanıt ve hata sayısı, tutar.

**LLM Mentions:** DataForSEO'nun yapay zekâ yanıtları veritabanında (Google AI Overview ve ChatGPT) alan adınızın geçtiği yanıtlar. Ücretlidir; **Verileri getir**'e basınca alınır ve 7 gün saklanır.
- **Özet:** anılma sayısı, AI arama hacmi, platformlar ve birlikte kaynak gösterilen alan adları.
- **Sorular:** alan adınızın geçtiği sorular, AI arama hacmine göre (100, 500 ya da 1.000 satır).
- **Rakiplerle:** sizin ve en çok 4 rakibin anılma sayısı ve AI arama hacmi yan yana.

**Planlı ölçümler** Sıralama Takibi'ndeki gibi çalışır: masaüstünde program açıkken, webde sunucuda.

## 10. Search Console

Projenin Search Console verilerini ve isterseniz Google Analytics 4 organik trafiğini okur; bunları Sıralama Takibi ve Site Denetimi ile birleştirir. Google verileri ücretsizdir, kredi kullanmaz.

**Google'a bağlanmak:**
- **Masaüstünde:**
  1. Program yerleşik bir Google istemcisiyle gelmiyorsa Google Cloud Console'da "Masaüstü uygulaması" türünde bir OAuth istemcisi oluşturun. Search Console API ile Google Analytics Data ve Admin API'lerini açın.
  2. İstemci kimliğini ve gizli anahtarını sayfadaki forma girin. Gizli anahtar sistemin anahtar zincirinde saklanır.
  3. **Google'a bağlan**'a basın; tarayıcıda Google izin sayfası açılır. Analytics'i de bağlamak için kutuyu işaretli bırakın.
- **Webde:** **Google'a bağlan**'a basın; Google izin sayfasına gidip geri dönersiniz. Sunucunun Google istemcisi yönetici tarafından kurulmuş olmalıdır.
- Yalnızca okuma izni istenir. **Bağlantıyı kes** Google iznini geri alır.

**Mülkü seçmek:**
1. **Search Console mülkü** listesinden projenin mülkünü seçin; alan adınızı kapsayan mülk "önerilen" diye işaretlidir.
2. İsterseniz **Google Analytics 4 mülkü**nü seçin ve **Kaydet**'e basın.
3. **Dönem** (7, 28, 90 ya da 180 gün) ve **Sıklık** (günlük, haftalık ya da elle; varsayılan haftalık) seçin.
4. **Şimdi güncelle**'ye basın. Seçili dönem (üç gün öncesine kadar, Google'ın kesinleşmiş verisi) ve bir önceki aynı uzunluktaki dönem okunur.

**Raporda neler var:**
- **Tıklama, gösterim, TO ve ortalama sıra:** önceki döneme göre değişimiyle. Analytics bağlıysa organik oturum ve önemli etkinlikler de görünür.
- **Günlük performans:** bu dönemin tıklama ve gösterimleri; kesikli çizgi önceki dönemin aynı günleri.
- **Diğer modüllerle:**
  - takip ettiğiniz kelimelerden kaçının Search Console'da göründüğü;
  - tıklama alan sayfalardan kaçının son site denetiminde hatalı, yönlendirmeli ya da dizine kapalı çıktığı;
  - kaç sorguda birden fazla sayfanızın yarıştığı.
- **Cihaz ve ülke** dağılımı.
- **Sorgular:** tıklamaya göre ilk 500 sorgu.
  - **İlk sıralara yakın:** ortalama sırası 4–20 arasında ve yeterli gösterimi olan sorgular.
  - **Düşük TO:** sırasının genelde aldığı tıklama oranının yarısından azını alan sorgular.
  - Uyarı simgesi sorguda birden fazla sayfanın gösterildiğini belirtir; sorguya basınca sayfaları açılır.
  - Seçtiğiniz sorguları **Sıralamayı takip et** ile Sıralama Takibi'ne ekleyebilirsiniz.
- **Sayfalar:** tıklamaya göre ilk 500 sayfa; Analytics bağlıysa organik oturum, önemli etkinlik ve gelir; denetim yapıldıysa sayfanın son denetimdeki durumu.
- **Aynı sorguda yarışan sayfalar:** birden fazla sayfanızın gösterildiği sorgular.
- Sorgular ve sayfalar CSV olarak indirilebilir.

**Planlı güncellemeler** masaüstünde program açıkken, webde sunucuda çalışır.

## 11. Raporlar

Müşterilerinize ajansınızın markasıyla rapor hazırlar. Raporlar modüllerin mevcut verilerinden hazırlanır; veri kaynağına ücret ödenmez.

**Marka (beyaz etiket):** **Marka** düğmesiyle şirket adınızı, logonuzu (PNG, JPEG, WebP ya da SVG; en çok 300 KB), vurgu renginizi, web sitenizi ve alt bilgi satırını girin. İsterseniz "OctopusOn SEO ile hazırlandı" yazısını gizleyin. Marka tüm projelerin raporlarında görünür.

**Rapor oluşturmak:**
1. **Yeni rapor**'a basın; raporun adını, dilini (Türkçe, English, Deutsch, Español) ve dönemini (7, 28 ya da 90 gün) seçin. Rapor, arayüzün dilinden bağımsız olarak kendi dilinde yazılır.
2. Bölümleri işaretleyin ve oklarla sıralayın:
   - **Özet:** her modülün en önemli iki rakamı.
   - **Sıralamalar:** görünürlük ve değişimi, ortalama sıra, eğilim grafiği, en çok aranan 25 takip edilen kelime.
   - **Rakipler:** rakiplere karşı görünürlük payı.
   - **Search Console:** tıklama, gösterim, TO ve sıra (önceki döneme göre), organik oturum, günlük grafik, en iyi sorgular ve sayfalar.
   - **Site denetimi:** sağlık ve GEO hazırlığı (bir önceki denetime göre) ve öne çıkan sorunlar.
   - **AI görünürlüğü:** anılma ve kaynak gösterilme oranı, ses payı, asistanlara göre anılma.
   - **Backlink'ler:** saklanan backlink ve alan adı özetinden; rapor için veri satın alınmaz.

   Verisi olmayan bölüm "henüz veri yok" der.
3. Sağdaki önizleme PDF'in, bağlantının ve e-postanın göstereceği sayfadır.

**İndirmek:** **PDF** (bilgisayarda Chrome, Chromium ya da Edge gerekir), **Excel** (her tablo ayrı sayfada) ya da **HTML**. Masaüstünde dosyanın nereye kaydedileceğini siz seçersiniz. Webde her ayın ilk PDF'leri ücretsizdir; sonrası krediden düşer.

**Yalnız webde:**
- **Salt okunur bağlantı:** **Bağlantı oluştur** ile müşteriniz raporu oturum açmadan görür; her açılışta güncel verilerle hazırlanır. Kaç kez açıldığı görünür; **Paylaşımı kaldır** bağlantıyı kapatır.
- **E-postayla gönderim:** **Haftalık** ya da **Aylık** seçip en çok 5 alıcı girin. Rapor PDF olarak gider (bağlantı varsa e-postada o da yer alır); yanıtlar hesabınızın e-posta adresine gelir. **Şimdi gönder** hemen gönderir. Son gönderimler kartta listelenir.

## 12. AI asistanları (MCP) ve komut satırı

Claude, ChatGPT, Cursor gibi MCP destekleyen bir AI asistanı projelerinizin verilerini okuyabilir ve sorularınızı bu verilerle yanıtlar. Örneğin "Bu hafta hangi kelimelerde düştük?", "Sitemin en önemli 5 sorunu ne?" ya da "Rakibim hangi konularda benden güçlü?" diye sorabilirsiniz.

**Ücretli veri izninizle alınır.** Asistan önce saklanan verilere bakar. Taze veri gerekiyorsa önce fiyatı (kredi ve dolar) söyler ve onayınızı bekler. Kendi başına harcama yapamaz; aylık bütçeleriniz burada da geçerlidir.

**Masaüstünde:**
1. **Ayarlar → AI asistanları** bölümündeki MCP ayarlarını kopyalayın.
2. Asistanınızın MCP ayarlarına yapıştırın. Örneğin Claude Desktop'ta `claude_desktop_config.json` dosyasına, Claude Code'da gösterilen `claude mcp add …` komutuyla eklenir.
3. Asistan bu programı `mcp` ile penceresiz başlatır; ayrıca bir şey kurmanız gerekmez. Aynı veri klasörünü kullanır ve program açıkken de çalışır.

**Web'de (app.octopuson.com):**
1. **Ayarlar → AI asistanları** bölümünde yeni anahtara bir ad verin (örn. "Claude Desktop") ve **Anahtar oluştur**'a basın.
2. Anahtar yalnızca bir kez gösterilir. Hazır MCP ayarlarını hemen kopyalayın: adres `https://app.octopuson.com/mcp`, başlık `Authorization: Bearer opt_…`.
3. Her asistana ayrı anahtar oluşturun; en fazla 10 anahtarınız olabilir. Kullanmadığınız anahtarı **İptal et** ile kapatın. my.octopuson.com'da programın erişimini kaldırırsanız tüm anahtarlar da çalışmaz.

**Hazır beceriler (skills):** `distribution/geo-seo-platform/skills/` klasöründe asistanlar için beş beceri bulunur:
- `seo-audit`: site denetimi
- `keyword-research`: anahtar kelime araştırması
- `competitor-analysis`: rakip analizi
- `geo-visibility`: AI görünürlüğü
- `seo-coach`: haftalık değerlendirme ve sonraki adımlar

Claude Code'da bu klasörleri `~/.claude/skills/` içine kopyalayın. Claude uygulamalarında bir beceri klasörünü zip yapıp **Ayarlar → Yetenekler** bölümünden yükleyin.

**Komut satırı:** `octopuson-platform` komutu programın tüm işlemlerini penceresiz çalıştırır. Komut, GitHub'daki sürüm sayfasında Windows, macOS ve Linux arşivleri olarak bulunur:
- `octopuson-platform projects`: projeleri listeler.
- `octopuson-platform call rank_report '{"project_id":1}'`: herhangi bir işlemi çalıştırır; JSON alır, JSON döndürür.
- `octopuson-platform commands`: tüm işlemleri listeler.

## 13. Masaüstü ve web eşitlemesi

Masaüstü programında OctopusOn SEO hesabınızı bağladığınızda projeleriniz app.octopuson.com ile kendiliğinden eşitlenir. Bir tarafta yaptığınız değişiklik birkaç saniye içinde, en geç yarım dakikada öbür tarafta görünür.

- **Eşitlenenler:**
  - projeler, rakipler ve marka adları;
  - izlenen yerler ve anahtar kelimeler, AI soruları ve ayarları;
  - anahtar kelime listeleri, rapor markası ve raporlar;
  - biten sıralama kontrolleri, AI yanıtları, site denetimleri ve Search Console verileri;
  - ödenmiş alan adı, backlink, anahtar kelime ve LLM Mentions raporları. Bir tarafta ödediğiniz rapor öbür tarafta yeniden ödenmez.
- **Eşitlenmeyenler:** kendi API anahtarlarınız, ödeme şekli ve bütçe ayarları, harcama tablosu ve Google oturumu. Bunlar her cihazda ayrıdır.
- **Zamanlanmış işler:** eşitleme açıkken günlük/haftalık sıralama, AI görünürlüğü, denetim ve Search Console işleri web sunucusunda çalışır; bilgisayarınız kapalıyken de. Krediyle ya da webde girdiğiniz anahtarlarla ödenir. Elle başlattığınız işler başlattığınız yerde çalışır. Eşitlemeyi kapatırsanız zamanlanmış işler yine masaüstünde çalışır.
- **İlk eşitleme:** hem bilgisayarınızda hem webde proje varsa program sorar:
  - **Birleştir** (önerilen): iki taraftaki projelerin hepsi her yerde olur.
  - **Webdekini bu bilgisayardakiyle değiştir.**
  - **Bu bilgisayardakini webdekiyle değiştir.**
  - **Şimdilik eşitleme.**

  Başka bir hesap bağladığınızda ya da webdeki veriler yedekten döndüğünde de aynı soru gelir.
- **Denetim ayrıntıları:** denetimi yapan bilgisayar tarama dosyasını (en fazla 30 MB) webe gönderir. Her projenin son denetiminin ayrıntıları öbür tarafa da gelir.
- **Ayarlar → Hesapla eşitleme:** son eşitleme zamanı, gönderilmeyi bekleyen değişiklikler, **Şimdi eşitle** ve **Eşitlemeyi kapat**.

## 14. Ayarlar

- **Hesapla eşitleme (masaüstü):** eşitlemenin durumu, **Şimdi eşitle** ve açma/kapama (bkz. 13. bölüm). Web sürümünde bu kart yalnızca nasıl çalıştığını anlatır.
- **Dil:** Türkçe, English, Deutsch veya Español.
- **Vekil sunucu (proxy):** yalnızca masaüstünde; istekleri bir vekil sunucudan geçirmek için.
- **AI asistanları:** MCP ayarları ve web'de erişim anahtarları (bkz. 12. bölüm).
- **Çıkış yap (web):** yalnızca o tarayıcının oturumunu kapatır. Planlanmış işler hesabınızla çalışmaya devam eder. Tamamen durdurmak için my.octopuson.com'da **Jetonlar** sayfasından programın erişimini kaldırın.

## Gizlilik

- **Kendi anahtarınızla:** istekler doğrudan sağlayıcıya gider.
- **Kredili modda:** istekler octopuson üzerinden geçer, ama içerikleri saklanmaz; yalnızca hangi kaynağın ne kadar kullanıldığı kaydedilir.
- **AI asistanları:** asistanın okuduğu veriler, asistanın kendi sağlayıcısına (örn. Anthropic, OpenAI) gider. Bunun için o sağlayıcının gizlilik koşulları geçerlidir. Sunucu erişim anahtarının kendisini değil, yalnızca özetini (hash) saklar.
- **Paylaşılan raporlar:** bağlantıyı bilen herkes raporu görür; bağlantıyı kaldırınca kapanır. Sunucu bağlantının kendisini değil, yalnızca özetini (hash) saklar.
- **Google (Search Console, Analytics):** veriler doğrudan Google'dan okunur, octopuson'dan geçmez. Google'ın verdiği yenileme anahtarı diğer anahtarlarınızla birlikte şifreli saklanır.
- **Masaüstünde:** projeleriniz ve saklanan veriler bilgisayarınızdadır. Eşitleme açıkken aynı veriler hesabınızın web alanında da tutulur; kendi API anahtarlarınız bilgisayarınızdan çıkmaz.
- **Web'de:** veriler hesabınıza özel klasördedir. Anahtarlarınız ve hesap jetonunuz şifrelenerek saklanır.
