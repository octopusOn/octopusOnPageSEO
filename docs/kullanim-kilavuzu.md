# OctopusOn Log Analyser — Kullanım kılavuzu

OctopusOn Log Analyser, sunucu erişim loglarını SEO açısından analiz eder: arama motoru ve yapay
zekâ botlarının sitenizi gerçekte nasıl taradığını, hangi URL'lerde hata aldığını, tarama
bütçesinin nereye harcandığını, sahte botları, robots.txt kurallarına uyulup uyulmadığını ve
YZ asistanlarından gelen ziyaretleri gösterir. Masaüstü uygulaması ve `octopuson-log` komut satırı
aracı aynı veri klasörünü kullanır; birinde yapılan her şey diğerinde görünür.

## 1. Müşteri ve site

Her müşterinin bir veya birden fazla sitesi olur. Site oluştururken log formatı seçilir:

- **Otomatik algıla** (önerilen): ilk dosyanın ilk satırlarından format tahmin edilir. Başlık
  satırı olan formatlar (IIS/CloudFront `#Fields:`, CSV başlığı) da böyle algılanır.
- **Yerleşik formatlar**: Nginx/Apache combined ve common, Traefik CLF ve JSON, Microsoft IIS
  (W3C), Amazon CloudFront, AWS Elastic Load Balancing (ALB / Classic), HAProxy (HTTP log),
  genel JSON (Caddy, Cloudflare Logpush, Google Cloud yük dengeleyici, Nginx JSON, Akamai…),
  başlık satırlı CSV/TSV.
- **Özel Nginx `log_format` / Apache `LogFormat`**: sunucu yapılandırmasındaki format metni
  aynen yapıştırılır.

Format daha sonra site ayarlarından değiştirilebilir. Bir dosyanın doğru çözümlenip
çözümlenmediğini içe aktarmadan önce **Formatı test et** (CLI: `octopuson-log inspect`) ile görün.

```sh
octopuson-log client add "Acme"
octopuson-log site add acme.com --client "Acme"            # otomatik algılama
octopuson-log site add shop.acme.com --client "Acme" --format combined
```

### Site ayarları (neyin saklanacağı)

**İçe aktar** sekmesindeki **Site ayarları** kartı (CLI: `octopuson-log site settings`) sonraki içe
aktarmaları süzer; büyük sitelerde disk ve zaman kazandırır:

- **Site alan adı**: bu alan adından (ve alt alan adlarından) gelen referer'lar *site içi*
  sayılır. Boşsa site adı kullanılır.
- **Saklanan trafik**: tüm istekler ya da **yalnızca botlar** (isteğe bağlı kategori seçimiyle,
  ör. yalnızca arama motorları ve YZ botları).
- **Yalnızca şu URL'ler / şu URL'ler asla**: düzenli ifadeler (`^/blog/`, `\.(css|js)$`).
- **Kaldırılacak parametreler** (`sid`, `jsessionid`) ve isteğe bağlı olarak tüm izleme
  parametreleri (`utm_*`, `gclid`, `fbclid`…).

Ayarlar yalnızca sonraki içe aktarmalara uygulanır; filtrelenen satır sayısı içe aktarma
sonucunda gösterilir.

```sh
octopuson-log site settings acme.com --traffic bots --exclude '^/wp-admin/' --strip-param sid --strip-tracking true
octopuson-log site settings acme.com            # mevcut ayarları göster
```

### Site arşivi (paylaşma ve taşıma)

Bir siteyi tüm verisi, formatı ve ayarlarıyla tek bir `.snxsite` dosyasına aktarıp bir
iş arkadaşınıza verebilir ya da başka bir bilgisayara taşıyabilirsiniz. Masaüstünde site
ayarları penceresinde **Arşive aktar…**, kenar çubuğunda **Site arşivi aç**.

```sh
octopuson-log site export acme.com acme.snxsite
octopuson-log site import acme.snxsite --name acme-kopya.com --client "Acme"
```

## 2. İçe aktarma

