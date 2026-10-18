<?php

namespace Tests\Feature;

use App\Enums\PageType;
use App\Models\Page;
use App\Models\PageTranslation;
use Database\Seeders\LegalSeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Tests\TestCase;

/** The account site's legal texts: seeded, served by key or slug, kept off the website. */
class LegalTest extends TestCase
{
    use RefreshDatabase;

    protected function setUp(): void
    {
        parent::setUp();
        config(['cms.revalidate_url' => null]);
        $this->languages();
        $this->page(PageType::Home, ['en' => 'Home', 'tr' => 'Ana sayfa']);
    }

    public function test_the_seeder_adds_each_text_once_in_the_sites_languages(): void
    {
        $this->seed(LegalSeeder::class);
        $privacy = Page::query()->where('key', 'privacy')->firstOrFail();
        $this->translation($privacy, 'tr')->update(['title' => 'Düzenlendi']);
        $this->seed(LegalSeeder::class);

        $this->assertSame(['credit-terms', 'pre-contract-information', 'distance-sales', 'privacy', 'cookies'], Page::query()->where('type', PageType::Legal)->orderBy('position')->pluck('key')->all());
        // The test site has English and Turkish; the German and Spanish files wait for those languages.
        $this->assertSame(10, PageTranslation::query()->whereHas('page', fn ($q) => $q->where('type', PageType::Legal))->count());
        $this->assertSame('Düzenlendi', $this->translation($privacy, 'tr')->title);
    }

    public function test_the_api_lists_the_texts_and_finds_one_by_slug_key_or_another_languages_slug(): void
    {
        $this->seed(LegalSeeder::class);

        $this->getJson('/api/v1/legal?locale=tr')
            ->assertOk()
            ->assertJsonCount(5, 'data')
            ->assertJsonPath('data.0.key', 'credit-terms')
            ->assertJsonPath('data.0.slug', 'kredi-kosullari')
            ->assertJsonPath('data.0.title', 'OctopusOn SEO kredi kullanım koşulları')
            ->assertJsonPath('data.3.label', 'KVKK');
        $document = $this->getJson('/api/v1/legal/kvkk?locale=tr')
            ->assertOk()
            ->assertJsonPath('data.key', 'privacy')
            ->assertJsonPath('data.locale', 'tr')
            ->assertJsonPath('data.title', 'Kişisel verilerin işlenmesine ilişkin aydınlatma metni')
            ->assertJsonPath('data.blocks.0.type', 'rich_text');
        $this->assertEqualsCanonicalizing([['locale' => 'en', 'slug' => 'privacy-notice'], ['locale' => 'tr', 'slug' => 'kvkk']], $document->json('data.alternates'));
        $this->assertStringContainsString('- Unvan: {{company.title}}', $document->json('data.blocks.0.data.body'));
        $this->assertStringStartsWith('## 1. Veri sorumlusu', $document->json('data.blocks.0.data.body'));

        $this->getJson('/api/v1/legal/privacy?locale=en')->assertOk()->assertJsonPath('data.slug', 'privacy-notice');
        $this->getJson('/api/v1/legal/privacy-notice?locale=tr')->assertOk()->assertJsonPath('data.slug', 'kvkk');
        $this->getJson('/api/v1/legal/nothing?locale=tr')->assertNotFound();
        $this->getJson('/api/v1/legal')->assertUnprocessable();
    }

    public function test_a_text_missing_in_a_language_comes_in_the_default_language(): void
    {
        $this->page(PageType::Legal, ['en' => ['Refunds', 'refunds']], attributes: ['key' => 'refunds']);

        $this->getJson('/api/v1/legal/refunds?locale=tr')->assertOk()->assertJsonPath('data.locale', 'en')->assertJsonPath('data.slug', 'refunds');
        $this->getJson('/api/v1/legal?locale=tr')->assertJsonPath('data.0.locale', 'en');
    }

    public function test_the_website_never_sees_legal_texts(): void
    {
        $this->seed(LegalSeeder::class);
        $privacy = $this->translation(Page::query()->where('key', 'privacy')->firstOrFail(), 'tr');
        // A renamed text leaves a redirect behind, which the website must not follow either.
        $this->actingAs($this->admin());
        $this->put("/admin/pages/{$privacy->page_id}/tr", ['title' => $privacy->title, 'slug' => 'kvkk-metni', 'status' => 'published', 'blocks' => '[]']);

        $this->getJson('/api/v1/pages/resolve?locale=tr&path=kvkk')->assertNotFound();
        $this->getJson('/api/v1/pages/resolve?locale=en&path=privacy-notice')->assertNotFound();
        $this->assertEqualsCanonicalizing(['/en', '/tr'], collect($this->getJson('/api/v1/sitemap')->json('data.urls'))->pluck('url')->all());
    }

    public function test_admins_create_legal_pages_with_a_key_at_the_top_level(): void
    {
        $this->actingAs($this->admin());
        $parent = $this->page(PageType::Page, ['en' => ['About', 'about']]);

        $this->post('/admin/pages', ['type' => 'legal', 'key' => 'Bad Key', 'locale' => 'en', 'title' => 'Refunds'])->assertSessionHasErrors('key');
        $this->post('/admin/pages', ['type' => 'legal', 'key' => 'refunds', 'parent_id' => $parent->id, 'locale' => 'en', 'title' => 'Refunds'])->assertSessionHasErrors('parent_id');
        $this->post('/admin/pages', ['type' => 'legal', 'key' => 'refunds', 'locale' => 'en', 'title' => 'Refunds'])->assertSessionHasNoErrors();
        $this->post('/admin/pages', ['type' => 'legal', 'key' => 'refunds', 'locale' => 'en', 'title' => 'Refunds again'])->assertSessionHasErrors('key');

        $page = Page::query()->where('key', 'refunds')->firstOrFail();
        $this->assertSame(PageType::Legal, $page->type);
        $this->assertSame('rich_text', $this->translation($page, 'en')->blocks[0]['type']);
    }
}
