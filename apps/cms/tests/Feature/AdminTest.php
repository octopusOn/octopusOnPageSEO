<?php

namespace Tests\Feature;

use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Database\Seeders\ContentSeeder;
use Database\Seeders\ProgramSeeder;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Http\Client\Request;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Storage;
use Tests\TestCase;

class AdminTest extends TestCase
{
    use RefreshDatabase;

    protected function setUp(): void
    {
        parent::setUp();
        config(['cms.revalidate_url' => null]);
        $this->languages();
    }

    public function test_guests_are_sent_to_the_login_page(): void
    {
        $this->get('/admin/pages')->assertRedirect('/login');
        $this->get('/login')->assertOk()->assertSee('Giriş yap');
    }

    public function test_admins_sign_in_with_their_password(): void
    {
        $this->admin();

        $this->post('/login', ['email' => 'ayse@octopuson.test', 'password' => 'wrong-password'])->assertSessionHasErrors('email');
        $this->assertGuest();
        $this->post('/login', ['email' => 'ayse@octopuson.test', 'password' => 'correct-horse-battery'])->assertRedirect('/admin');
        $this->assertAuthenticated();
    }

    public function test_every_admin_screen_renders(): void
    {
        $this->actingAs($this->admin());
        $page = $this->page(PageType::Category, ['en' => 'Blog']);

        foreach (['/admin', '/admin/pages', '/admin/pages/create', "/admin/pages/{$page->id}/edit", "/admin/pages/{$page->id}/en", "/admin/pages/{$page->id}/tr", '/admin/media', '/admin/languages', '/admin/languages/create', '/admin/redirects', '/admin/users'] as $url) {
            $this->get($url)->assertOk();
        }
    }

    public function test_a_new_page_starts_as_a_draft_in_its_first_language(): void
    {
        $this->actingAs($this->admin());
        $blog = $this->page(PageType::Category, ['tr' => ['Blog', 'blog']]);

        $response = $this->post('/admin/pages', [
            'type' => 'post', 'parent_id' => $blog->id, 'locale' => 'tr', 'title' => 'Log analizi rehberi', 'slug' => '',
        ]);

        $page = Page::query()->latest('id')->first();
        $response->assertRedirect("/admin/pages/{$page->id}/tr");
        $translation = $this->translation($page, 'tr');
        $this->assertSame('blog/log-analizi-rehberi', $translation->path);
        $this->assertSame(PublishStatus::Draft, $translation->status);
        $this->assertSame('rich_text', $translation->blocks[0]['type']);
    }

    public function test_only_one_home_page_exists(): void
    {
        $this->actingAs($this->admin());
        $this->page(PageType::Home, ['en' => 'Home']);

        $this->post('/admin/pages', ['type' => 'home', 'locale' => 'en', 'title' => 'Second home'])->assertSessionHasErrors('type');
    }

    public function test_saving_a_translation_cleans_blocks_and_stamps_the_publication(): void
    {
        $this->actingAs($this->admin());
        $page = $this->page(PageType::Page, ['en' => ['Features', 'features']], status: PublishStatus::Draft);

        $this->put("/admin/pages/{$page->id}/tr", [
            'title' => 'Özellikler',
            'slug' => 'Özellikler',
            'status' => 'published',
            'blocks' => json_encode([
                ['id' => 'abc12345', 'type' => 'faq', 'data' => ['title' => ' SSS ', 'items' => [['question' => 'Ücretsiz mi?', 'answer' => 'Evet.', 'extra' => 'x']], 'junk' => 1]],
            ]),
        ])->assertRedirect("/admin/pages/{$page->id}/tr")->assertSessionHas('status');

        $translation = $this->translation($page, 'tr');
        $this->assertSame('ozellikler', $translation->slug);
        $this->assertTrue($translation->isLive());
        $this->assertNotNull($translation->published_at);
        $this->assertSame(['id' => 'abc12345', 'type' => 'faq', 'data' => ['title' => 'SSS', 'items' => [['question' => 'Ücretsiz mi?', 'answer' => 'Evet.']]]], $translation->blocks[0]);
    }

    public function test_invalid_blocks_are_refused_with_the_block_named(): void
    {
        $this->actingAs($this->admin());
        $page = $this->page(PageType::Page, ['en' => ['Features', 'features']]);

        $this->put("/admin/pages/{$page->id}/en", [
            'title' => 'Features', 'status' => 'published',
            'blocks' => json_encode([['type' => 'faq', 'data' => ['items' => [['question' => '', 'answer' => 'x']]]], ['type' => 'nope']]),
        ])->assertSessionHasErrors(['blocks.0.items.0.question', 'blocks.1']);
    }

    public function test_a_changed_slug_keeps_the_old_address_working(): void
    {
        $this->actingAs($this->admin());
        $page = $this->page(PageType::Page, ['en' => ['Pricing', 'pricing']]);

        $this->put("/admin/pages/{$page->id}/en", ['title' => 'Plans', 'slug' => 'plans', 'status' => 'published', 'blocks' => '[]']);

        $this->assertSame('/en/plans', Redirect::query()->where('from_path', '/en/pricing')->first()->destination());
    }