Dosyalar veya klasörler sürükleyip bırakılabilir. Desteklenenler: düz metin, `.gz`, `.zst`,
`.bz2`, `.zip` (sıkıştırma dosya adından değil içeriğinden algılanır; `access.log.1` gibi
döndürülmüş dosyalar da çalışır).

- Daha önce aktarılmış dosyalar okunmadan atlanır.
- Çakışan dosyalardaki (ör. `access.log` ve `access.log.1`) aynı satırlar bir kez saklanır.
- Hiçbir satırı site formatına uymayan dosyalar kaydedilmez; formatı düzeltip tekrar
  aktarabilirsiniz.
- İçe aktarma durdurulabilir; o anki dosya geri alınır, tamamlanan dosyalar kalır.
- Yanlış bir dosya aktarıldıysa **içe aktarma geçmişinden** (çöp kutusu simgesi) o içe aktarma
  verisiyle birlikte silinir (CLI: `octopuson-log site remove-import acme.com 3`).

```sh
octopuson-log import acme.com /var/log/nginx/
```

## 3. Botlar ve doğrulama

Her istek user-agent'ına göre ~110 bilinen bottan birine atanır (Googlebot mobil/masaüstü,
Bingbot, GPTBot, ClaudeBot, PerplexityBot, AhrefsBot…). User-agent kolayca taklit edildiği için
her (IP, bot) çifti doğrulanır:

- **verified (doğrulandı)**: IP, operatörün resmi IP listesinde veya ters + ileri DNS doğrulaması
  başarılı.
- **spoofed (sahte)**: operatör doğrulama verisi yayınlıyor ve IP bu doğrulamadan geçemedi.
- **unverified (doğrulanmadı)**: operatör doğrulama verisi yayınlamıyor (ör. Bytespider), DNS
  kapalıydı veya yanıt vermedi.

Google'ın listesi yalnızca şu an kullanılan aralıkları içerir; eski loglardaki gerçek Googlebot
IP'leri bu yüzden DNS ile doğrulanır. İçe aktarma DNS'i varsayılan olarak kullanır (çevrimdışı
çalışmak için `--no-dns`). Resmi IP listelerini güncellemek ve kayıtlı kararları yeniden
kontrol etmek için:

```sh
octopuson-log bots update
octopuson-log bots verify acme.com --all
octopuson-log bots check-ip 66.249.66.1 --bot googlebot-smartphone
```

Masaüstünde: **Botlar** sekmesi → *Güncel listeleri indir* ve *Yeniden doğrula*.

### Özel botlar

Kendi tarayıcınızı, izleme servislerini ya da katalogda henüz olmayan bir botu tanımlayın:
kimlik (`bot:acme-monitor` olarak sorgulanır), kategori, user-agent desenleri ve doğrulama
yöntemi — sabit IP/CIDR aralıkları, yayımlanmış bir IP listesi URL'si (`bots update` ile
indirilir) ve/veya ters DNS alan adı sonekleri. Özel botlar yerleşik botlardan önce eşleştirilir;
kaydettiğinizde saklanmış istekler bir sonraki kullanımda otomatik olarak yeniden sınıflandırılır.

```sh
octopuson-log bots custom add acme-monitor --name "Acme Monitor" --category monitoring \
  --pattern AcmeMonitor --ip-range 203.0.113.0/24
octopuson-log bots custom export ekip-botlari.json      # paylaşmak için; içe aktarma: import
```

Masaüstünde: **Botlar** → **Özel botlar** kartı.

## 4. Genel bakış

Seçilen dönem için toplam istek, doğrulanmış arama motoru istekleri, taranan URL sayısı, günlük
tarama isteği ve günlük taranan URL, tarama hataları (4xx/5xx), ortalama yanıt boyutu ve süresi,
2xx dışı tarama oranı, parametreli URL ve statik dosya payı (tarama israfı) ile sahte bot
istekleri; kategoriye göre günlük trafik, mobil/masaüstü Googlebot ve durum sınıfı grafikleri.

