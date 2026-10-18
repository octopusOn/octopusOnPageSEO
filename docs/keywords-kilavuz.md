# OctopusOn SEO Keyword Research — Kullanım kılavuzu

SEO Keyword Research (SEO Keyword Research) kelimeleri keşfeder, tek bir sayfanın hedefleyebileceği kümelere ayırır ve kümeleri sitenizin sayfalarıyla eşler.

Masaüstü uygulaması ve `octopuson-keywords` komut satırı aracı aynı veri klasörünü kullanır:
- Bu klasör `OCTOPUSSEO_KW_DATA_DIR` ortam değişkeniyle ya da işletim sisteminin "octopusSEO Keyword Miner" uygulama verisi klasörüyle belirlenir.
- SERP Checker'ın klasöründen tamamen ayrıdır: anahtarlar, bütçeler ve sonuç sayfaları paylaşılmaz.

Teknik ayrıntılar için [keywords.md](keywords.md), tasarım kararları için [ADR 0069](adr/0069-keyword-miner.md).

## 1. Proje

Her proje bir pazardır: ülke, dil ve arama motorları (ilki birincil motordur). İsterseniz sitenizi ve rakip alan adlarını da ekleyin.

- **Uygulamada:** Projeler → **Yeni proje**.
- **Komut satırında:** `octopuson-keywords project create "Örnek Kahve" --site ornek.com.tr --engine google --engine yandex --competitor rakip.com`

## 2. Kelime toplamak

### Elle ve dosyadan

- **Elle:** Keşif → **Kelime ekle** alanına her satıra bir kelime yazın.
- **Dosyadan:** Keşif → **Dosya seç** ile içe aktarın.

İçe aktarma:
- Desteklenen dosyalar:
  - Ahrefs, Semrush, Keyword Planner, Search Console ve Wordstat dışa aktarımları.
  - Diğer CSV, TSV, TXT ve Excel tabloları.
  - Search Console'un ZIP dosyası.
- Önizleme her sütunun nasıl okunacağını gösterir. Hacim (`1K – 10K` gibi aralıklar dahil), TBM, rekabet, zorluk ve aylık aramalar kelimeyle birlikte gelir.
- Büyük/küçük harf, Türkçe karakter ya da boşluk farkı olan yazımlar tek kelimedir: `Türk Kahvesi` ile `turk kahvesi` aynıdır.

### Ücretli kaynaklar (Kaynaklar sayfası)

| Sağlayıcı | Anahtar | Ne verir |
|---|---|---|
| Yandex Wordstat | `api_key`, `folder_id` | Tohum başına 2.000'e kadar ifade ve çağrışım (Yandex hacmi) |
| DataForSEO | `kullanıcı:şifre` | Google Ads hacim ve fikirleri; Labs fikirleri, ilgili aramalar, öneriler, rakip kelimeleri, zorluk ve niyet; otomatik tamamlama |
| SerpApi | `api_key` | Google otomatik tamamlama |

Anahtarlar ve bütçeler:
- Anahtarları **siz girersiniz**. Sistem anahtarlığında saklanır ve sağlayıcının adresine bağlanır. Ayar dosyalarına, dışa aktarımlara ve loglara asla yazılmaz.
- Her sağlayıcıya aylık bütçe koyabilir ya da onu kapatabilirsiniz.

**Keşif:**
1. Keşif → tohum kelimeleri yazın.
2. Kaynakları ve gerekiyorsa genişletmeleri (sorular, satın alma kelimeleri, a–z, ç ğ ı ö ş ü) seçin. İsterseniz filtre de ekleyin (içermesi gereken ya da istenmeyen kelimeler, en az arama, en çok kelime).
3. **Tahmin et** kaynak başına istek sayısını ve maliyeti gösterir. **Keşfet** işi başlatır. Onay sınırını (varsayılan 1 USD) aşan işlerde onay istenir.

Kaynaklar nasıl yönetilir:
- Anahtarı reddedilen ya da bakiyesi biten sağlayıcı o iş boyunca atlanır.
- Wordstat saatte 100 istek sınırını aşmamak için bekler. İlerleme kartı bunu gösterir.

**Sayılar:** "Bilinen kelimeler için sayılar" bölümü projedeki kelimeler için hacim, zorluk ve niyeti biner biner sorar.

### OctopusOn SEO kredisiyle

