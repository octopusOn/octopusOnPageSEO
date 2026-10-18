// The legal texts of the credit service, in Turkish. DRAFTS: to be reviewed by a lawyer before
// sales open. Bracketed parts are business decisions or missing company details.

import type { Company } from "./company";

export interface LegalDocument {
  slug: string;
  title: string;
  sections: { heading: string; body: string[] }[];
}

const seller = (c: Company) =>
  `${c.title}, ${c.address}; vergi dairesi ve no: ${c.taxOffice} ${c.taxNumber}; MERSİS: ${c.mersis}; e-posta: ${c.email}; telefon: ${c.phone}; KEP: ${c.kep}.`;

export function legalDocuments(c: Company): LegalDocument[] {
  return [
    {
      slug: "kredi-kosullari",
      title: "OctopusOn SEO kredi kullanım koşulları",
      sections: [
        {
          heading: "1. Kredi nedir?",
          body: [
            "OctopusOn SEO kredisi, OctopusOn SEO masaüstü programlarında (OctopusOn SERP Checker, OctopusOn SEO Keyword Research ve ileride eklenecekler) ücretli veri kaynaklarına (ör. arama sonucu, arama hacmi, backlink verisi) OctopusOn SEO hesabı üzerinden erişmek için kullanılan, hesaba yüklenmiş bir kullanım bakiyesidir.",
            "Kredi yalnızca OctopusOn SEO hizmetleri içinde kullanılabilir. Başka bir kişiye devredilemez, satılamaz, nakde veya başka bir ödeme aracına çevrilemez; elektronik para veya ödeme aracı değildir.",
            "Programlar kendi API anahtarlarınızla krediye ihtiyaç duymadan ücretsiz çalışmaya devam eder. Kredi kullanımı isteğe bağlıdır.",
          ],
        },
        {
          heading: "2. Fiyat ve kullanım",
          body: [
            "Her kredili istekte, ilgili veri sağlayıcısının o istek için uyguladığı maliyete OctopusOn SEO hizmet bedeli eklenerek hesaplanan tutar bakiyenizden düşülür. Kaynak bazında güncel fiyat çarpanı programlarda ve hesabınızda gösterilir.",
            "İstek gönderilmeden önce tahmini en yüksek tutar ayrılır; yanıt geldiğinde gerçek tutar düşülür ve kalan serbest bırakılır. Sağlayıcının hata ile yanıtladığı isteklerden ücret alınmaz.",
            "Kredi, ödeme anındaki Türkiye Cumhuriyet Merkez Bankası döviz satış kuru ve hesap ekranında belirtilen kur payı esas alınarak Türk lirası ile satılır. Fiyatlara KDV dahildir.",
          ],
        },
        {
          heading: "3. Geçerlilik",
          body: ["[Karar: Kredilerin süresiz mi, yoksa son kullanımdan itibaren belirli bir süre (ör. 24 ay) mı geçerli olacağı burada belirtilecek.]"],
        },
        {
          heading: "4. Sınırlar ve kötüye kullanım",
          body: [
            "Hizmetin güvenliği için cihaz başına aylık harcama sınırı, kullanıcı başına günlük harcama sınırı ve istek hızı sınırları uygulanabilir.",
            "Hesabın veya kredilerin hileli ödeme, ters ibraz, veri sağlayıcılarının kullanım koşullarına aykırı kullanım ya da hizmeti aksatacak biçimde kullanılması hâlinde hesap askıya alınabilir; ters ibraz edilen ödemelere karşılık gelen krediler bakiyeden düşülür.",
            "Veri sağlayıcıları hizmetlerini değiştirebilir veya durdurabilir; böyle bir durumda ilgili kaynak kredi ile kullanılamaz hâle gelebilir. Kalan bakiyeniz diğer kaynaklarda kullanılabilir.",
          ],
        },
        {
          heading: "5. Gizlilik",
          body: [
            "Kendi anahtarlarınızla yaptığınız işlemler bilgisayarınızda kalır. Kredili modda istekleriniz yalnızca ilgili veri sağlayıcısına iletilmek üzere OctopusOn SEO ağ geçidinden geçer; istek ve yanıt içerikleri (anahtar kelimeler, sonuçlar) saklanmaz. Faturalama ve güvenlik için yalnızca kaynak, uç nokta türü, durum, süre ve tutar kaydedilir. Ayrıntılar KVKK aydınlatma metnindedir.",
          ],
        },
        { heading: "6. Satıcı", body: [seller(c)] },
      ],
    },
    {
      slug: "on-bilgilendirme",
      title: "Ön bilgilendirme formu",
      sections: [
        { heading: "1. Satıcı", body: [seller(c)] },
        {
          heading: "2. Hizmetin temel nitelikleri",
          body: [
            "Hizmet: OctopusOn SEO hesabına kredi yüklenmesi. Kredi, OctopusOn SEO programlarında ücretli veri kaynaklarına erişim için kullanılır (bkz. kredi kullanım koşulları).",
            "Satın alınan kredi miktarı, KDV dahil toplam fiyat ve kullanılan döviz kuru ödeme sayfasında ve sipariş özetinde gösterilir.",
          ],
        },
        {
          heading: "3. Fiyat ve ödeme",
          body: [
            "Fiyatlar Türk lirası cinsinden olup KDV dahildir. Ödeme, lisanslı ödeme kuruluşu aracılığıyla kredi kartı veya banka kartı ile tek çekim olarak alınır; kart bilgileriniz OctopusOn SEO tarafından görülmez ve saklanmaz.",
          ],
        },
        {
          heading: "4. İfa",
          body: ["Ödemenin onaylanmasının ardından kredi hesabınıza derhâl yüklenir. Fatura (e-Arşiv veya e-Fatura) fatura bilgilerinize göre düzenlenir ve hesabınızda görüntülenir."],
        },
        {
          heading: "5. Cayma hakkı",
          body: [
            "Mesafeli Sözleşmeler Yönetmeliği'nin 15. maddesinin birinci fıkrasının (ğ) bendi uyarınca, elektronik ortamda anında ifa edilen hizmetlere ilişkin sözleşmelerde cayma hakkı kullanılamaz. Kredi, onayınızla ödemeden hemen sonra hesabınıza yüklendiğinden cayma hakkınız bulunmamaktadır. Ödeme adımında bu hususu ayrıca onaylarsınız.",
          ],
        },
        {
          heading: "6. Şikâyet ve itiraz",
          body: [
            `Talep ve şikâyetlerinizi ${c.email} adresine iletebilirsiniz. Uyuşmazlıklarda, Ticaret Bakanlığınca her yıl ilan edilen parasal sınırlar içinde yerleşim yerinizdeki veya işlemin yapıldığı yerdeki tüketici hakem heyetine, bu sınırların üzerinde tüketici mahkemesine başvurabilirsiniz.`,
          ],
        },
      ],
    },
    {
      slug: "mesafeli-satis",
      title: "Mesafeli satış sözleşmesi",
      sections: [
        { heading: "1. Taraflar", body: [`Satıcı: ${seller(c)}`, "Alıcı: OctopusOn SEO hesabının sahibi; ad, adres ve iletişim bilgileri fatura bilgilerinde belirtilen kişi veya kuruluş."] },
        {
          heading: "2. Konu",
          body: [
            "Bu sözleşme, alıcının satıcıya ait my.octopuson.com üzerinden elektronik ortamda satın aldığı OctopusOn SEO kredisinin satışı ve ifasına ilişkin olarak 6502 sayılı Tüketicinin Korunması Hakkında Kanun ve Mesafeli Sözleşmeler Yönetmeliği uyarınca tarafların hak ve yükümlülüklerini düzenler.",
          ],
        },
        {
          heading: "3. Hizmet, fiyat ve ödeme",
          body: [
            "Kredi miktarı, KDV dahil fiyat, kur ve ödeme bilgileri sipariş özetinde ve ön bilgilendirme formunda belirtildiği gibidir; bu bilgiler sözleşmenin ayrılmaz parçasıdır.",
            "Ödeme lisanslı ödeme kuruluşu aracılığıyla alınır. Ödemenin herhangi bir nedenle onaylanmaması veya iptal edilmesi hâlinde kredi yüklenmez.",
          ],
        },
        {
          heading: "4. İfa",
          body: [
            "Satıcı, ödemenin onaylanmasının ardından krediyi derhâl alıcının hesabına yükler. Kredinin kullanımı kredi kullanım koşullarına tabidir.",
          ],
        },
        {
          heading: "5. Cayma hakkının bulunmaması",
          body: [
            "Alıcı, kredinin hesabına hemen yüklenmesini talep ettiğini ve Mesafeli Sözleşmeler Yönetmeliği md. 15/1-(ğ) uyarınca elektronik ortamda anında ifa edilen bu hizmette cayma hakkının bulunmadığını ödeme adımında onaylamıştır.",
            "Hizmetin satıcıdan kaynaklanan nedenlerle hiç sunulamaması hâlinde kullanılmamış kredi bedeli alıcıya iade edilir.",
          ],
        },
        {
          heading: "6. Uyuşmazlıklar",
          body: [
            "Uyuşmazlıklarda Ticaret Bakanlığınca ilan edilen parasal sınırlar dâhilinde tüketici hakem heyetleri, bu sınırların üzerinde tüketici mahkemeleri yetkilidir. Alıcının tüketici olmadığı (ticari) alımlarda [İstanbul] mahkemeleri ve icra daireleri yetkilidir.",
          ],
        },
        { heading: "7. Yürürlük", body: ["Alıcı, ödeme adımında bu sözleşmeyi elektronik ortamda onaylamakla sözleşmenin tüm hükümlerini kabul etmiş sayılır. Onay zamanı ve IP adresi sipariş kaydında saklanır."] },
      ],
    },
    {
      slug: "kvkk",
      title: "Kişisel verilerin işlenmesine ilişkin aydınlatma metni",
      sections: [
        { heading: "1. Veri sorumlusu", body: [`6698 sayılı Kişisel Verilerin Korunması Kanunu (KVKK) uyarınca veri sorumlusu: ${seller(c)}`] },
        {
          heading: "2. İşlenen veriler",
          body: [
            "Kimlik ve iletişim: ad soyad, e-posta, telefon. Fatura: unvan, T.C. kimlik veya vergi kimlik numarası, vergi dairesi, adres.",
            "Müşteri işlem: siparişler, ödeme kuruluşu işlem numaraları, kredi hareketleri, kredili isteklerin üst verisi (kaynak, uç nokta türü, durum, süre, tutar, program sürümü).",
            "İşlem güvenliği: oturum kayıtları, IP adresi, tarayıcı bilgisi, iki adımlı doğrulama kayıtları, cihaz jetonlarının özetleri.",
            "Kredili isteklerin içerikleri (anahtar kelimeler, arama sonuçları) saklanmaz; yalnızca ilgili veri sağlayıcısına iletilmek üzere ağ geçidinden geçer. Kart bilgileri yalnızca ödeme kuruluşu tarafından işlenir.",
          ],
        },
        {
          heading: "3. Amaçlar ve hukuki sebepler",
          body: [
            "Hesabın açılması, kredi satışı, kredili isteklerin iletilmesi ve ücretlendirilmesi: sözleşmenin kurulması ve ifası (KVKK md. 5/2-c).",
            "Fatura düzenlenmesi, vergi ve ticaret mevzuatı kapsamında kayıtların saklanması: hukuki yükümlülük (md. 5/2-ç).",
            "Dolandırıcılığın ve kötüye kullanımın önlenmesi, hizmet güvenliği, hataların giderilmesi: meşru menfaat (md. 5/2-f).",
          ],
        },
        {
          heading: "4. Aktarım",
          body: [
            "Ödeme kuruluşu ([iyzico / PayTR]) ve e-fatura hizmet sağlayıcısı ([Paraşüt]) ile, mevzuat gereği Gelir İdaresi Başkanlığı ve yetkili kamu kurumlarıyla paylaşılır.",
            "Sunucularımız [Contabo GmbH, Almanya] tarafından sağlanır. Yurt dışına aktarım KVKK md. 9 uyarınca standart sözleşme ile yapılır.",
            "Kredili modda yaptığınız isteklerin içerikleri, isteğin gönderildiği veri sağlayıcısına (ör. DataForSEO, SerpApi, Serper, Yandex) hizmetin ifası için iletilir; bu sağlayıcıların bir kısmı yurt dışındadır.",
          ],
        },
        {
          heading: "5. Saklama süreleri",
          body: [
            "Hesap verileri hesap silinene kadar; sipariş ve fatura kayıtları vergi ve ticaret mevzuatında öngörülen süreler (5–10 yıl) boyunca; ayrıntılı kullanım kayıtları 90 gün, sonrasında aylık toplamlar hâlinde saklanır.",
          ],
        },
        {
          heading: "6. Haklarınız",
          body: [
            `KVKK md. 11 kapsamındaki haklarınızı (bilgi talep etme, düzeltme, silme, itiraz vb.) ${c.email} adresine veya ${c.kep} KEP adresine yazılı olarak iletebilirsiniz. Başvurular en geç 30 gün içinde sonuçlandırılır.`,
          ],
        },
      ],
    },
    {
      slug: "cerezler",
      title: "Çerez politikası",
      sections: [
        {
          heading: "Kullanılan çerezler",
          body: [
            "my.octopuson.com yalnızca hizmetin çalışması için zorunlu çerezler kullanır: oturum çerezi (giriş yapmış olmanız), iki adımlı doğrulama sırasında kullanılan geçici çerez, \"bu tarayıcıya güven\" seçeneğinin çerezi ve dil tercihiniz.",
            "Analiz, reklam veya üçüncü taraf takip çerezi kullanılmaz. Zorunlu çerezler için onay gerekmez; tarayıcınızdan silebilirsiniz, ancak bu durumda yeniden giriş yapmanız gerekir.",
          ],
        },
      ],
    },
  ];
}