**Uyarılar** bölümü olağandışı günleri gösterir: doğrulanmış Googlebot'un, arama motorlarının
veya YZ botlarının taramasının birden düşmesi ya da artması ve sunucu hatası (5xx) patlamaları.
Her gün önceki 14 günün medyanıyla karşılaştırılır; küçük dalgalanmalar ve hafta sonu
düşüşleri uyarı üretmez.

```sh
octopuson-log overview acme.com --period last30d
```

## 5. Raporlar ve sorgu dili

39 hazır rapor vardır. Screaming Frog'un sekmelerine karşılık gelenler: **URL tarama özeti**
(son durum kodu, son tarama, son taramadan beri gün, boyut, hız, bot başına istek), **URL başına
durum kodları** (tutarsız yanıtlar, 1xx–5xx sütunları), **User agent'lar**, **Referer'lar**,
**Dizinler**, **İstemci IP'leri**, **Ülkeler**, **En büyük yanıtlar** (bayt ve tahmini CO2),
**İçerik türleri**. Ayrıca: 4xx/5xx sayfalar, yönlendirmeler, Googlebot tarama sıklığı,
**uzun süredir taranmayan URL'ler**, YZ botları ve **YZ botu hataları**, **YZ asistanlarından
gelen ziyaretler** ve giriş sayfaları, **kaynağa göre insan trafiği**, **bot başına bant
genişliği ve CO2**, sahte botlar ve **sahte botların ağları**, ağlar (ASN), tutarsız durum
kodları, yavaş URL'ler, parametreli URL'ler…

Masaüstünde filtreler **form** ile kurulur: her satırda bir alan (URL, durum kodu, boyut,
yanıt süresi, bot, tarih, içerik türü, referer türü, ülke…), bir işlem (içerir, eşittir, ile
başlar, aralıkta, son N gün…) ve değer(ler) seçilir. Sayılar ve tarihler için **en az / en çok**
kutuları vardır; birini boş bırakmak o tarafı sınırsız yapar. "Tüm koşullar / herhangi bir
koşul" seçimi koşulları VE ya da VEYA ile birleştirir. Gruplama, ölçümler, sıralama ve satır
sınırı da formdan seçilir.

Bot, user agent, URL, durum kodu, yöntem, IP, host, ülke gibi alanlarda değer kutusuna
tıklayınca sitenin verisinden **en sık görülen değerler** istek sayılarıyla listelenir; yazdıkça
liste daralır (büyük/küçük harf fark etmez, `*` joker olarak kullanılabilir). "Eşittir / eşit
değil" işlemlerinde birden fazla değer işaretlenebilir (herhangi biri eşleşir). Öneriler seçili
tarih ve trafik filtrelerine göre hesaplanır.

Öneriler yalnızca kısayoldur; **her kutuya istenen değer yazılabilir**. Yazılan değer Enter, Tab
ya da kutudan çıkınca (ör. doğrudan "Çalıştır"a basınca) eklenir. `*` her metinle eşleşir:
`googlebot*` tüm Googlebot türlerini, `*/amp` AMP sayfalarını, `*curl*` içinde curl geçen user
agent'ları bulur; joker ve tam değerler aynı listede karışık kullanılabilir. Excel'den
kopyalanan bir URL listesi yapıştırılırsa her satır ayrı değer olur. Sayı alanlarında (durum
kodu gibi) sayı olmayan değerler kırmızı gösterilir ve düzeltilene kadar sorguya girmez.

Form, altındaki **sorgu metnini** yazar; metin her zaman görünür ve elle de düzenlenebilir.
Metin değiştirildiğinde form kendini metne göre günceller. İç içe parantezli gibi formda
gösterilemeyen sorgular yalnızca metinden düzenlenir. Örnek sorgu metinleri:

```text
bot:googlebot* verification:verified status:4xx date:last30d group:url metric:hits,last_seen
group:url,content_type metric:hits,last_status,days_since_last,googlebot_hits,bingbot_hits
referer_type:ai -bot:* group:url,referer_host metric:hits
verification:spoofed group:bot,as_org,country metric:hits,unique_ips
```

