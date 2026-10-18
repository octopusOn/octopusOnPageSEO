<?php

namespace Tests;

use App\Cms\Paths;
use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\User;
use Illuminate\Foundation\Testing\TestCase as BaseTestCase;

abstract class TestCase extends BaseTestCase
{
    /** English (default) and Turkish. */
    protected function languages(): void
    {
        Language::query()->create(['code' => 'en', 'name' => 'İngilizce', 'native_name' => 'English', 'locale_tag' => 'en-US', 'is_default' => true, 'position' => 0]);
        Language::query()->create(['code' => 'tr', 'name' => 'Türkçe', 'native_name' => 'Türkçe', 'locale_tag' => 'tr-TR', 'position' => 1]);
    }

    /**
     * A page with translations: `['en' => 'Features', 'tr' => ['Özellikler', 'ozellikler']]`.
     * A plain string is the title; the slug comes from it.
     */
    protected function page(PageType $type, array $translations, ?Page $parent = null, array $attributes = [], PublishStatus $status = PublishStatus::Published): Page
    {
        $page = Page::query()->create(['type' => $type, 'parent_id' => $parent?->id] + $attributes);
        foreach ($translations as $locale => $value) {
            [$title, $slug] = is_array($value) ? $value : [$value, Paths::slugify($value, $locale)];
            $translation = $page->translations()->make(['locale' => $locale]);
            $translation->setRelation('page', $page);
            Paths::save($translation, [
                'title' => $title,
                'slug' => $type === PageType::Home ? '' : $slug,
                'blocks' => [],
                'status' => $status,
                'published_at' => $status === PublishStatus::Published ? now()->subMinute() : null,
            ]);
        }

        return $page->fresh('translations');
    }

    protected function translation(Page $page, string $locale): PageTranslation
    {
        return $page->translations()->where('locale', $locale)->firstOrFail();
    }

    protected function admin(): User
    {
        return User::query()->create(['name' => 'Ayşe', 'email' => 'ayse@octopuson.test', 'password' => 'correct-horse-battery']);
    }
}
