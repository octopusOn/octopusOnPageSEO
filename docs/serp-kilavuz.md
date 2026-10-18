# OctopusOn SERP Checker — Kullanım kılavuzu

SERP Checker Google, Bing ve Yandex sonuç sayfalarını okur. Sıralanan sayfaları Spider motoruyla tarar, içeriklerini ve sitelerini puanlar, kelime zorluğunu açıklar.

Masaüstü uygulaması ve `octopuson-serp` komut satırı aracı aynı veri klasörünü kullanır. Bu klasör `OCTOPUSSEO_SERP_DATA_DIR` ortam değişkeniyle ya da işletim sisteminin "octopusSEO SERP Finder" uygulama verisi klasörüyle belirlenir. Birinde yapılan her şey diğerinde görünür.

## 1. Kaynaklar ve anahtarlar

Her arama motoru kaynaklarını sizin belirlediğiniz sırayla sorar (Kaynaklar → Kaynak sırası). Bir kaynak şu durumlarda atlanır ve sıradakine geçilir: kapalıysa, anahtarı yoksa, aylık bütçesi dolduysa, kotası bittiyse ya da ağa ulaşılamıyorsa. Tarayıcıdaki doğrulama sayfası ise hiçbir zaman sessizce atlanmaz; sizi bekler. Bir kayıt her zaman tek bir kaynaktan gelir.

| Kaynak | Motorlar | Anahtar |
|---|---|---|
| Tarayıcı modu (ücretsiz) | Google, Bing, Yandex | yok |
| Yandex Search API | Yandex | `api_key`, `folder_id` |
| DataForSEO | Google, Bing, Yandex | `credentials` (`kullanıcı:şifre`) |
| Serper | Google | `api_key` |
| SerpApi | Google, Bing, Yandex | `api_key` |
| Kendi sağlayıcınız | tanımladığınız gibi | tanımladığınız gibi |

**Anahtarları girmek:** Uygulamada Kaynaklar sayfasındaki **Anahtar gir** düğmesini ya da `octopuson-serp providers set-key <sağlayıcı> <ad>` komutunu kullanın; komut anahtarı standart girdiden okur.

**Anahtarların saklanması:**
- Anahtarlar sistem anahtarlığında saklanır: macOS'ta Anahtar Zinciri, Windows'ta Kimlik Bilgileri Yöneticisi. Linux'ta ya da `OCTOPUSSEO_SECRET_STORE=file` ayarlıyken yalnızca sizin okuyabildiğiniz bir dosyaya yazılır.
- Her anahtar, kaydedildiği andaki sağlayıcı adresine bağlanır. Tanım başka bir adrese istek gönderecek şekilde değiştirilirse anahtar yeniden istenir.
- Ayar dosyalarına, dışa aktarımlara ve loglara asla yazılmaz.

**Derinlik ve istek sayısı:** DataForSEO ve Yandex Search API 100 sonucu tek istekte döndürür. Google ise Eylül 2025'ten beri `num=100`'ü yok sayıyor ve her 10 sonucu ayrı sayfa olarak veriyor. Bu yüzden tarayıcı modu, Serper ya da SerpApi ile Google'da derinlik 100, 10 istek demektir. Tarayıcı modunda Bing ve Yandex de sayfa başına 10 sonuç veriyor: 18.10.2026'daki canlı denemede Bing `count=50`'yi, Yandex `numdoc=50`'yi yok saydı.

**Maliyet:** Her çalıştırmadan önce ilk kullanılabilir kaynağın istek sayısı ve tahmini maliyeti gösterilir. Tutar onay sınırını (varsayılan 1 USD) aşarsa onay istenir. Komut satırında `octopuson-serp estimate` tahmini, `octopuson-serp usage` ise sağlayıcı ve ay bazında istekleri ve harcamayı gösterir.

### OctopusOn SEO kredisiyle

Yerleşik ücretli kaynaklar (Serper, SerpApi, DataForSEO, Yandex Search API) kendi anahtarınız olmadan da çalışır. OctopusOn SEO hesabınızı bağlayın: Kaynaklar → OctopusOn SEO hesabı → Hesaba bağlan (ya da `octopuson-serp account login`). Tarayıcıda açılan sayfada kodu onaylayın. Krediyi my.octopuson.com'dan yüklersiniz.