    public function test_a_page_moves_under_another_and_cannot_move_below_itself(): void
    {
        $this->actingAs($this->admin());
        $docs = $this->page(PageType::Category, ['en' => ['Docs', 'docs']]);
        $page = $this->page(PageType::Doc, ['en' => ['Query language', 'query-language']]);

        $this->put("/admin/pages/{$page->id}", ['type' => 'doc', 'parent_id' => $docs->id])->assertSessionHasNoErrors();
        $this->assertSame('/en/docs/query-language', $this->translation($page, 'en')->url());

        $this->put("/admin/pages/{$docs->id}", ['type' => 'category', 'parent_id' => $page->id])->assertSessionHasErrors('parent_id');
    }

    public function test_a_page_cannot_move_under_a_parent_missing_one_of_its_languages(): void
    {
        $this->actingAs($this->admin());
        $docs = $this->page(PageType::Category, ['en' => ['Docs', 'docs']]);
        $page = $this->page(PageType::Doc, ['en' => 'Guide', 'tr' => 'Rehber']);

        $this->put("/admin/pages/{$page->id}", ['type' => 'doc', 'parent_id' => $docs->id])->assertSessionHasErrors('parent_id');
        $this->assertNull($page->fresh()->parent_id);
    }

    public function test_pages_with_children_and_last_translations_are_not_deleted(): void
    {
        $this->actingAs($this->admin());
        $blog = $this->page(PageType::Category, ['en' => 'Blog', 'tr' => 'Blog']);
        $post = $this->page(PageType::Post, ['en' => 'Hello'], $blog);

        $this->delete("/admin/pages/{$blog->id}")->assertSessionHas('error');
        $this->delete("/admin/pages/{$post->id}/en")->assertSessionHas('error');
        $this->delete("/admin/pages/{$blog->id}/en")->assertSessionHas('error');
        $this->delete("/admin/pages/{$blog->id}/tr")->assertSessionHas('status');
        $this->delete("/admin/pages/{$post->id}")->assertRedirect('/admin/pages');
        $this->assertSame(1, Page::query()->count());
    }

    public function test_siblings_are_reordered(): void
    {
        $this->actingAs($this->admin());
        $a = $this->page(PageType::Page, ['en' => 'Alpha'], attributes: ['position' => 0]);
        $b = $this->page(PageType::Page, ['en' => 'Beta'], attributes: ['position' => 1]);

        $this->post("/admin/pages/{$b->id}/move", ['direction' => 'up']);

        $this->assertSame([$b->id, $a->id], Page::query()->orderBy('position')->pluck('id')->all());
    }

    public function test_uploaded_media_can_be_deleted_only_when_unused(): void
    {
        Storage::fake('public');
        $this->actingAs($this->admin());

        $this->postJson('/admin/media', ['files' => [UploadedFile::fake()->image('Ekran Görüntüsü.png', 40, 30)]])
            ->assertCreated()->assertJsonPath('data.0.width', 40);
        $media = Media::query()->firstOrFail();
        Storage::disk('public')->assertExists($media->path);

        $this->postJson('/admin/media', ['files' => [UploadedFile::fake()->createWithContent('x.svg', '<svg onload="alert(1)"/>')]])
            ->assertUnprocessable();

        $page = $this->page(PageType::Page, ['en' => 'Gallery']);
        $this->translation($page, 'en')->update(['blocks' => [['id' => 'i1', 'type' => 'image', 'data' => ['image' => $media->id]]]]);
        $this->delete("/admin/media/{$media->id}")->assertSessionHas('error');

        $this->translation($page, 'en')->update(['blocks' => []]);
        $this->delete("/admin/media/{$media->id}")->assertSessionHas('status');
        Storage::disk('public')->assertMissing($media->path);
    }

    public function test_language_rules_protect_addresses(): void
    {
        $this->actingAs($this->admin());
        $this->page(PageType::Page, ['tr' => 'Hakkında']);

        $this->delete('/admin/languages/en')->assertSessionHas('error');
        $this->delete('/admin/languages/tr')->assertSessionHas('error');
        $this->put('/admin/languages/tr', ['code' => 'tk', 'name' => 'Türkçe', 'native_name' => 'Türkçe', 'locale_tag' => 'tr-TR', 'is_active' => 1])
            ->assertSessionHasErrors('code');

        $this->post('/admin/languages', ['code' => 'fr', 'name' => 'Fransızca', 'native_name' => 'Français', 'locale_tag' => 'fr-FR', 'is_default' => 1]);
        $this->assertSame('fr', Language::default()->code);
        $this->assertSame(1, Language::query()->where('is_default', true)->count());
    }

    public function test_content_changes_ask_the_website_to_revalidate(): void
    {
        config(['cms.revalidate_url' => 'http://web.test/api/revalidate', 'cms.revalidate_secret' => 'shh']);
        Http::fake(['web.test/*' => Http::response(['revalidated' => true])]);
        $this->actingAs($this->admin());
        $page = $this->page(PageType::Page, ['en' => ['Features', 'features']]);
        Http::fake(['web.test/*' => Http::response(['revalidated' => true])]);

        $this->put("/admin/pages/{$page->id}/en", ['title' => 'Features!', 'slug' => 'features', 'status' => 'published', 'blocks' => '[]']);

        Http::assertSentCount(1);
        Http::assertSent(fn (Request $request) => $request->url() === 'http://web.test/api/revalidate'
            && $request->hasHeader('Authorization', 'Bearer shh')
            && $request['tags'] === ['cms']);
    }