- Boşlukla ayrılan koşulların hepsi sağlanmalıdır; `OR`, `-`/`NOT` ve parantez kullanılabilir.
- `group:` gruplar, `metric:` ölçer, `sort:` sıralar, `limit:` sınırlar.
- Yeni alanlar: `content_type` (uzantıdan tahmin: html, image, js…), `referer_host`,
  `referer_type` (doğrudan, site içi, arama, **YZ asistanları**, sosyal, e-posta, diğer),
  `country`, `asn`, `as_org` (IP konum verisi gerekir, bkz. 9. bölüm).
- Yeni ölçümler: `last_status`, `days_since_last`, `errors`, `hits_1xx`…`hits_5xx`,
  `inconsistent`, `avg_bytes`, `co2_mg` (tahmini), `bot_hits`, `search_hits`, `ai_hits`,
  `googlebot_hits`, `googlebot_smartphone_hits`, `googlebot_desktop_hits`, `bingbot_hits`,
  `yandex_hits`.
- Ayrıntılar: [query-language.md](query-language.md).

Sonuçlar CSV, Excel (XLSX) veya JSON olarak dışa aktarılır. CSV'de formül enjeksiyonuna karşı
koruma vardır (log satırları saldırganın kontrolündedir).

```sh
octopuson-log query acme.com --template googlebot-errors -o googlebot-hatalari.xlsx
octopuson-log query acme.com --template ai-referrals 'date:last30d'
```

### Kayıtlı sorgular

Sık kullandığınız filtreleri adlandırıp kaydedin; tüm sitelerde kullanılabilirler:

- Masaüstü: **Raporlar** → sorguyu kurun → **Kaydet…** → ad ve açıklama. Tarih ve trafik
  seçimleri de kaydedilir. Kayıtlar listenin üstündeki **Kayıtlı sorgularım** bölümünde
  görünür; tıklamak sorguyu yükleyip çalıştırır. Açtığınız kaydı değiştirip aynı adla
  kaydetmek onu günceller; başka bir kaydın adını kullanırsanız üzerine yazmadan önce sorulur.
- Ekip içinde paylaşım: bölüm başlığındaki simgelerle **dosyaya aktar** / **dosyadan al**
  (JSON). Aynı adlı kayıtlar içe aktarırken korunur.

```sh
octopuson-log saved add "Googlebot 404" 'bot:googlebot* verification:verified status:404 group:url'
octopuson-log saved list
octopuson-log query acme.com --saved "Googlebot 404" 'date:last7d'   # ek koşul eklenebilir
octopuson-log saved export ekip-sorgulari.json
octopuson-log saved import ekip-sorgulari.json        # --force: aynı adlıların üzerine yaz
```

## 6. robots.txt uyumu

**Robots.txt** sekmesi (CLI: `octopuson-log robots`) loglardaki her bot isteğini robots.txt ile
karşılaştırır. Kurallar Google'ın uyguladığı gibi değerlendirilir: botun adına en özel uyan
`User-agent` grubu (yoksa `*`), en uzun eşleşen kural, eşitlikte `Allow`; `*` ve `$` desteklenir.

- **Bot başına**: hangi grubun uygulandığı, robots.txt'nin botu tüm sitede engelleyip
  engellemediği (ör. GPTBot `Disallow: /`), toplam ve yasaklı istekler, doğrulanmış IP'lerden
  gelen yasaklı istekler (gerçek botun kuralı yok sayması), yasaklı URL sayısı.
- **Yasaklı istekler listesi**: URL, bot, doğrulama, istek, son görülme ve karar veren kural;
  Excel/CSV olarak dışa aktarılabilir.
- Kullanıcı tetiklemeli istekler (ChatGPT-User gibi) ayrıca işaretlenir; sahipleri bu isteklere
  robots.txt uygulamayabilir.

Varsayılan olarak sitenin canlı `https://<alan adı>/robots.txt` dosyası indirilir; bir dosya veya
başka bir URL de verilebilir. Güncel robots.txt tüm döneme uygulanır.

```sh
octopuson-log robots acme.com --filter 'date:last30d verification:verified' -o robots-ihlalleri.xlsx
octopuson-log robots acme.com --robots eski-robots.txt
```