- Her yerleşik kaynağın bir ödeme modu vardır:
  - **Otomatik:** Kendi anahtarınız varsa onu, yoksa krediyi kullanır.
  - **Kendi anahtarım**
  - **Kredi:** Komut satırında `octopuson-serp providers mode serper credit`.
  - Kendi tanımladığınız sağlayıcılar krediyle çalışmaz.
- Kredili istekler OctopusOn SEO ağ geçidinden geçer. Kendi anahtarlarınız oraya gönderilmez ve istek içerikleri saklanmaz. DataForSEO ve Wordstat üzerinden alınan hacim ve backlink özetleri de aynı moda uyar.
- Tahminler kredi fiyatını gösterir. Kaydedilen maliyet, ağ geçidinin aldığı tutardır. Kredili kısmı `octopuson-serp usage` gösterir.
- Kredi bitince (402) zincir, reddedilen her anahtarda olduğu gibi sıradaki kaynağa geçer.
- Hesap jetonu, tüm OctopusOn SEO programlarının ortak kullandığı ve yalnızca sizin okuyabildiğiniz tek bir dosyada (`account.json`) durur. Bir programda bağlanınca diğerleri de birkaç saniye içinde bağlanmış olur. `octopuson-serp account logout` bağlantıyı hepsinde keser.

### Tarayıcı modu

Kendi profiliyle (`<veri>/browser-profile`) görünür bir Chrome ya da Edge penceresi açar ve her seferinde tek sorgu arar.

- **Hız:** Aramalar arasında sabit bir bekleme vardır (varsayılan 30 sn, en az 10 sn). Motor başına günlük bir sınır uygulanır (varsayılan 100). Her doğrulama sayfasından sonra bekleme iki katına çıkar.
- **Doğrulama sayfaları:** Motor bir çerez onayı ya da doğrulama sayfası gösterirse pencere öne gelir ve arama, sayfayı sizin geçmeniz için en fazla 10 dakika bekler. Uygulama CAPTCHA çözmez, tarayıcının parmak izini ya da user-agent'ını değiştirmez, proxy döndürmez. Kendi tek proxy'nizi kullanabilirsiniz.
- **Kapsam:** Yalnızca masaüstü sonuçları getirir. Mobil sonuçlar için API kullanın.
- **Google adresleri gizliyor:** 2026'dan beri Google sonuç bağlantıları şifreli (`/goto?url=…`). Tarayıcı modu her Google sonucunun sitesini görünen adresinden okur ve onu *yalnızca site* olarak işaretler.
  - Domain ya da host bazında sıralar kesindir.
  - URL öneki eşleştirme ve kelime analizinde sayfa taraması için bir Google API kaynağı (Serper, DataForSEO, SerpApi) gerekir.
  - Bing ve Yandex sonuçları tam adresleriyle gelir.
- **Derinlik:** Google ilk sayfada 6–9 organik sonuç gösteriyor; ilk on için derinliği 20 seçin.

Otomatik arama, arama motorlarının kullanım koşullarıyla çelişebilir. Tarayıcı modunu bilgisayar başındayken düşük hacimde kullanın; düzenli takip için API tercih edin.

### Kendi sağlayıcınız

Herhangi bir SERP API'sini bir JSON tanımıyla ekleyebilirsiniz.

- **İstek:** Şablon olarak yazılır. Yer tutucular şunlardır: `{keyword}`, `{gl}`, `{hl}`, `{device}`, `{page}`, `{num:int}`, `{secret:token}`. Her yer tutucu, bulunduğu yere göre kodlanır.
- **Yanıt:** Sonuçlar ve SERP özellikleri JSONPath (`$.organic[*]`), CSS seçicileri (`a @href`, `h3 ::text`) ya da adlandırılmış gruplu bir düzenli ifadeyle okunur.
- **Uygulamada:** Kaynaklar → Kendi sağlayıcılarınız bölümünde tanımı ekleyin. Yapıştırdığınız bir yanıtla istek göndermeden test edebilirsiniz.
- **Komut satırında:** `octopuson-serp providers add tanim.json` tanımı ekler. `octopuson-serp providers test tanim.json --response yanit.json` kaydedilmiş bir yanıtı istek göndermeden ayrıştırır.

