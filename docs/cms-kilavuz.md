# OctopusOn SEO İçerik Yönetimi — Kullanım kılavuzu

İçerik yönetimi (`apps/cms`), OctopusOn SEO web sitesinin bütün sayfalarını yönetir. Sayfalar, kategoriler, blog yazıları, dokümanlar, menüler, diller ve yönlendirmeler buradan değişir; kod değişikliği ya da yeniden yayın gerekmez.

Kurulum ve teknik ayrıntılar için [cms.md](cms.md), tasarım kararları için [ADR 0073](adr/0073-website-cms.md).

## 1. Giriş

Yerelde `http://localhost:8000/admin`, sunucuda `https://cms.octopuson.com/admin` adresinden e-posta ve parolanızla girin.

- Kayıt ekranı yoktur. Yeni kullanıcıyı **Kullanıcılar → Kullanıcı ekle** ekranından ya da `php artisan cms:admin` komutuyla ekleyin.
- Art arda yanlış parola girilirse giriş bir dakika kilitlenir.

## 2. Sayfalar ve ağaç

**Sayfalar** ekranı sitenin ağacıdır. Her şey bir sayfadır ve türü onu ayırır:

| Tür | Ne için |
|---|---|
| Ana sayfa | Dilin kök adresi: `/tr`. Yalnızca bir tane olur. |
| Sayfa | Bloklarla kurulan serbest sayfa: Özellikler, İndir. |
| Kategori | Alt sayfalarını kendiliğinden listeler: Blog, Dokümanlar, ürün grupları. Ayrı bir kategori tablosu yoktur. |
| Ürün | Bir OctopusOn SEO programının sayfası. |
| Yazı | Blog yazısı; kategorisinde yayın tarihine göre, en yenisi önde listelenir. |
| Doküman | Doküman sayfası; kategorisinde sırasına göre listelenir. |

Ekrandaki düğmeler:
- **Alt sayfa** düğmesi o sayfanın altına yeni sayfa açar. Kategorinin altında tür kendiliğinden "Yazı" gelir.
- Oklar bir sayfayı kardeşleri arasında yukarı ya da aşağı taşır. Menülerdeki sıra da budur.
- Dil rozetleri her dilin durumunu gösterir: **yeşil** yayında, **sarı** taslak, **kesik çizgili** çeviri yok. Bir rozete tıklayınca o dildeki sürüm açılır; yoksa oluşturulur.

**Sayfa ayarları** sekmesinde tüm dillerde ortak alanlar vardır:
- tür ve üst sayfa;
- **Program**: ürün kartlarında ve indirme sayfasında bu sayfa o programı temsil eder;
- ikon;
- **Üst menüde göster** ve **Alt menü sütunu**: Ürün, Kaynaklar ya da Şirket.

Bir sayfayı başka bir sayfanın altına taşıdığınızda:
- Adresleri tüm dillerde yenilenir ve eski adresler 301 ile yeni adreslere yönlenir.
- Yeni üst sayfanın, taşınan sayfanın her dilinde çevirisi olmalıdır.
- Alt sayfası olan bir sayfa silinemez.

## 3. Bir dildeki sayfa

Sayfanın her dili ayrı düzenlenir ve ayrı yayınlanır.

- **Başlık:** Sayfanın adı. Başlık bloğu boşsa sayfanın büyük başlığı budur.
- **Adres:** Bu dildeki kısa ad. Türkçe karakterler kendiliğinden dönüşür ("Sürüm notları" → `surum-notlari`) ve üst sayfanın adresine eklenir: `/tr/blog/log-analizi-rehberi`.
  - Adres değişirse eski adres yeni adrese otomatik 301 ile yönlenir.
  - Altındaki sayfaların adresleri de birlikte değişir.
- **Menü adı:** Menülerde, kartlarda ve breadcrumb'da başlık yerine kullanılır ("Özellikler").
- **Özet:** Kategori listelerinde ve ürün kartlarında görünür. SEO açıklaması boşsa o da budur.
- **Yayın:**
  - **Taslak** sitede görünmez.
  - **Yayında** hemen görünür.
  - İleri bir **yayın tarihi** seçerseniz sayfa o an yayına girer.
- **Önizle:** Taslağı sitede, yayınlamadan gösterir. Bağlantı bir saat geçerlidir. Sayfanın üstündeki pembe şeritten önizlemeden çıkarsınız.
- **SEO:**
  - Google sonuç önizlemesi ve karakter sayaçları yol gösterir: başlık 30–60, açıklama 70–160 karakter önerilir.
  - **Paylaşım görseli** (Open Graph) için 1200×630 önerilir. Kategori kartlarında da görünür.
  - **Kanonik adres**i yalnızca içerik başka bir adreste de varsa doldurun.
  - **noindex** sayfayı arama motorlarından ve site haritasından çıkarır.

Yeni bir çeviri açtığınızda alanlar varsayılan dilden kopyalanır; metinleri çevirip kaydedin. Bir sayfanın çevirisi, üst sayfası o dile çevrilmeden kaydedilemez.

## 4. İçerik blokları

Sayfanın gövdesi bloklardan kurulur:
- **Blok ekle** listesinden bir tür seçin.
- Oklarla sırasını değiştirin, kopya düğmesiyle çoğaltın.
- Başlığa tıklayarak açıp kapatın.

