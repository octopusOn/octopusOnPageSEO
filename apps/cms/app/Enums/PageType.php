<?php

namespace App\Enums;

/** What a page is. Every kind lives in the same `pages` table and tree. */
enum PageType: string
{
    case Home = 'home';
    case Page = 'page';
    case Category = 'category';
    case Product = 'product';
    case Post = 'post';
    case Doc = 'doc';
    case Legal = 'legal';

    public function label(): string
    {
        return match ($this) {
            self::Home => 'Ana sayfa',
            self::Page => 'Sayfa',
            self::Category => 'Kategori',
            self::Product => 'Ürün',
            self::Post => 'Yazı',
            self::Doc => 'Doküman',
            self::Legal => 'Yasal metin',
        };
    }

    public function description(): string
    {
        return match ($this) {
            self::Home => 'Dilin kök adresi (/tr). Yalnızca bir tane olur.',
            self::Page => 'Bloklarla kurulan serbest sayfa.',
            self::Category => 'Alt sayfalarını otomatik listeler: blog, dokümanlar, ürün grupları.',
            self::Product => 'Bir OctopusOn SEO programının sayfası; indirme ve ürün kartlarında kullanılır.',
            self::Post => 'Blog yazısı; kategorisinde yayın tarihine göre listelenir.',
            self::Doc => 'Doküman sayfası; kategorisinde sırasına göre listelenir.',
            self::Legal => 'Hesap sitesinin (my.octopuson.com) yasal metni; web sitesinde yayınlanmaz. {{company.title}} gibi alanları hesap sitesi kendi ayarlarından doldurur.',
        };
    }

    public function badge(): string
    {
        return match ($this) {
            self::Home => 'pop',
            self::Category => 'accent',
            self::Product => 'product',
            self::Post => 'info',
            self::Doc => 'good',
            self::Legal => 'warning',
            self::Page => '',
        };
    }

    /** @return array<string, string> */
    public static function options(): array
    {
        return collect(self::cases())->mapWithKeys(fn (self $type) => [$type->value => $type->label()])->all();
    }
}