### Arama motoru HTML'ini değiştirdiğinde

Tarayıcı modu sayfaları sürümlü yerleşik seçicilerle okur (`google-html@1` gibi). Her yanıt `<veri>/raw/` altında sıkıştırılmış olarak saklanır.

Sonuçlar okunamamaya başlarsa uygulama "düşük kapsam" uyarısı verir ve sayfayı `<veri>/diagnostics/` altına kaydeder. Bu durumda:

1. Kaynaklar → Sonuç sayfası seçicileri bölümünde seçicileri bir JSON merge patch ile yamayın.
2. Yamayı saklanan bir sayfada test edin ve kaydedin.
3. Eski kayıtları yeniden okuyun: SERP kayıtları → **Yeniden oku**, ya da `octopuson-serp reparse <kayıt>`.

Yeniden okunan bir kaydı kullanan sıra kontrollerinin sıraları da düzeltilir.

## 2. Kelime analizi

Uygulamada **Kelime analizi** sayfasını ya da şu komutu kullanın:

```sh
octopuson-serp analyze "türk kahvesi nasıl yapılır" --engine google --country tr --site ornek.com.tr
```

Analiz şu adımlarla ilerler:

1. **Arama:** Kelime aranır.
2. **Tarama:** İlk sonuçlar (varsayılan 10, en fazla 20) ve verdiyseniz sizin sayfanız Spider motoruyla taranır. Tarama robots.txt'ye uyar, OctopusOn SEO ya da Chrome user-agent'ını kullanır, isterseniz JavaScript çalıştırır.
3. **Domain bilgileri:** Bulunan bilgiler önbelleğe alınır.
   - **Yaş:** RDAP'tan alınır; `.tr` gibi uzantılarda WHOIS, gerekirse ilk Wayback kaydı kullanılır. 180 gün saklanır.
   - **Bağlantı otoritesi:** Ücretsiz Open PageRank anahtarıyla puan, sıra ve yönlendiren domain sayısı alınır. 30 gün saklanır.
   - **İsteğe bağlı:** DataForSEO backlink özetleri (ücretli) ve indirilmişse Tranco sırası.
4. **Puanlama:** Sayfa, site, zorluk, niyet ve fırsat puanları hesaplanıp saklanır. Daha sonra `octopuson-serp rescore <id>` ile saklanan bir analiz, yeni istek göndermeden güncel modellerle yeniden puanlanır.

Her puan bileşenlerini ölçülen değer, ağırlık, katkı ve gerekçeyle birlikte listeler. Eksik bir sinyal hesaptan çıkarılır, kalan ağırlıklar yeniden dağıtılır ve **güven** değeri düşer.

**İçerik (`content-1`):**

| Bileşen | Ağırlık | Ne ölçülür |
|---|---|---|
| Kelime yerleşimi | 20 | Kökleriyle başlık, H1, ilk 100 kelime, URL ve açıklamada |
| Konu kapsamı | 25 | Diğer ilk sayfaların en az %40'ının kullandığı terimler |
| Derinlik | 15 | Kelime sayısının medyana oranı |
| Yapı | 10 | |
| Yanıtlanan sorular | 5 | |
| Medya | 5 | |
| Schema | 10 | |
| Snippet uygunluğu | 5 | |
| Tazelik | 5 | |

**Teknik (`tech-1`):**
- Spider denetim bulguları puan düşürür: yüksek öncelik 12, orta 6, düşük 2 puan.
- HTTPS/HSTS ve sunucu yanıt süresi de hesaba katılır.
- 200 dışı ya da noindex sayfalar 0 alır.

**Site (`site-1`):**

| Sinyal | Ağırlık |
|---|---|
| Open PageRank | 30 |
| Yönlendiren domainler | 25 |
| Tranco (isteğe bağlı) | 20 |
| Domain yaşı | 15 |
| HTTPS/HSTS | 10 |

**Kelime zorluğu (`kd-1`):**
- Her sonucun gücü = 0,55 × site + 0,30 × içerik + 0,15 × hedefleme.
- Taban değer, ilk on sonucun tıklama payıyla ağırlıklı ortalamasıdır.
- Düzeltmeler: marka baskınlığı en fazla +10, forum ve kullanıcı içeriği en fazla −10, üç ve daha fazla reklam en fazla +5.
- Bantlar:

| Puan | Bant |
|---|---|
| 0–14 | Çok kolay |
| 15–29 | Kolay |
| 30–49 | Orta |
| 50–69 | Zor |
| 70–84 | Çok zor |
| 85+ | Aşırı |

**Fırsat (`opp-1`, siteniz verilince):**

| Bileşen | Ağırlık |
|---|---|
| Zorluğa karşı kazanma şansı | 40 |
| Talep (hacim) | 25 |
| Kalan tıklamalar | 20 |
| Sıra | 15 |

Kalan tıklamalar; yapay zekâ yanıtları, yerel paketler, başkasının öne çıkan snippet'i ve reklamlar çıkarıldıktan sonra geriye kalan payı ölçer.

**Niyet (`intent-1`):** Şu sinyallerden çıkarılır: Türkçe ve İngilizce kalıplar ("nasıl", "fiyat", "en iyi" gibi), il ve ilçe adları, markalar, SERP özellikleri ve ilk sayfaların schema tipleri.

**Öneriler:**
- İlk sayfaların kelime sayısı ve H2 aralıkları
- Sık kullanılan alt başlık konuları
- Schema tipleri
- Başlık uzunluğu ve kalıbı
- Yanıtlanacak sorular
- Sonuç sayfasında yapay zekâ yanıtı ya da snippet varsa başa kısa bir yanıt
- İçerik boşlukları
- İlgili kelime fikirleri

**Aylık arama hacmi:** Kuruluysa Wordstat'tan (Yandex Search API anahtarıyla) ya da DataForSEO'dan (Google Ads verisi) gelir.

## 3. Sıra takibi

```sh
octopuson-serp project create "Örnek" --site ornek.com.tr --engine google --country tr --language tr --depth 20
octopuson-serp project add-keywords 1 "türk kahvesi" "filtre kahve" --tag kahve
octopuson-serp track 1            # şimdi kontrol et (maliyet sınırı aşılırsa sorar; --yes sormaz)
octopuson-serp track --due        # zamanı gelen tüm projeler, cron için
octopuson-serp export rank 1 siralar.xlsx
```

**Proje:**
- Bir siteyi takip eder. Eşleştirme kayıtlı domain, tam host ya da URL önekiyle yapılır.
- Bir ya da birden fazla motor ve yerel ayar üzerinde çalışır.
- Kontroller günlük, haftalık ya da yalnızca istenince yapılır.

**Zamanlama:**
- Masaüstü uygulaması açıldıktan 20 saniye sonra, sonra da açık kaldığı sürece her 10 dakikada bir, zamanı gelen projeleri tek tek kontrol eder.
- Uygulama kapalıyken kaçırılan kontroller açılışta yapılır.
- Geçilmeyen bir doğrulama sayfası çalıştırmayı durdurur. Böylece engelleyen bir motor tekrar tekrar sorulmaz.

**Sıra raporu şunları gösterir:**
- Kelime başına son sıra, değişim, en iyi sıra, geçmiş, sıralanan URL ve aylık hacim
- Kanibalizasyon: aynı kelimede sitenizin birden çok URL'sinin sıralanması
- Günlük görünürlük: hacimle ağırlıklı tıklama payının, her yerde ilk sırada olsaydınız alacağınız paya oranı
- Sitenizin ve rakiplerinin görünürlük payı

## 4. Veri klasörü

```
settings.json            varsayılanlar, kaynak sırası, bütçeler, tarayıcı modu, proxy
providers.json           kendi sağlayıcı tanımlarınız
parser-overrides.json    seçici yamaları
secret-bindings.json     hangi anahtarın hangi adrese ait olduğu (anahtarların kendisi değil)
serp.duckdb              çalıştırmalar, kayıtlar, sonuçlar, analizler, projeler, sıralar
raw/YYYY-MM/             sıkıştırılmış ham yanıtlar
crawls/                  analiz edilen sayfaların Spider taramaları
datasets/                RDAP listesi, Tranco
browser-profile/         tarayıcı modunun profili (Ayarlar → Tarayıcı profilini sıfırla)
diagnostics/             sonuç okunamayan sayfalar
```

Komut satırı ve uygulama sırayla kullanılabilir. Biri veritabanına yazarken veritabanı kilitlenir.