Ücretli kaynaklar kendi anahtarınız olmadan da çalışır. OctopusOn SEO hesabınızı bağlayın: Kaynaklar → OctopusOn SEO hesabı → Hesaba bağlan (ya da `octopuson-keywords account login`). Tarayıcıda açılan sayfada programdaki kodu onaylayın. Krediyi my.octopuson.com'dan yüklersiniz.

- Her ücretli kaynağın bir ödeme modu vardır:
  - **Otomatik:** Kendi anahtarınız varsa onu, yoksa krediyi kullanır.
  - **Kendi anahtarım**
  - **Kredi:** Komut satırında `octopuson-keywords providers mode serper credit`.
- Kredili istekler OctopusOn SEO ağ geçidine (`api.octopuson.com`) gider. Ağ geçidi isteği OctopusOn SEO'nun sağlayıcı hesabıyla iletir ve krediden düşer. Kendi anahtarlarınız oraya hiç gönderilmez. İstek içerikleri saklanmaz; yalnızca kaynak, uç nokta, durum ve tutar tutulur.
- Tahminler kredi fiyatını gösterir: sağlayıcı fiyatı × kaynak kartındaki çarpan. Kaydedilen harcama, ağ geçidinin aldığı tutardır. Kredili kısmı `octopuson-keywords usage` ve Kaynaklar sayfası gösterir.
- Kredi bitince (402) kredili kaynak durur. Otomatik moddaki kaynak, varsa kendi anahtarınızla devam eder.
- Hesap jetonu, bilgisayardaki tüm OctopusOn SEO programlarının ortak kullandığı tek bir dosyada durur. Bu dosya ortak octopusSEO veri klasöründeki `account.json`'dır ve yalnızca sizin hesabınız okuyabilir; `OCTOPUSSEO_ACCOUNT_DIR` ile yeri değiştirilebilir.
  - Bir programda bağlanınca diğerleri de bağlanmış olur; açık olan programlar bunu birkaç saniye içinde fark eder.
  - Bağlantıyı `octopuson-keywords account logout` tüm programlarda keser. Cihazları my.octopuson.com'dan da kapatabilirsiniz.

### Kendi Google veriniz

Kendi OAuth istemcinizi kullanırsınız: Google Cloud → Kimlik bilgileri → OAuth istemci kimliği → *Masaüstü uygulaması*. Search Console API'yi, Ads için Google Ads API'yi etkinleştirin.

1. Kaynaklar → Google bölümüne istemci kimliğini ve gizli anahtarı girin.
2. **Google ile giriş yap**'a basın. Giriş kendi tarayıcınızda yapılır; uygulama yalnızca `127.0.0.1` üzerinden yanıtı alır. Yenileme jetonu anahtarlıkta saklanır.

Bağlandıktan sonra:
- **Site eşleme → Search Console:** Bir mülk seçip sorguları içe aktarın. Sorgular kelime olur, sayfaları eşleme için saklanır. Son üç gün hariç, istek başına 25.000, en çok 50.000 satır okunur.
- **Google Ads (isteğe bağlı):** Ads müşteri kimliğinizi girin. Keşifte "Google Ads Keyword Planner" kaynağını, sayılarda "Hacimler kendi Google Ads hesabınızdan" seçeneğini kullanabilirsiniz.
  - Keyword Planner için Google Cloud projenizin en az **Basic** erişimi olmalı. Az harcayan hesaplar yalnızca aralık görebilir.

### Ücretsiz öneri uçları

Google ve Yandex öneri uçları resmî API değildir; Yandex'in koşulları izinsiz otomatik sorguyu yasaklar.

Bu uçlar **varsayılan olarak kapalıdır**. Açmak için Kaynaklar → Ücretsiz öneriler bölümünde uyarıyı onaylamanız gerekir.

Açıldıklarında şöyle çalışırlar:
- İstekler uygulamanın kendi kullanıcı aracısıyla gider: en az 2 saniyede bir, motor başına günde en çok 2.000.
- İlk ret (429, 403, doğrulama sayfası) o motoru ertesi güne kadar durdurur.

## 3. Kümeleme

