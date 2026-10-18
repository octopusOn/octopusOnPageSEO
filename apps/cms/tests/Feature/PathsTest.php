<?php

namespace Tests\Feature;

use App\Cms\Paths;
use App\Enums\PageType;
use App\Models\Redirect;
use Illuminate\Foundation\Testing\RefreshDatabase;
use Illuminate\Validation\ValidationException;
use Tests\TestCase;

class PathsTest extends TestCase
{
    use RefreshDatabase;

    protected function setUp(): void
    {
        parent::setUp();
        $this->languages();
    }

    public function test_paths_follow_the_parent_in_each_language(): void
    {
        $blog = $this->page(PageType::Category, ['en' => 'Blog', 'tr' => ['Blog', 'blog']]);
        $post = $this->page(PageType::Post, ['en' => 'Log analysis guide', 'tr' => 'Log analizi rehberi'], $blog);

        $this->assertSame('blog/log-analysis-guide', $this->translation($post, 'en')->path);
        $this->assertSame('/tr/blog/log-analizi-rehberi', $this->translation($post, 'tr')->url());
    }

    public function test_slugs_are_transliterated_per_language(): void
    {
        $this->assertSame('ozellikler', Paths::slugify('Özellikler', 'tr'));
        $this->assertSame('aenderungen', Paths::slugify('Änderungen', 'de'));
        $this->assertSame('surum-notlari', Paths::slugify('Sürüm notları', 'tr'));
    }

    public function test_a_renamed_parent_moves_its_children_and_leaves_301s(): void
    {
        $docs = $this->page(PageType::Category, ['tr' => ['Dokümanlar', 'dokumanlar']]);
        $guide = $this->page(PageType::Doc, ['tr' => ['Hızlı başlangıç', 'hizli-baslangic']], $docs);

        Paths::save($this->translation($docs, 'tr'), ['slug' => 'dokumantasyon']);

        $child = $this->translation($guide, 'tr');
        $this->assertSame('dokumantasyon/hizli-baslangic', $child->path);
        $this->assertSame($child->id, Redirect::query()->where('from_path', '/tr/dokumanlar/hizli-baslangic')->value('translation_id'));
        $this->assertTrue(Redirect::query()->where('from_path', '/tr/dokumanlar')->exists());
    }

    public function test_redirects_follow_a_page_that_moves_again(): void
    {
        $page = $this->page(PageType::Page, ['en' => ['About', 'about']]);
        Paths::save($this->translation($page, 'en'), ['slug' => 'about-us']);
        Paths::save($this->translation($page, 'en')->fresh(), ['slug' => 'company']);

        $redirect = Redirect::query()->where('from_path', '/en/about')->firstOrFail();
        $this->assertSame('/en/company', $redirect->destination());
    }

    public function test_an_address_that_is_live_again_is_no_longer_redirected(): void
    {
        $page = $this->page(PageType::Page, ['en' => ['Pricing', 'pricing']]);
        Paths::save($this->translation($page, 'en'), ['slug' => 'plans']);
        $this->assertTrue(Redirect::query()->where('from_path', '/en/pricing')->exists());

        $this->page(PageType::Page, ['en' => ['Pricing', 'pricing']]);

        $this->assertFalse(Redirect::query()->where('from_path', '/en/pricing')->exists());
    }

    public function test_two_pages_cannot_share_an_address(): void
    {
        $this->page(PageType::Page, ['en' => ['Features', 'features']]);

        $this->expectException(ValidationException::class);
        $this->page(PageType::Page, ['en' => ['Other features', 'features']]);
    }

    public function test_a_child_needs_its_parents_translation(): void
    {
        $blog = $this->page(PageType::Category, ['en' => 'Blog']);

        $this->expectException(ValidationException::class);
        $this->page(PageType::Post, ['tr' => 'Yazı'], $blog);
    }

    public function test_moving_a_page_rebuilds_its_paths(): void
    {
        $docs = $this->page(PageType::Category, ['en' => ['Docs', 'docs'], 'tr' => ['Dokümanlar', 'dokumanlar']]);
        $page = $this->page(PageType::Page, ['en' => ['Query language', 'query-language'], 'tr' => ['Sorgu dili', 'sorgu-dili']]);

        $page->update(['parent_id' => $docs->id]);
        Paths::rebuild($page);

        $this->assertSame('/en/docs/query-language', $this->translation($page, 'en')->url());
        $this->assertSame('/tr/dokumanlar/sorgu-dili', $this->translation($page, 'tr')->url());
        $this->assertTrue(Redirect::query()->where('from_path', '/tr/sorgu-dili')->exists());
    }
}
