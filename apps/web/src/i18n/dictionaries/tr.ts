import type { Dictionary } from "./en";

export const tr: Dictionary = {
  meta: {
    title: "octopusSEO Log Analyser — Google ve yapay zekâ botları sitenizi nasıl tarıyor?",
    description:
      "SEO log analizi için ücretsiz masaüstü uygulaması ve komut satırı aracı: doğrulanmış Googlebot ve YZ botu etkinliği, tarama bütçesi, hatalar, robots.txt uyumu ve yetim sayfalar. Windows, macOS ve Linux.",
  },
  nav: {
    features: "Özellikler",
    download: "İndir",
    docs: "Dokümanlar",
    changelog: "Sürüm notları",
    github: "GitHub",
    menu: "Menü",
    language: "Dil",
  },
  hero: {
    badge: "Yeni: robots.txt uyumu ve YZ yönlendirme raporları",
    title: "Sunucu loglarınız biliyor:",
    highlight: "Google ve yapay zekâ botları gerçekte ne yapıyor",
    subtitle:
      "octopusSEO Log Analyser ham erişim loglarını tarama içgörüsüne çevirir: Googlebot ve YZ botları hangi sayfaları istiyor, nerede hata alıyor, hangi botlar sahte ve tarama bütçeniz nereye gidiyor.",
    primary: "{os} için indir",
    primaryGeneric: "Ücretsiz indir",
    secondary: "Özellikleri keşfet",
    note: "Ücretsiz · Windows, macOS ve Linux · Loglarınız bilgisayarınızdan çıkmaz",
  },
  mockup: {
    title: "acme.com — son 30 gün",
    hits: "Doğrulanmış arama istekleri",
    urls: "Taranan URL",
    errors: "Tarama hataları",
    spoofed: "Sahte bot istekleri",
    chart: "Bota göre günlük tarama",
    table: "Bot",
    verified: "doğrulandı",
    fake: "sahte",
  },
  formats: {
    title: "Elinizdeki logları okur — format otomatik algılanır",
  },
  features: {
    eyebrow: "Hepsi tek yerde",
    title: "Ham log satırlarından SEO kararlarına",
    subtitle: "Her ay onlarca siteyi analiz eden ajanslar için SEO'cular tarafından geliştirildi.",
    items: {
      verification: {
        title: "Kullanıcı aracısı değil, doğrulanmış bot",
        text: "110'dan fazla tarayıcı tanınır ve operatörlerin resmi IP listeleri ile ters DNS üzerinden doğrulanır. Sahte Googlebot'lar sayılmaz, işaretlenir.",
      },
      ai: {
        title: "Yapay zekâ görünürlüğü",
        text: "GPTBot, ClaudeBot, PerplexityBot ve ChatGPT-User neyi istiyor görün — ChatGPT, Perplexity ya da Gemini'nin size gönderdiği ziyaretlerle birlikte.",
      },
      robots: {
        title: "robots.txt uyumu",
        text: "Hangi botlar yasaklı URL'leri istiyor, robots.txt'niz hangilerini tamamen engelliyor — Google'ın değerlendirdiği şekilde.",
      },
      budget: {
        title: "Tarama bütçesi",
        text: "URL ve dizin başına tarama sıklığı, son taramadan beri geçen gün, parametreli URL'ler, yönlendirmeler ve bütçeyi tüketen statik dosyalar.",
      },
      orphans: {
        title: "Yetim ve taranmayan sayfalar",
        text: "Tarama dışa aktarımlarını ve XML site haritalarını loglarla eşleştirin; diğer sütunlar DÜŞEYARA gibi eklenir.",
      },
      reports: {
        title: "39 rapor + sorgu dili",
        text: "Hazır raporlar ve kendi filtreleriniz için basit bir form. Kaydedin, paylaşın; Excel, CSV ya da JSON olarak dışa aktarın.",
      },
      geo: {
        title: "Ülkeler ve ağlar",
        text: "Her istemci IP'sinin ülkesi ve ağı (ASN), kazıyıcı bulutlarını ve sahte botların nereden geldiğini gösterir. Aramalar yerelde yapılır.",
      },
      alerts: {
        title: "Tarama uyarıları",
        text: "Googlebot'taki ani düşüşler, YZ taramasındaki sıçramalar ve sunucu hatası patlamaları genel bakışta öne çıkar.",
      },
    },
  },
  ai: {
    eyebrow: "Yapay zekâ araması geldi",
    title: "YZ asistanlarının sayfalarınızı ne zaman okuduğunu — ve önerdiğini — bilin",
    text: "Eğitim tarayıcıları, YZ arama dizinleyicileri ve asistanların anlık istekleri ayrı tutulur; hangi içeriğin modelleri beslediğini, hangisinin alıntılandığını ve YZ asistanlarının geri gönderdiği gerçek ziyaretçileri görürsünüz.",
    bullets: [
      "YZ eğitimi, YZ arama ve YZ asistanları için ayrı kategoriler",
      "YZ botlarına dönen hatalar — başarısız her istek kaybedilmiş bir alıntıdır",
      "ChatGPT, Perplexity, Gemini, Copilot ve Claude'dan gelen ziyaretler",
      "Tarayıcı başına bant genişliği ve tahmini CO₂",
    ],
  },
  terminal: {
    eyebrow: "Otomatikleştirin",
    title: "Her rapor için bir komut",
    text: "snx-log komut satırı aracı masaüstü uygulamasıyla aynı veriyi kullanır. Aylık raporları betiğe dökün, sunucularınızda içe aktarmayı zamanlayın, doğrudan Excel'e aktarın.",
    points: ["Saniyede ~750.000 satır", "Tamamen bilgisayarınızda çalışır"],
  },
  steps: {
    title: "İlk içgörüye üç adımda",
    items: [
      { title: "Loglarınızı bırakın", text: "Düz metin, gzip, zstd, bzip2 veya zip — dosya ya da klasör. Tekrar içe aktarmak güvenlidir." },
      { title: "Botları biz doğrularız", text: "Her bot isteği resmi verilerle doğrulandı, sahte ya da doğrulanmadı olarak işaretlenir." },
      { title: "Raporlarla harekete geçin", text: "Botların aldığı hataları düzeltin, tarama israfını durdurun, önemli sayfaları açın ve YZ görünürlüğünü izleyin." },
    ],
  },
  stats: {
    bots: "tanınan tarayıcı",
    formats: "log formatı",
    reports: "hazır rapor",
    speed: "satır / saniye",
  },
  faq: {
    title: "Sık sorulan sorular",
    items: [
      {
        q: "octopusSEO Log Analyser gerçekten ücretsiz mi?",
        a: "Evet. Masaüstü uygulaması ve komut satırı aracı ücretsiz ve açık kaynaklıdır; log satırı ya da site sınırı yoktur.",
      },
      {
        q: "Loglarım bilgisayarımdan çıkıyor mu?",
        a: "Hayır. Her şey yerelde saklanır ve analiz edilir. İndirilen tek şey herkese açık bot IP listeleri ve isterseniz IP konum veritabanıdır.",
      },
      {
        q: "Hangi log formatları destekleniyor?",
        a: "Nginx ve Apache (özel formatlar dahil), IIS, CloudFront, AWS yük dengeleyicileri, HAProxy, Traefik, Caddy, Cloudflare, Google Cloud, JSON ve CSV loglar. Format otomatik algılanır.",
      },
      {
        q: "Loglarım ne kadar büyük olabilir?",
        a: "Yüz milyonlarca satır. Loglar saniyede yaklaşık 750.000 satır hızla içe aktarılır ve özgün boyutunun yaklaşık %8'i kadar sıkıştırılarak saklanır.",
      },
      {
        q: "Uygulama hangi dilleri destekliyor?",
        a: "Masaüstü uygulaması Türkçe ve İngilizce kullanılabilir; raporlar ve dışa aktarmalar iki dilde de aynı çalışır.",
      },
    ],
  },
  cta: {
    title: "Sitenizi Googlebot'un gözünden görün",
    text: "Uygulamayı indirin, bir log dosyası bırakın, ilk raporunuzu dakikalar içinde alın.",
    button: "Ücretsiz indir",
  },
  footer: {
    tagline: "Arama motorlarının ve yapay zekânın sitenizi gerçekte nasıl gördüğünü gösteren SEO yazılımları.",
    product: "Ürün",
    resources: "Kaynaklar",
    company: "Şirket",
    logAnalyser: "Log Analyser",
    spider: "SEO Spider",
    comingSoon: "yakında",
    contact: "İletişim",
    issues: "Sorun bildir",
    rights: "© {year} octopusSEO. Ücretsiz ve açık kaynak yazılım.",
    geo: "Ülke ve ağ verisi: IP Geolocation by DB-IP",
  },
  featuresPage: {
    title: "Özellikler",
    subtitle: "Log Analyser'ın yaptığı her şey, ayrıntılarıyla.",
    sections: {
      bots: {
        title: "Bot tanıma ve doğrulama",
        text: "User-agent taklit etmek çok kolaydır. Her bot isteği doğrulanır; raporlar yalnızca gerçek tarayıcıları sayar.",
        bullets: [
          "Kategorilere ayrılmış 110'dan fazla tarayıcı: arama, YZ eğitimi, YZ arama, YZ asistanları, reklam, SEO araçları, sosyal, izleme",
          "Google, Bing, OpenAI, Anthropic, Perplexity, Apple ve diğerlerinin resmi IP listeleri ile ileri doğrulamalı ters DNS",
          "İstek başına karar: doğrulandı, sahte ya da doğrulanmadı — dilediğiniz zaman yeniden kontrol",
          "Kendi botlarınız: IP aralıkları, IP listesi URL'leri ya da ters DNS ile özel user-agent desenleri",
        ],
      },
      ai: {
        title: "YZ tarayıcıları ve YZ araması",
        text: "YZ platformlarının içeriğinizi nasıl kullandığını — ve karşılığında ne gönderdiğini — anlayın.",
        bullets: [
          "Her YZ tarayıcısının günlük etkinliği ve istediği URL'ler",
          "YZ botlarına ve asistanlarına dönen hatalar",
          "YZ asistanlarından gelen insan ziyaretleri ve giriş sayfaları",
          "Bot başına bant genişliği ve tahmini emisyon",
        ],
      },
      robots: {
        title: "robots.txt uyumu",
        text: "Loglardaki her bot isteğini canlı robots.txt'nizle ya da herhangi bir sürümüyle karşılaştırın.",
        bullets: [
          "Google tarzı değerlendirme: en özel user-agent grubu, en uzun eşleşme, eşitlikte Allow",
          "Bot başına yasaklı istekler — doğrulanmış ve sahte IP'lerden",
          "YZ eğitim tarayıcıları gibi tüm sitede engellenen botlar",
          "Her ihlali karar veren kuralla birlikte dışa aktarın",
        ],
      },
      crawl: {
        title: "Tarama bütçesi ve teknik sağlık",
        text: "Tarayıcıların nerede zaman kaybettiğini ve neye ulaşamadığını bulun.",
        bullets: [
          "URL özeti: son durum kodu, son tarama, son taramadan beri gün, boyut, hız ve bot başına istek",
          "1xx–5xx sütunları ve tutarsız yanıtlarla URL başına durum kodları",
          "Yönlendirmeler, 4xx/5xx, yavaş ve büyük URL'ler, parametreli URL'ler, statik dosyalar",
          "Tarama düşüşleri, sıçramaları ve sunucu hatası patlamaları için uyarılar",
        ],
      },
      urls: {
        title: "Taranmayan ve yetim sayfalar",
        text: "Tarama verinizi botların gerçekte yaptıklarıyla birleştirin.",
        bullets: [
          "Tarayıcı CSV dışa aktarımları, düz listeler ve XML site haritaları (site haritası dizinleri dahil)",
          "Listede olup hiç taranmayan, taranıp listede olmayan (yetim) sayfalar",
          "Dışa aktarımınızın diğer tüm sütunları log verisinin yanında",
          "URL, dizin ya da bot bazında dönem karşılaştırması — taşıma öncesi ve sonrası",
        ],
      },
      data: {
        title: "Verileriniz, sizin kurallarınız",
        text: "Hızlı, gizli ve betiklenebilir.",
        bullets: [
          "Formatlar otomatik algılanır; özel Nginx ve Apache formatları desteklenir",
          "Site başına içe aktarma ayarları: yalnızca botlar, URL dahil/hariç, parametre temizleme",
          "Kayıtlı sorgularla sorgu dili; Excel, CSV ve JSON dışa aktarma",
          "Tüm siteyi tek arşiv olarak paylaşın; otomasyon için komut satırı aracı",
        ],
      },
    },
  },
  download: {
    title: "octopusSEO Log Analyser'ı indirin",
    subtitle: "Windows, macOS ve Linux için ücretsiz. Otomasyon için komut satırı aracı da dahil.",
    recommended: "Sisteminiz için önerilen",
    version: "Sürüm {version}",
    released: "{date} tarihinde yayımlandı",
    totalDownloads: "{count} indirme",
    desktop: "Masaüstü uygulaması",
    cli: "Komut satırı (snx-log)",
    file: "Dosya",
    size: "Boyut",
    checksum: "SHA-256",
    getIt: "İndir",
    unavailableTitle: "İndirmeler çok yakında",
    unavailableText:
      "İlk herkese açık sürüm hazırlanıyor. Bu arada projeyi takip edebilir ya da kaynak koddan derleyebilirsiniz.",
    allReleases: "Tüm sürümler",
    sourceTitle: "Kaynak koddan derleyin",
    sourceText: "Rust (stable) ve masaüstü uygulaması için Node.js 22+ ile pnpm gerekir.",
    requirementsTitle: "Sistem gereksinimleri",
    requirements: [
      "Windows 10/11, macOS 11 veya üzeri (Apple Silicon ve Intel) ya da güncel bir Linux dağıtımı",
      "4 GB RAM; ham log boyutunuzun yaklaşık %10'u kadar disk alanı",
      "İnternet yalnızca bot IP listesi ve konum verisi güncellemeleri için",
    ],
    platforms: {
      macos: "macOS",
      windows: "Windows",
      linux: "Linux",
    },
    arch: { universal: "Evrensel", arm64: "ARM64", x64: "x64", arm32: "ARM 32 bit", appleSilicon: "Apple Silicon", intel: "Intel" },
    kinds: {
      dmg: "Disk görüntüsü",
      exe: "Kurulum programı",
      msi: "MSI paketi",
      appimage: "AppImage",
      deb: "Debian / Ubuntu paketi",
      rpm: "Fedora / RHEL paketi",
      exeOffline: "Çevrimdışı WebView2 içeren kurulum",
      cli: "Komut satırı arşivi",
    },
  },
  docs: {
    title: "Dokümantasyon",
    subtitle: "Dakikalar içinde başlayın, sonra sorgu diliyle derinleşin.",
    quickstart: "Hızlı başlangıç",
    quickstartText: "Bir müşteri ve site oluşturun, logları içe aktarın ve bir rapor açın. Masaüstü uygulaması aynısını birkaç tıklamayla yapar.",
    query: "Sorgu dili",
    queryText: "Her rapor bir sorgudur: önce koşullar, ardından isteğe bağlı gruplama, ölçümler, sıralama ve sınır.",
    examples: "Örnekler",
    exampleItems: [
      { query: "bot:googlebot* verification:verified status:4xx date:last30d group:url", text: "Doğrulanmış Googlebot'un son 30 günde aldığı hatalar" },
      { query: "bot_category:ai-* group:bot,day metric:hits", text: "YZ tarayıcılarının günlük etkinliği" },
      { query: "referer_type:ai -bot:* group:url,referer_host", text: "YZ asistanlarından gelinen sayfalar" },
      { query: "verification:spoofed group:bot,as_org,country", text: "Sahte botların nereden geldiği" },
      { query: "group:url metric:last_status,days_since_last,googlebot_hits", text: "URL başına son tarama" },
    ],
    fields: "Alanlar",
    metrics: "Ölçümler",
    more: "GitHub'da tam başvuru",
    guide: "Kullanım kılavuzu",
  },
  changelog: {
    title: "Sürüm notları",
    subtitle: "Her sürümdeki yeni özellikler ve düzeltmeler.",
    empty: "Henüz yayımlanmış sürüm yok.",
    viewOnGithub: "GitHub'da gör",
  },
  notFound: {
    title: "Sayfa bulunamadı",
    text: "Bu sayfa yok — belki bir tarayıcı uydurmuştur.",
    home: "Ana sayfaya dön",
  },
  common: {
    copy: "Kopyala",
    copied: "Kopyalandı",
    learnMore: "Daha fazla",
  },
};
