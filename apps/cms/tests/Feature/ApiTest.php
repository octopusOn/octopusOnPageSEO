<?php

namespace Tests\Feature;

use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\Redirect;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

class ApiTest extends TestCase
{
    use RefreshDatabase;

    protected function setUp(): void
    {
        parent::setUp();
        config(['cms.preview_secret' => 'preview-secret', 'cms.revalidate_url' => null]);
        $this->languages();
        $this->page(PageType::Home, ['en' => 'Home', 'tr' => 'Ana sayfa']);
    }

    public function test_it_resolves_a_page_with_its_language_versions_and_breadcrumbs(): void
    {
        $page = $this->page(PageType::Page, ['en' => ['Features', 'features'], 'tr' => ['Özellikler', 'ozellikler']], attributes: ['product' => 'log-analyser']);
        $this->translation($page, 'tr')->update(['seo_description' => 'Tüm özellikler']);

        $this->getJson('/api/v1/pages/resolve?locale=tr&path=ozellikler')
            ->assertOk()
            ->assertJsonPath('data.title', 'Özellikler')
            ->assertJsonPath('data.url', '/tr/ozellikler')
            ->assertJsonPath('data.product', 'log-analyser')
            ->assertJsonPath('data.seo.description', 'Tüm özellikler')
            ->assertJsonPath('data.alternates', [['locale' => 'en', 'url' => '/en/features'], ['locale' => 'tr', 'url' => '/tr/ozellikler']])
            ->assertJsonPath('data.x_default', '/en/features')
            ->assertJsonPath('data.breadcrumbs', [['title' => 'Ana sayfa', 'url' => '/tr'], ['title' => 'Özellikler', 'url' => '/tr/ozellikler']]);
    }

    public function test_the_home_page_resolves_from_an_empty_path(): void
    {
        $this->getJson('/api/v1/pages/resolve?locale=en')->assertOk()->assertJsonPath('data.type', 'home')->assertJsonPath('data.url', '/en');
    }

    public function test_drafts_and_inactive_languages_are_hidden_but_previews_see_drafts(): void
    {
        $this->page(PageType::Page, ['en' => ['Secret', 'secret']], status: PublishStatus::Draft);

        $this->getJson('/api/v1/pages/resolve?locale=en&path=secret')->assertNotFound();
        $this->getJson('/api/v1/pages/resolve?locale=en&path=secret', ['Authorization' => 'Bearer wrong'])->assertNotFound();
        $this->getJson('/api/v1/pages/resolve?locale=en&path=secret', ['Authorization' => 'Bearer preview-secret'])
            ->assertOk()->assertJsonPath('data.status', 'draft');

        Language::query()->where('code', 'tr')->update(['is_active' => false]);
        $this->getJson('/api/v1/pages/resolve?locale=tr')->assertNotFound();
    }

    public function test_scheduled_translations_appear_when_their_time_comes(): void
    {
        $page = $this->page(PageType::Page, ['en' => ['Launch', 'launch']]);
        $this->translation($page, 'en')->update(['published_at' => now()->addDay()]);

        $this->getJson('/api/v1/pages/resolve?locale=en&path=launch')->assertNotFound();
        $this->travel(2)->days();
        $this->getJson('/api/v1/pages/resolve?locale=en&path=launch')->assertOk();
    }

    public function test_old_addresses_redirect_and_count_hits(): void
    {
        $page = $this->page(PageType::Page, ['tr' => ['Özellikler', 'ozellikler']]);
        Redirect::query()->create(['from_path' => '/tr/features', 'translation_id' => $this->translation($page, 'tr')->id, 'automatic' => true]);
        Redirect::query()->create(['from_path' => '/eski-blog/yazi', 'target' => 'https://example.com/yazi', 'status_code' => 302]);

        $this->getJson('/api/v1/pages/resolve?locale=tr&path=features')
            ->assertOk()->assertExactJson(['redirect' => ['to' => '/tr/ozellikler', 'status' => 301]]);
        // The website first sends unprefixed addresses into a language; manual ones still match.
        $this->getJson('/api/v1/pages/resolve?locale=tr&path=eski-blog/yazi')
            ->assertJsonPath('redirect.to', 'https://example.com/yazi')->assertJsonPath('redirect.status', 302);
        $this->assertSame(1, Redirect::query()->where('from_path', '/tr/features')->value('hits'));
    }