## 7. URL listesi karşılaştırma

Bir tarama dışa aktarımı (Screaming Frog CSV'sinde `Address` sütunu otomatik bulunur), düz URL
listesi veya XML site haritası (site haritası dizinleri ve `.gz` dahil, dosya veya URL olarak)
loglarla karşılaştırılır:

- **ikisinde de**: listede var ve botlar istemiş.
- **loglarda yok**: listede var ama seçilen trafik (varsayılan: doğrulanmış arama motorları)
  hiç istememiş — taranmayan sayfalar.
- **listede yok**: istenmiş ama listede olmayan URL'ler — yetim sayfalar, kaldırılmış
  sayfalar, parametre varyantları.

CSV dışa aktarımlarının **diğer sütunları** (indexability, inlinks, title, derinlik…) ve site
haritalarının `lastmod` değerleri sonuç tablosuna eklenir (Excel'deki DÜŞEYARA gibi), böylece
tarama verisi log verisinin yanında görünür ve sıralanır. Masaüstü, eklediğiniz dosya ve
URL'leri site başına hatırlar.

URL'ler karşılaştırılmadan önce normalleştirilir (şema/host, varsayılan port, yüzde kodlama,
`utm_*`, `gclid` gibi izleme parametreleri). İsteğe bağlı: tüm parametreleri yok sayma, sondaki
`/` ve büyük/küçük harf farkını yok sayma.

```sh
octopuson-log compare-urls acme.com https://acme.com/sitemap.xml tarama.csv -o karsilastirma.xlsx
```

## 8. Dönem karşılaştırma

İki dönemi URL, dizin, bot, durum kodu vb. bazında karşılaştırır: yeni, kaybolan, artan,
azalan.

```sh
octopuson-log compare-periods acme.com --before 2026-09-01..2026-09-30 \
  --after 2026-10-01..2026-10-31 --group directory --filter 'bot:googlebot* verification:verified'
```

## 9. IP konum verisi (ülke ve ağ)

**Ayarlar** → **IP konum verisi** → *İndir* (CLI: `octopuson-log geo update`) ücretsiz DB-IP Lite
ülke ve ASN veritabanlarını (yaklaşık 20 MB, ayda bir güncellenir) indirir. Ardından her istemci
IP'sinin ülkesi ve ağı (ör. *Hetzner Online GmbH*, *Amazon.com, Inc.*) sorgularda kullanılabilir:
**Ülkeler**, **Ağlar (ASN)** ve **Sahte botların ağları** raporları; `country:TR`,
`as_org:*Hetzner*` filtreleri. Aramalar yerelde yapılır; hiçbir IP bilgisayarınızdan çıkmaz.
Veri lisansı (CC BY 4.0) gereği sonuçlarda "IP Geolocation by DB-IP" atfı gösterilir.

## 10. Veri klasörü ve kaynaklar

- Varsayılan veri klasörü: macOS `~/Library/Application Support/com.octopusSEO.octopusSEO-Log-Analyser`,
  Windows `%APPDATA%\octopusSEO\octopusSEO Log Analyser\data`, Linux `~/.local/share/octopusseologanalyser`.
  SEONexus adıyla kurulmuş eski sürümlerin verisi varsa uygulama eski klasörü kullanmaya devam eder.
  `OCTOPUSSEO_DATA_DIR` ile değiştirilebilir. Özel botlar (`custom-bots.json`), indirilen bot IP
  listeleri ve IP konum verisi (`geo/`) de burada durur.
- Depolama motoru belleği varsayılan 1 GiB ile sınırlıdır (`OCTOPUSSEO_MEMORY_LIMIT_MB`); daha
  büyük işler diske taşar.
- Bir siteye aynı anda tek içe aktarma yapılabilir; içe aktarma sürerken o sitede sorgular
  "site meşgul" uyarısı verir.
- `co2_mg` bir tahmindir: Sustainable Web Design modeli v3 (GB başına 0,81 kWh, küresel ortalama
  442 g CO2e/kWh), aktarılan baytlardan hesaplanır.
