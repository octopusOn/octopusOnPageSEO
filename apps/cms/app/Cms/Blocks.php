<?php

namespace App\Cms;

use App\Models\Media;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Content blocks a translation is built from. Each type lists its fields; the admin's block
 * editor draws its form from these definitions, `clean()` keeps only defined fields with the
 * right types, and apps/web renders each type with its own component
 * (apps/web/src/components/blocks).
 *
 * Field types: text, textarea, markdown, code, url, select (options), toggle, number, strings (a
 * list of lines), items (a repeater with its own fields), media (an uploaded file's id), icon,
 * product.
 */
class Blocks
{
    public const MAX_BLOCKS = 60;

    public const MAX_ITEMS = 60;

    /** @return array<string, array{label: string, icon: string, hint: string, fields: list<array<string, mixed>>}> */
    public static function definitions(): array
    {
        $eyebrow = ['name' => 'eyebrow', 'type' => 'text', 'label' => 'Üst başlık'];
        $title = ['name' => 'title', 'type' => 'text', 'label' => 'Başlık'];
        $subtitle = ['name' => 'subtitle', 'type' => 'textarea', 'label' => 'Alt başlık'];
        $text = ['name' => 'text', 'type' => 'textarea', 'label' => 'Metin'];
        $states = ['yes' => '✓ Var', 'partial' => '◐ Kısmen / ücretli sürümde', 'no' => '✗ Yok', 'none' => 'İşaretsiz'];

        return [
            'hero' => [
                'label' => 'Giriş (hero)',
                'icon' => 'sparkles',
                'hint' => 'Ana sayfanın büyük girişi: rozet, başlık, indirme düğmesi ve görsel.',
                'fields' => [
                    ['name' => 'badge', 'type' => 'text', 'label' => 'Rozet'],
                    ['name' => 'badge_link', 'type' => 'url', 'label' => 'Rozet bağlantısı'],
                    ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true],
                    ['name' => 'highlight', 'type' => 'text', 'label' => 'Vurgulu devam', 'hint' => 'Başlığın parlak renkli ikinci kısmı.'],
                    $subtitle,
                    ['name' => 'product', 'type' => 'product', 'label' => 'İndirilecek program', 'hint' => 'Boşsa düğme aşağıdaki bağlantıya gider.'],
                    ['name' => 'download_label', 'type' => 'text', 'label' => 'İndirme düğmesi', 'hint' => 'İşletim sistemi bilinmediğinde gösterilir: “Ücretsiz indir”.'],
                    ['name' => 'primary_link', 'type' => 'url', 'label' => 'Düğme bağlantısı', 'hint' => 'Program seçilmediğinde ana düğmenin gittiği adres.'],
                    ['name' => 'secondary_label', 'type' => 'text', 'label' => 'İkinci düğme'],
                    ['name' => 'secondary_link', 'type' => 'url', 'label' => 'İkinci düğme bağlantısı'],
                    ['name' => 'note', 'type' => 'text', 'label' => 'Alt not'],
                    ['name' => 'visual', 'type' => 'select', 'label' => 'Görsel', 'options' => ['none' => 'Yok', 'programs' => 'Program ikonları', 'screenshot' => 'Program ekranı + canlı terminal (animasyonlu)', 'log-mockup' => 'Log Analyser ekranı', 'image' => 'Yüklenen görsel']],
                    ['name' => 'image', 'type' => 'media', 'label' => 'Görsel dosyası', 'hint' => 'Program ekranı görselinde pencere içinde gösterilir.'],
                    ['name' => 'code', 'type' => 'code', 'label' => 'Terminal komutları', 'hint' => 'Program ekranı görselinde terminal bunları satır satır yazar.'],
                ],
            ],
            'page_header' => [
                'label' => 'Sayfa başlığı',
                'icon' => 'file-text',
                'hint' => 'Sayfanın başlığı ve giriş cümlesi; isterseniz bölüm bağlantıları ve indirme düğmesi.',
                'fields' => [
                    ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'hint' => 'Boş bırakılırsa sayfa başlığı kullanılır.'],
                    $subtitle,
                    ['name' => 'anchors', 'type' => 'toggle', 'label' => 'Bölüm bağlantılarını göster', 'hint' => 'Özellik bölümlerine ve indirmelere giden çipler.'],
                    ['name' => 'download', 'type' => 'toggle', 'label' => 'Sayfanın programı için indirme düğmesi'],
                ],
            ],
            'formats' => [
                'label' => 'Etiket şeridi',
                'icon' => 'layers',
                'hint' => 'Tek satırlık başlık ve kısa etiketler (desteklenen formatlar gibi).',
                'fields' => [$title, ['name' => 'items', 'type' => 'strings', 'label' => 'Etiketler']],
            ],
            'feature_grid' => [
                'label' => 'Özellik kartları',
                'icon' => 'layers',
                'hint' => 'İkonlu kartlardan oluşan ızgara.',
                'fields' => [$eyebrow, $title, $subtitle, [
                    'name' => 'items', 'type' => 'items', 'label' => 'Kartlar', 'item_label' => 'Kart',
                    'fields' => [
                        ['name' => 'icon', 'type' => 'icon', 'label' => 'İkon'],
                        ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true],
                        $text,
                        ['name' => 'link', 'type' => 'url', 'label' => 'Bağlantı'],
                    ],
                ]],
            ],
            'split' => [
                'label' => 'Metin ve görsel',
                'icon' => 'bot-message-square',
                'hint' => 'Solda metin ve madde işaretleri, sağda görsel.',
                'fields' => [
                    $eyebrow, $title, $text,
                    ['name' => 'bullets', 'type' => 'strings', 'label' => 'Maddeler'],
                    ['name' => 'visual', 'type' => 'select', 'label' => 'Görsel', 'options' => ['none' => 'Yok', 'ai-bots' => 'Yapay zekâ botları grafiği', 'image' => 'Yüklenen görsel']],
                    ['name' => 'image', 'type' => 'media', 'label' => 'Görsel dosyası'],
                    ['name' => 'tinted', 'type' => 'toggle', 'label' => 'Renkli zemin'],
                ],
            ],
            'steps' => [
                'label' => 'Adımlar',
                'icon' => 'rocket',
                'hint' => 'Numaralı adım kartları.',
                'fields' => [$title, [
                    'name' => 'items', 'type' => 'items', 'label' => 'Adımlar', 'item_label' => 'Adım',
                    'fields' => [['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true], $text],
                ]],
            ],
            'stats' => [
                'label' => 'Rakamlar',
                'icon' => 'chart-column-big',
                'hint' => 'Büyük rakamlar ve açıklamaları.',
                'fields' => [[
                    'name' => 'items', 'type' => 'items', 'label' => 'Rakamlar', 'item_label' => 'Rakam',
                    'fields' => [
                        ['name' => 'value', 'type' => 'text', 'label' => 'Değer', 'required' => true, 'max' => 20],
                        ['name' => 'label', 'type' => 'text', 'label' => 'Açıklama'],
                    ],
                ]],
            ],
            'terminal' => [
                'label' => 'Komut satırı',
                'icon' => 'terminal',
                'hint' => 'Koyu bölüm: metin ve terminal. Kod boşsa Log Analyser animasyonu görünür.',
                'fields' => [
                    $eyebrow, $title, $text,
                    ['name' => 'points', 'type' => 'strings', 'label' => 'Kısa notlar'],
                    ['name' => 'code', 'type' => 'code', 'label' => 'Komutlar'],
                ],
            ],
            'products' => [
                'label' => 'Program kartları',
                'icon' => 'layers',
                'hint' => 'Programı seçili tüm sayfaların kartları, her biri kendi renginde; metinleri o sayfaların özetidir. Yeni program sayfası eklendikçe kendiliğinden çoğalır.',
                'fields' => [$eyebrow, $title, $subtitle],
            ],
            'faq' => [
                'label' => 'Sık sorulan sorular',
                'icon' => 'circle-check',
                'hint' => 'Açılır sorular; sayfaya FAQPage yapılandırılmış verisi eklenir.',
                'fields' => [$title, [
                    'name' => 'items', 'type' => 'items', 'label' => 'Sorular', 'item_label' => 'Soru',
                    'fields' => [
                        ['name' => 'question', 'type' => 'text', 'label' => 'Soru', 'required' => true, 'max' => 500],
                        ['name' => 'answer', 'type' => 'textarea', 'label' => 'Cevap', 'required' => true],
                    ],
                ]],
            ],
            'cta' => [
                'label' => 'Çağrı kutusu',
                'icon' => 'download',
                'hint' => 'Sayfa sonundaki büyük kutu ve düğme.',
                'fields' => [
                    $title, $text,
                    ['name' => 'button_label', 'type' => 'text', 'label' => 'Düğme'],
                    ['name' => 'action', 'type' => 'select', 'label' => 'Düğme ne yapsın', 'options' => ['download' => 'İşletim sistemine göre indir', 'link' => 'Bağlantıya git']],
                    ['name' => 'product', 'type' => 'product', 'label' => 'İndirilecek program'],
                    ['name' => 'link', 'type' => 'url', 'label' => 'Bağlantı'],
                ],
            ],
            'sections' => [
                'label' => 'Özellik bölümleri',
                'icon' => 'circle-check',
                'hint' => 'Sırayla sağa sola dizilen bölümler: başlık, metin ve madde listesi.',
                'fields' => [[
                    'name' => 'items', 'type' => 'items', 'label' => 'Bölümler', 'item_label' => 'Bölüm',
                    'fields' => [
                        ['name' => 'anchor', 'type' => 'text', 'label' => 'Bağlantı adı', 'hint' => 'Adresin # kısmı: bots → /tr/ozellikler#bots', 'max' => 60],
                        ['name' => 'icon', 'type' => 'icon', 'label' => 'İkon'],
                        ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true],
                        $text,
                        ['name' => 'bullets', 'type' => 'strings', 'label' => 'Maddeler'],
                    ],
                ]],
            ],
            'api_options' => [
                'label' => 'Kullanım seçenekleri (API / kredi)',
                'icon' => 'zap',
                'hint' => 'Ücretsiz kullanım, kendi API anahtarlarınız ve OctopusOn kredileri kartları; son kart hesap sitesine gider.',
                'fields' => [$eyebrow, $title, $subtitle, [
                    'name' => 'items', 'type' => 'items', 'label' => 'Seçenekler', 'item_label' => 'Seçenek',
                    'fields' => [
                        ['name' => 'icon', 'type' => 'icon', 'label' => 'İkon'],
                        ['name' => 'badge', 'type' => 'text', 'label' => 'Rozet', 'hint' => 'Kartın köşesinde: “%100 ücretsiz”, “İsteğe bağlı”.', 'max' => 40],
                        ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true],
                        $text,
                        ['name' => 'bullets', 'type' => 'strings', 'label' => 'Maddeler'],
                        ['name' => 'button_label', 'type' => 'text', 'label' => 'Düğme'],
                        ['name' => 'link', 'type' => 'url', 'label' => 'Bağlantı', 'hint' => 'Boşsa hesap sitesi (CMS_ACCOUNT_URL).'],
                        ['name' => 'highlight', 'type' => 'toggle', 'label' => 'Öne çıkar'],
                    ],
                ]],
            ],
            'comparison' => [
                'label' => 'Karşılaştırma tablosu',
                'icon' => 'git-compare',
                'hint' => 'Programı başka bir araçla satır satır karşılaştırır. Rakip bilgilerinin kaynağını ve tarihini dipnotta verin.',
                'fields' => [
                    $eyebrow, $title, $subtitle,
                    ['name' => 'feature_label', 'type' => 'text', 'label' => 'Özellik sütunu başlığı', 'hint' => 'Boşsa “Özellik”.'],
                    ['name' => 'ours', 'type' => 'text', 'label' => 'Bizim sütun', 'required' => true],
                    ['name' => 'theirs', 'type' => 'text', 'label' => 'Karşılaştırılan sütun', 'required' => true],
                    [
                        'name' => 'rows', 'type' => 'items', 'label' => 'Satırlar', 'item_label' => 'Satır',
                        'fields' => [
                            ['name' => 'feature', 'type' => 'text', 'label' => 'Özellik', 'required' => true],
                            ['name' => 'ours_state', 'type' => 'select', 'label' => 'Bizde', 'options' => $states],
                            ['name' => 'ours', 'type' => 'text', 'label' => 'Bizim açıklama'],
                            ['name' => 'theirs_state', 'type' => 'select', 'label' => 'Karşılaştırılanda', 'options' => $states],
                            ['name' => 'theirs', 'type' => 'text', 'label' => 'Karşılaştırılanın açıklaması'],
                        ],
                    ],
                    ['name' => 'note', 'type' => 'textarea', 'label' => 'Dipnot', 'hint' => 'Kaynak ve tarih: “Ekim 2026, screamingfrog.co.uk fiyat sayfası”.'],
                ],
            ],
            'credit_calculator' => [
                'label' => 'Kredi hesaplayıcı',
                'icon' => 'chart-column-big',
                'hint' => 'Ziyaretçi miktarları girer; kredi ve dolar tutarı hesaplanır. Fiyatlar buradan güncellenir.',
                'fields' => [
                    $eyebrow, $title, $text,
                    ['name' => 'credit_usd', 'type' => 'number', 'label' => '1 kredi (USD)', 'hint' => 'Hesap sitesindeki kredi birimi: 0.001.'],
                    [
                        'name' => 'items', 'type' => 'items', 'label' => 'İşlemler', 'item_label' => 'İşlem',
                        'fields' => [
                            ['name' => 'label', 'type' => 'text', 'label' => 'İşlem', 'required' => true],
                            ['name' => 'unit', 'type' => 'text', 'label' => 'Birim', 'hint' => 'örn. sorgu, 1.000 kelime'],
                            ['name' => 'credits', 'type' => 'number', 'label' => 'Birim başına kredi', 'required' => true],
                            ['name' => 'quantity', 'type' => 'number', 'label' => 'Varsayılan miktar'],
                            ['name' => 'max', 'type' => 'number', 'label' => 'En fazla miktar'],
                        ],
                    ],
                    ['name' => 'note', 'type' => 'textarea', 'label' => 'Not'],
                    ['name' => 'button_label', 'type' => 'text', 'label' => 'Düğme'],
                    ['name' => 'link', 'type' => 'url', 'label' => 'Bağlantı', 'hint' => 'Boşsa hesap sitesi (CMS_ACCOUNT_URL).'],
                ],
            ],
            'note' => [
                'label' => 'Bilgi notu',
                'icon' => 'shield-check',
                'hint' => 'Çerçeveli açıklama kutusu.',
                'fields' => [$title, $text],
            ],
            'code' => [
                'label' => 'Kod',
                'icon' => 'code',
                'hint' => 'Başlık, açıklama ve kopyalanabilir kod.',
                'fields' => [
                    $title, $text,
                    ['name' => 'code', 'type' => 'code', 'label' => 'Kod', 'required' => true],
                    ['name' => 'copy', 'type' => 'toggle', 'label' => 'Kopyala düğmesi'],
                ],
            ],
            'rich_text' => [
                'label' => 'Metin (Markdown)',
                'icon' => 'file-text',
                'hint' => 'Başlıklar (##), listeler, **kalın**, `kod`, [bağlantı](/tr/...) ve ![görsel](adres).',
                'fields' => [['name' => 'body', 'type' => 'markdown', 'label' => 'Metin', 'required' => true]],
            ],
            'image' => [
                'label' => 'Görsel',
                'icon' => 'folder',
                'hint' => 'Tek görsel ve açıklaması.',
                'fields' => [
                    ['name' => 'image', 'type' => 'media', 'label' => 'Görsel', 'required' => true],
                    ['name' => 'caption', 'type' => 'text', 'label' => 'Açıklama'],
                    ['name' => 'wide', 'type' => 'toggle', 'label' => 'Geniş'],
                ],
            ],
            'query_examples' => [
                'label' => 'Sorgu örnekleri',
                'icon' => 'search',
                'hint' => 'Başlık, açıklama ve sorgu + açıklama kartları.',
                'fields' => [
                    $title, $text,
                    ['name' => 'examples_title', 'type' => 'text', 'label' => 'Örnekler başlığı'],
                    [
                        'name' => 'items', 'type' => 'items', 'label' => 'Örnekler', 'item_label' => 'Örnek',
                        'fields' => [
                            ['name' => 'query', 'type' => 'text', 'label' => 'Sorgu', 'required' => true, 'max' => 500],
                            ['name' => 'text', 'type' => 'text', 'label' => 'Açıklama'],
                        ],
                    ],
                ],
            ],
            'token_lists' => [
                'label' => 'Kod listeleri',
                'icon' => 'database',
                'hint' => 'Yan yana kartlarda alan ve metrik adları.',
                'fields' => [[
                    'name' => 'items', 'type' => 'items', 'label' => 'Kartlar', 'item_label' => 'Kart',
                    'fields' => [
                        ['name' => 'title', 'type' => 'text', 'label' => 'Başlık', 'required' => true],
                        ['name' => 'tokens', 'type' => 'strings', 'label' => 'Adlar'],
                    ],
                ]],
            ],
            'links' => [
                'label' => 'Bağlantılar',
                'icon' => 'link',
                'hint' => 'Ok işaretli bağlantı satırı.',
                'fields' => [[
                    'name' => 'items', 'type' => 'items', 'label' => 'Bağlantılar', 'item_label' => 'Bağlantı',
                    'fields' => [
                        ['name' => 'label', 'type' => 'text', 'label' => 'Metin', 'required' => true],
                        ['name' => 'url', 'type' => 'url', 'label' => 'Adres', 'required' => true],
                    ],
                ]],
            ],
            'downloads' => [
                'label' => 'Tüm indirmeler',
                'icon' => 'download',
                'hint' => 'Her programın son sürümü ve dosyaları (GitHub sürümlerinden), sistem gereksinimleri ve kaynaktan derleme.',
                'fields' => [
                    ['name' => 'requirements_title', 'type' => 'text', 'label' => 'Gereksinimler başlığı'],
                    ['name' => 'requirements', 'type' => 'strings', 'label' => 'Gereksinimler'],
                    ['name' => 'source_title', 'type' => 'text', 'label' => 'Kaynaktan derleme başlığı'],
                    ['name' => 'source_text', 'type' => 'textarea', 'label' => 'Kaynaktan derleme metni'],
                    ['name' => 'source_code', 'type' => 'code', 'label' => 'Derleme komutları'],
                ],
            ],
            'product_downloads' => [
                'label' => 'Program indirmeleri',
                'icon' => 'download',
                'hint' => 'Sayfanın programının tüm dosyaları (platformlar ve komut satırı).',
                'fields' => [
                    ['name' => 'link_label', 'type' => 'text', 'label' => 'Tüm indirmeler bağlantısı'],
                    ['name' => 'link', 'type' => 'url', 'label' => 'Tüm indirmeler adresi'],
                ],
            ],
            'changelog' => [
                'label' => 'Sürüm notları',
                'icon' => 'newspaper',
                'hint' => 'GitHub sürümlerinin notları, en yeni üstte.',
                'fields' => [],
            ],
            'children' => [
                'label' => 'Alt sayfalar',
                'icon' => 'folder',
                'hint' => 'Bu sayfanın yayındaki alt sayfaları. Kategoriler bu blok olmadan da listeyi sayfa sonunda gösterir.',
                'fields' => [$title, ['name' => 'style', 'type' => 'select', 'label' => 'Görünüm', 'options' => ['cards' => 'Kartlar', 'list' => 'Liste']]],
            ],
        ];
    }

    /**
     * Normalises posted blocks: unknown types and fields are dropped, values are cast and
     * trimmed, every block keeps a stable id. Throws a ValidationException naming the block
     * and field of every problem.
     *
     * @param  mixed  $blocks  decoded JSON from the block editor
     * @return list<array{id: string, type: string, data: array<string, mixed>}>
     */
    public static function clean(mixed $blocks): array
    {
        if (! is_array($blocks)) {
            throw ValidationException::withMessages(['blocks' => 'Bloklar okunamadı; sayfayı yenileyip tekrar deneyin.']);
        }
        if (count($blocks) > self::MAX_BLOCKS) {
            throw ValidationException::withMessages(['blocks' => 'Bir sayfada en fazla '.self::MAX_BLOCKS.' blok olabilir.']);
        }
        $definitions = self::definitions();
        $errors = [];
        $clean = [];
        foreach (array_values($blocks) as $index => $block) {
            $type = is_array($block) ? ($block['type'] ?? null) : null;
            if (! is_string($type) || ! isset($definitions[$type])) {
                $errors["blocks.$index"] = 'Blok '.($index + 1).': bilinmeyen blok türü.';

                continue;
            }
            $definition = $definitions[$type];
            $prefix = 'Blok '.($index + 1).' ('.$definition['label'].')';
            $id = is_string($block['id'] ?? null) && preg_match('/^[a-z0-9_-]{4,40}$/i', $block['id']) ? $block['id'] : 'b'.Str::lower(Str::random(10));
            $data = is_array($block['data'] ?? null) ? $block['data'] : [];
            $clean[] = [
                'id' => $id,
                'type' => $type,
                'data' => self::cleanFields($definition['fields'], $data, $prefix, "blocks.$index", $errors),
            ];
        }
        if ($errors) {
            throw ValidationException::withMessages($errors);
        }

        return $clean;
    }

    /** Empty data of a block type, for a newly added block. */
    public static function blank(string $type): array
    {
        $data = [];
        foreach (self::definitions()[$type]['fields'] ?? [] as $field) {
            $data[$field['name']] = self::emptyValue($field);
        }

        return $data;
    }

    /** Ids of every media file the blocks use. */
    public static function mediaIds(array $blocks): array
    {
        $definitions = self::definitions();
        $ids = [];
        foreach ($blocks as $block) {
            $fields = $definitions[$block['type'] ?? '']['fields'] ?? [];
            self::collectMedia($fields, $block['data'] ?? [], $ids);
        }

        return array_values(array_unique($ids));
    }

    private static function collectMedia(array $fields, array $data, array &$ids): void
    {
        foreach ($fields as $field) {
            $value = $data[$field['name']] ?? null;
            if ($field['type'] === 'media' && is_int($value)) {
                $ids[] = $value;
            } elseif ($field['type'] === 'items' && is_array($value)) {
                foreach ($value as $item) {
                    self::collectMedia($field['fields'], is_array($item) ? $item : [], $ids);
                }
            }
        }
    }

    private static function emptyValue(array $field): mixed
    {
        return match ($field['type']) {
            'toggle' => false,
            'strings', 'items' => [],
            'media', 'number' => null,
            'select' => array_key_first($field['options']),
            default => '',
        };
    }

    private static function cleanFields(array $fields, array $data, string $prefix, string $key, array &$errors): array
    {
        $out = [];
        foreach ($fields as $field) {
            $name = $field['name'];
            $label = "$prefix · {$field['label']}";
            $out[$name] = self::cleanValue($field, $data[$name] ?? null, $label, "$key.$name", $errors);
            $empty = $out[$name] === '' || $out[$name] === null || $out[$name] === [];
            if (($field['required'] ?? false) && $empty) {
                $errors["$key.$name"] = "$label: boş bırakılamaz.";
            }
        }

        return $out;
    }

    private static function cleanValue(array $field, mixed $value, string $label, string $key, array &$errors): mixed
    {
        switch ($field['type']) {
            case 'text':
            case 'textarea':
            case 'markdown':
            case 'code':
                $value = is_scalar($value) ? (string) $value : '';
                $value = $field['type'] === 'code' ? rtrim(str_replace("\r\n", "\n", $value)) : trim(str_replace("\r\n", "\n", $value));
                $max = $field['max'] ?? match ($field['type']) {
                    'text' => 255,
                    'textarea' => 3000,
                    default => 60000,
                };
                if (mb_strlen($value) > $max) {
                    $errors[$key] = "$label: en fazla $max karakter olabilir.";
                }

                return $value;
            case 'url':
                $value = is_scalar($value) ? trim((string) $value) : '';
                if ($value !== '' && ! preg_match('~^(/|#|https?://|mailto:)~i', $value)) {
                    $errors[$key] = "$label: / ile başlayan bir site adresi ya da https:// ile başlayan tam adres girin.";
                }
                if (mb_strlen($value) > 500) {
                    $errors[$key] = "$label: en fazla 500 karakter olabilir.";
                }

                return $value;
            case 'select':
                $value = is_scalar($value) ? (string) $value : '';

                return array_key_exists($value, $field['options']) ? $value : array_key_first($field['options']);
            case 'icon':
                $value = is_scalar($value) ? (string) $value : '';

                return in_array($value, config('cms.icons'), true) ? $value : '';
            case 'product':
                $value = is_scalar($value) ? (string) $value : '';

                return array_key_exists($value, config('cms.products')) ? $value : '';
            case 'toggle':
                return filter_var($value, FILTER_VALIDATE_BOOL);
            case 'number':
                if ($value === null || $value === '') {
                    return null;
                }
                $number = filter_var(str_replace(',', '.', (string) $value), FILTER_VALIDATE_FLOAT);
                if ($number === false || $number < 0 || $number > 1_000_000_000) {
                    $errors[$key] = "$label: 0 ya da daha büyük bir sayı girin.";

                    return null;
                }

                return $number;
            case 'strings':
                $lines = is_array($value) ? $value : preg_split('/\R/', is_scalar($value) ? (string) $value : '');
                $lines = array_values(array_filter(array_map(fn ($line) => is_scalar($line) ? trim((string) $line) : '', $lines), fn ($line) => $line !== ''));
                if (count($lines) > self::MAX_ITEMS) {
                    $errors[$key] = "$label: en fazla ".self::MAX_ITEMS.' satır olabilir.';
                }
                foreach ($lines as $line) {
                    if (mb_strlen($line) > 1000) {
                        $errors[$key] = "$label: bir satır en fazla 1000 karakter olabilir.";
                    }
                }

                return $lines;
            case 'media':
                $id = filter_var($value, FILTER_VALIDATE_INT);
                if ($id === false || $id === null) {
                    return null;
                }
                if (! Media::query()->whereKey($id)->exists()) {
                    $errors[$key] = "$label: seçilen dosya artık yok.";

                    return null;
                }

                return $id;
            case 'items':
                $items = is_array($value) ? array_values($value) : [];
                if (count($items) > self::MAX_ITEMS) {
                    $errors[$key] = "$label: en fazla ".self::MAX_ITEMS.' öğe olabilir.';
                }
                $out = [];
                foreach ($items as $i => $item) {
                    $out[] = self::cleanFields($field['fields'], is_array($item) ? $item : [], "$label ".($i + 1), "$key.$i", $errors);
                }

                return $out;
        }

        return null;
    }
}