    public function test_categories_list_their_live_children_with_pages(): void
    {
        config(['cms.per_page' => 2]);
        $blog = $this->page(PageType::Category, ['en' => ['Blog', 'blog']]);
        foreach (['First', 'Second', 'Third'] as $i => $title) {
            $post = $this->page(PageType::Post, ['en' => $title], $blog);
            $this->translation($post, 'en')->update(['published_at' => now()->subDays(10 - $i)]);
        }
        $this->page(PageType::Post, ['en' => 'Draft'], $blog, status: PublishStatus::Draft);

        $this->getJson('/api/v1/pages/resolve?locale=en&path=blog')
            ->assertJsonPath('data.children.total', 3)
            ->assertJsonPath('data.children.last_page', 2)
            ->assertJsonPath('data.children.items.0.title', 'Third')
            ->assertJsonPath('data.children.items.1.url', '/en/blog/second');
        $this->getJson('/api/v1/pages/resolve?locale=en&path=blog/page/2')
            ->assertJsonPath('data.url', '/en/blog')
            ->assertJsonPath('data.children.page', 2)
            ->assertJsonPath('data.children.items.0.title', 'First');
        $this->getJson('/api/v1/pages/resolve?locale=en&path=blog/page/3')->assertNotFound();
    }

    public function test_blocks_get_their_media_and_product_cards(): void
    {
        $media = Media::query()->create(['disk' => 'public', 'path' => 'media/x.png', 'filename' => 'x.png', 'mime' => 'image/png', 'size' => 10, 'width' => 4, 'height' => 3, 'alt' => 'Ekran']);
        $this->page(PageType::Product, ['en' => ['OctopusOn SEO Crawler', 'seo-crawler']], attributes: ['product' => 'seo-crawler', 'icon' => 'file-search']);
        $home = $this->translation(Page::query()->where('type', 'home')->first(), 'en');
        $home->update(['blocks' => [
            ['id' => 'b1', 'type' => 'image', 'data' => ['image' => $media->id, 'caption' => 'Ekran', 'wide' => false]],
            ['id' => 'b2', 'type' => 'products', 'data' => ['eyebrow' => '', 'title' => 'Family', 'subtitle' => '']],
        ]]);

        $this->getJson('/api/v1/pages/resolve?locale=en')
            ->assertJsonPath('data.blocks.0.data.image.alt', 'Ekran')
            ->assertJsonPath('data.blocks.0.data.image.width', 4)
            ->assertJsonPath('data.blocks.1.products.0.name', 'SEO Crawler')
            ->assertJsonPath('data.blocks.1.products.0.url', '/en/seo-crawler');
    }

    public function test_site_returns_languages_and_menus(): void
    {
        $this->page(PageType::Page, ['tr' => ['Özellikler', 'ozellikler']], attributes: ['in_header' => true, 'footer_column' => 'product', 'product' => 'log-analyser', 'position' => 1]);
        $download = $this->page(PageType::Page, ['tr' => ['İndir', 'indir']], attributes: ['footer_column' => 'product', 'position' => 2]);
        $this->translation($download, 'tr')->update(['blocks' => [['id' => 'd1', 'type' => 'downloads', 'data' => []]]]);

        $this->getJson('/api/v1/site?locale=tr')
            ->assertJsonPath('data.languages.0.code', 'en')
            ->assertJsonPath('data.languages.0.is_default', true)
            ->assertJsonPath('data.navigation.header', [['label' => 'Özellikler', 'url' => '/tr/ozellikler', 'product' => 'log-analyser']])
            ->assertJsonPath('data.navigation.footer.product.0.label', 'Log Analyser')
            ->assertJsonPath('data.navigation.footer.product.1.label', 'İndir')
            ->assertJsonPath('data.navigation.download', '/tr/indir');
    }

    public function test_the_sitemap_lists_indexable_live_pages_with_alternates(): void
    {
        $this->page(PageType::Page, ['en' => ['Features', 'features'], 'tr' => ['Özellikler', 'ozellikler']]);
        $hidden = $this->page(PageType::Page, ['en' => ['Thanks', 'thanks']]);
        $this->translation($hidden, 'en')->update(['noindex' => true]);
        $this->page(PageType::Page, ['en' => ['Draft', 'draft']], status: PublishStatus::Draft);

        $urls = collect($this->getJson('/api/v1/sitemap')->assertOk()->json('data.urls'));

        $this->assertEqualsCanonicalizing(['/en', '/tr', '/en/features', '/tr/ozellikler'], $urls->pluck('url')->all());
        $this->assertSame(['en' => '/en/features', 'tr' => '/tr/ozellikler'], $urls->firstWhere('url', '/tr/ozellikler')['alternates']);
    }
}