| Blok | Ne gösterir |
|---|---|
| Giriş (hero) | Ana sayfanın büyük girişi: rozet, başlık, işletim sistemine göre indirme düğmesi, görsel. |
| Sayfa başlığı | Başlık ve giriş cümlesi; isterseniz bölüm bağlantıları ve programın indirme düğmesi. |
| Özellik kartları | İkonlu kart ızgarası. |
| Özellik bölümleri | Sağa sola dizilen bölümler. **Bağlantı adı** verilen bölümler başlıktaki çiplerde görünür. |
| Metin ve görsel, Adımlar, Rakamlar, Komut satırı | Ana sayfadaki bölümler. |
| Program kartları | Programı seçili sayfaların kartları. Metin o sayfaların özetidir. |
| Karşılaştırma tablosu | Bir programı başka bir araçla satır satır karşılaştırır: her satırda iki taraf için durum (var, kısmen ya da ücretli, yok) ve kısa açıklama. Rakibin bilgilerinin kaynağını ve tarihini dipnoda yazın. |
| Sık sorulan sorular | Açılır sorular; Google için FAQ yapılandırılmış verisi de eklenir. |
| Çağrı kutusu | Sayfa sonundaki büyük kutu ve düğme. |
| Metin (Markdown) | Yazılar ve dokümanlar için serbest metin. |
| Görsel, Kod, Bilgi notu, Bağlantılar | Tek görsel, kopyalanabilir kod, çerçeveli not, ok işaretli bağlantılar. |
| Tüm indirmeler, Program indirmeleri, Sürüm notları | GitHub sürümlerinden kendiliğinden dolan bölümler. Blokta yalnızca çevresindeki metinler yazılır. |
| Alt sayfalar | Sayfanın alt sayfalarını listeler. Kategoriler bu blok olmadan da listeler. |

Markdown blokta şunlar kullanılabilir:
- başlıklar (`## Başlık`), maddeli ya da numaralı listeler;
- `**kalın**` ve `` `kod` ``;
- site içi ya da dış bağlantı: `[metin](/tr/ozellikler)`;
- görsel: `![açıklama](görsel adresi)`. Görsel adresini **Medya** ekranındaki **Adresi kopyala** düğmesinden alın.

## 5. Blog ve kategoriler

1. **Sayfalar → Yeni sayfa** ekranında türü **Kategori** seçin, örneğin "Blog". Gerekirse her dilde çevirisini oluşturup yayınlayın.
2. Kategorinin satırındaki **Alt sayfa** düğmesiyle yazı ekleyin: başlık, özet, paylaşım görseli ve bir Metin bloğu.
3. Yayınlanan yazılar kategoride listelenir. Sayfa başına 12 yazı gösterilir; sonrakiler `/tr/blog/page/2` adresindedir.

Ürün grupları ya da doküman bölümleri de aynı şekilde kurulur: bir kategori ve altındaki sayfalar.

Karşılaştırma sayfaları da böyledir: **Karşılaştırmalar** kategorisinin (`/tr/karsilastirma`) altında her rakip için bir sayfa. Sayfanın SEO başlığına aranan ifadeyi yazın: “Ücretsiz Screaming Frog SEO Spider Alternatifi”.

## 6. Menüler

Menüler sayfaların kendisinden oluşur:
- **Üst menü:** **Sayfa ayarları**nda "Üst menüde göster" işaretli sayfalar.
- **Alt menü:** "Alt menü sütunu" seçilmiş sayfalar.
- Sıra, sayfaların ağaçtaki sırasıdır.
- Menüde **Menü adı**, o yoksa başlık görünür. Ürün sütununda programı seçili sayfalar program adıyla görünür.
- Üstteki **İndir** düğmesi, **Tüm indirmeler** bloğunu içeren sayfaya gider.

## 7. Diller

**Diller** ekranından dil eklenir.

- **Kod** adresin ön ekidir (`/fr/…`) ve hreflang değeridir. Çevirisi olan bir dilin kodu değiştirilemez.
- Yeni dili ekledikten sonra sayfaların o dildeki çevirilerini oluşturun. Çevrilmeyen sayfalar o dilde yoktur.
- Sitenin düğme ve etiket metinleri o dil için yazılana kadar İngilizce görünür.
- **Pasif** bir dil sitede görünmez ama çevirileri korunur.
- **Varsayılan** dil hreflang x-default'tur.

## 8. Medya

Görselleri (JPG, PNG, WebP, AVIF, GIF) ve PDF'leri yükleyin. Bir dosya en fazla 10 MB olabilir. SVG güvenlik nedeniyle kabul edilmez.

- Her görsele bir **alternatif metin** yazın.
- Kullanımdaki bir dosya silinemez: ekran hangi sayfalarda kullanıldığını söyler.

## 9. Yönlendirmeler

- Adresi değişen sayfaların 301'leri kendiliğinden eklenir ve sayfa nereye taşınırsa onu izler. Zincir oluşmaz.
- Başka bir siteden taşınan adresleri **Eski adres → Yeni adres** olarak elle ekleyin: `/eski-blog/yazi` → `/tr/blog/yazi`.
- **İstek** sütunu, yönlendirmenin kaç kez kullanıldığını gösterir.

## 10. Değişiklikler sitede ne zaman görünür?

Kaydettiğiniz an site haber alır ve sayfayı bir sonraki ziyarette yeniden oluşturur.

**Panel** ekranında "Site son değişiklikten sonra yenilenemedi" uyarısı varsa haber verilemedi demektir. O durumda:
- Değişiklik en geç bir saat içinde kendiliğinden görünür.
- Sunucu ayarlarında `CMS_REVALIDATE_URL` ve `CMS_REVALIDATE_SECRET` değerlerini kontrol edin.