Kümeler sayfasında üç yöntem vardır:
- **İfade:** Ücretsiz ve anında. Aynı kökleri paylaşan kelimeler birlikte gruplanır. Türkçe ekler atılır, ama `nasıl` ve `fiyat` gibi niyet kelimeleri korunur; bu yüzden "nasıl yapılır" ile "fiyatları" ayrı kalır.
- **Sonuç sayfası:** Kesin. İlk ondaki ortak sonuç sayısı eşiği (varsayılan 3) geçen kelimeler birlikte gruplanır. Soft mod sütun kelimeyle, hard mod tüm üyelerle karşılaştırır.
- **Hibrit:** Sonuç sayfası olan kelimeler sonuçlara, diğerleri ifadeye göre gruplanır. Doğrulanmamış üyeler işaretlenir.

Önerilen akış:
1. Önce **İfade** ile kümeleyin.
2. **Sonuç sayfaları** bölümünden her kümenin en çok aranan 3 kelimesi için sayfaları alın (maliyet önce gösterilir; taze sayfalar yeniden kullanılır).
3. **Hibrit** ile yeniden kümeleyin.

### Tarayıcı modu (gözetimli)

Küçük listelerde sonuç sayfaları bir API yerine görünür bir Chrome ya da Edge penceresinden okunabilir. Bu mod hiçbir varsayılan zincirde yoktur; Kaynaklar → Tarayıcı modu bölümünden motor seçerek kendiniz eklersiniz (API'lerden sonra, en sona eklenir).

Nasıl çalışır:
- Pencerenin kendi profili vardır (`<veri klasörü>/browser-profile`); kendi tarayıcınızın profiline dokunulmaz. **Tarayıcı profilini sıfırla** çerezleri ve onay seçimlerini siler.
- Sorgular tek tek, sabit bir arayla gider (varsayılan 30 sn, en az 10). Motor başına günlük sınır vardır (varsayılan 100 sayfa). İkisi de Ayarlar → Tarayıcı modu bölümünden değiştirilebilir.
- Çerez onayı ya da CAPTCHA çıkarsa pencere öne gelir ve sizi 10 dakikaya kadar bekler; iş kartında da gösterilir. Siz geçince iş devam eder; geçilmezse iş durur.
- Uygulama CAPTCHA çözmez, otomasyonu gizlemez, kullanıcı aracısını ya da parmak izini değiştirmez, proxy döndürmez. Pencere için tek bir proxy girebilirsiniz.

Google tarayıcıda sonuç adreslerini gizler; bu yüzden Google sayfaları yalnızca siteleri verir. Kümeleme o zaman siteleri karşılaştırır, site eşleme bu sayfaları kullanmaz. Otomatik arama motorların koşullarıyla çelişebilir; büyük listeler için bir API kaynağı kullanın.

Her küme şunları gösterir:
- toplam arama, kelime sayısı, niyet (karışıksa uyarılır), zorluk ve TBM;
- doğrulanıp doğrulanmadığı;
- içerik özeti: insanların sorduğu sorular, ilgili aramalar ve üst sıradaki sayfalar.

Kelime taşıma:
- Üyeleri seçip **Şuraya taşı**, **Ayrı küme** ya da **Taşımayı geri al** diyebilirsiniz.
- Taşımalar sonraki kümelemelerde de korunur.

## 4. Site eşleme

Kümeleri sitenizin sayfalarıyla karşılaştırır:
- **Eşlenenler:** Her kümeye hizmet eden sayfanız.
- **Kanibalizasyon:** Aynı kümede birden çok sayfanızın gösterim aldığı durumlar.
- **Boşluklar:** Rakiplerin sıralandığı ama sizin sıralanmadığınız kümeler.

Eşleme Search Console verisini ve sonuç sayfalarını kullanır. **Yeniden eşle** yeni veriden sonra kümeleri değiştirmeden eşlemeyi tazeler.

## 5. Dışa aktarma

- **Kelimeler:** Kelimeler sayfasında filtrelere uyan tüm kelimeler dışa aktarılır.
- **Kümeler:** Kümeler sayfasında her kelime kümesinin toplamları ve eşlemesiyle aynı satırda yer alır.
- Excel, CSV ve JSON desteklenir.

## Komut satırı özeti

```sh
octopuson-keywords import 1 ahrefs.csv --preview
octopuson-keywords import 1 ahrefs.csv
octopuson-keywords discover 1 --seed "türk kahvesi" --source related,suggestions --estimate
octopuson-keywords enrich 1 --ask volume,difficulty
octopuson-keywords cluster 1 --method lexical
octopuson-keywords serp 1 --per-cluster 3
octopuson-keywords cluster 1 --method hybrid
octopuson-keywords map 1
octopuson-keywords export clusters 1 kumeler.xlsx
octopuson-keywords usage
```