    public function test_later_programs_arrive_as_drafts_after_the_other_programs(): void
    {
        Storage::fake('public');
        $this->seed(ContentSeeder::class);
        $this->seed(ProgramSeeder::class);
        $this->seed(ProgramSeeder::class);

        $platform = Page::query()->where('product', 'geo-seo-platform')->sole();
        $this->assertSame(4, $platform->translations()->count());
        $this->assertSame([PublishStatus::Draft], $platform->translations->pluck('status')->unique()->values()->all());
        $order = Page::query()->whereNull('parent_id')->orderBy('position')->pluck('product')->all();
        $this->assertSame(
            [null, 'seo-crawler', 'log-analyser', 'serp-checker', 'seo-keyword-research', 'index-checker', 'geo-seo-platform', null],
            array_slice($order, 0, 8),
        );
        $hero = $platform->translations->firstWhere('locale', 'en')->blocks[0];
        $this->assertSame('hero', $hero['type']);
        $this->assertNotNull(Media::query()->find($hero['data']['image']));

        // A draft stays off the website until it is published in the admin.
        $home = $this->getJson('/api/v1/pages/resolve?locale=en')->assertOk();
        $this->assertNotContains('geo-seo-platform', array_column($home->json('data.blocks.1.products'), 'product'));
        $this->getJson('/api/v1/pages/resolve?locale=en&path=geo-seo-platform')->assertNotFound();

        // Published, it is a whole program page.
        $platform->translations()->update(['status' => PublishStatus::Published, 'published_at' => now()]);
        $page = $this->getJson('/api/v1/pages/resolve?locale=tr&path=geo-seo-platformu')->assertOk();
        $this->assertSame(
            ['hero', 'feature_grid', 'sections', 'credit_calculator', 'faq', 'code', 'product_downloads', 'cta'],
            array_column($page->json('data.blocks'), 'type'),
        );
        $this->assertSame('geo-seo-platform', $page->json('data.blocks.0.data.product'));
        $this->assertStringContainsString('octopuson-platform mcp', $page->json('data.blocks.0.data.code'));
        $home = $this->getJson('/api/v1/pages/resolve?locale=en')->assertOk();
        $this->assertSame('geo-seo-platform', array_column($home->json('data.blocks.1.products'), 'product')[5]);
    }

    public function test_the_seed_imports_the_website_content(): void
    {
        Storage::fake('public');
        $this->seed(ContentSeeder::class);

        $this->assertSame(13, Page::query()->count());
        $this->assertSame(52, PageTranslation::query()->count());
        $this->getJson('/api/v1/pages/resolve?locale=tr&path=features')->assertJsonPath('redirect.to', '/tr/log-analyser');
        $this->getJson('/api/v1/pages/resolve?locale=de&path=funktionen')->assertJsonPath('redirect.to', '/de/log-analyser');
        $this->getJson('/api/v1/pages/resolve?locale=en&path=spider')->assertJsonPath('redirect.to', '/en/seo-crawler');
        $home = $this->getJson('/api/v1/pages/resolve?locale=en')->assertOk();
        $this->assertSame(
            ['seo-crawler', 'log-analyser', 'serp-checker', 'seo-keyword-research', 'index-checker'],
            array_column($home->json('data.blocks.1.products'), 'product'),
        );
        $this->assertContains('credit_calculator', array_column($home->json('data.blocks'), 'type'));
        $this->getJson('/api/v1/site?locale=es')
            ->assertJsonPath('data.navigation.programs.0.url', '/es/seo-crawler')
            ->assertJsonPath('data.navigation.account', 'https://my.octopuson.com');
        $crawler = $this->getJson('/api/v1/pages/resolve?locale=tr&path=seo-crawler')->assertOk();
        $this->assertSame('screenshot', $crawler->json('data.blocks.0.data.visual'));
        $this->assertStringContainsString('octopuson-crawler', $crawler->json('data.blocks.0.data.code'));
        $this->assertContains('comparison', array_column($crawler->json('data.blocks'), 'type'));

        // Comparison pages live under a "Compare" category in every language.
        $versus = $this->getJson('/api/v1/pages/resolve?locale=tr&path=karsilastirma/screaming-frog-seo-spider-alternatifi')->assertOk();
        $this->assertSame('comparison', $versus->json('data.blocks.1.type'));
        $this->assertSame('Screaming Frog SEO Spider', $versus->json('data.blocks.1.data.theirs'));
        $this->assertStringContainsString('Screaming Frog', $versus->json('data.seo.title'));
        $this->getJson('/api/v1/pages/resolve?locale=en&path=compare')
            ->assertOk()
            ->assertJsonPath('data.children.total', 2);
    }
}
