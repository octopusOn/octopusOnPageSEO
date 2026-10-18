<?php

namespace App\Cms;

use App\Enums\PageType;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;
use Illuminate\Validation\ValidationException;

/**
 * Full paths of translations. A translation's path is its parent's path in the same language
 * plus its own slug; when a slug or parent changes, every translation below it moves too and
 * each old address gets a 301 to the translation it belonged to.
 */
class Paths
{
    /** Slugs the website uses itself: `page/2` paginates a category. */
    public const RESERVED = ['page', 'api', 'admin', '_next', 'sitemap.xml', 'robots.txt'];

    public const SLUG_PATTERN = '/^[a-z0-9]+(?:-[a-z0-9]+)*$/';

    /** URL-safe slug in the language's transliteration: "Özellikler" → `ozellikler`, "Änderungen" → `aenderungen`. */
    public static function slugify(string $text, string $locale = 'en'): string
    {
        return Str::slug($text, '-', explode('-', $locale)[0]);
    }

    /** The path a translation of `$page` with `$slug` gets in `$locale`. */
    public static function compose(Page $page, string $locale, string $slug): string
    {
        if ($page->type === PageType::Home) {
            return '';
        }
        $parent = $page->parent;
        if (! $parent) {
            return $slug;
        }
        $parentTranslation = $parent->translation($locale);
        if (! $parentTranslation) {
            throw ValidationException::withMessages([
                'slug' => "Önce üst sayfanın ({$parent->adminTitle()}) ".strtoupper($locale).' çevirisini oluşturun.',
            ]);
        }

        return ltrim($parentTranslation->path.'/'.$slug, '/');
    }

    /**
     * Saves a translation with `$attributes` and moves everything below it when its path
     * changed. Throws a ValidationException (keyed `slug`) when the new address is taken.
     */
    public static function save(PageTranslation $translation, array $attributes): PageTranslation
    {
        return DB::transaction(function () use ($translation, $attributes) {
            $oldPath = $translation->exists ? $translation->getOriginal('path') : null;
            $translation->fill($attributes);
            $translation->path = self::compose($translation->page, $translation->locale, (string) $translation->slug);
            self::assertFree($translation);
            $translation->save();
            self::claim($translation);

            if ($oldPath !== null && $oldPath !== $translation->path) {
                self::remember($translation, $oldPath);
                self::moveChildren($translation->page, $translation->locale);
            }

            return $translation;
        });
    }

    /** Recomputes the paths of every translation of `$page` (after its parent or type changed). */
    public static function rebuild(Page $page): void
    {
        DB::transaction(function () use ($page) {
            $page->unsetRelation('parent');
            foreach ($page->translations()->get() as $translation) {
                self::move($translation);
            }
        });
    }

    /** Languages in which `$page` has a translation but `$parent` has none. */
    public static function missingParentLanguages(Page $page, ?Page $parent): array
    {
        if (! $parent) {
            return [];
        }
        $have = $parent->translations()->pluck('locale')->all();

        return $page->translations()->whereNotIn('locale', $have)->pluck('locale')->all();
    }

    private static function moveChildren(Page $page, string $locale): void
    {
        foreach ($page->children()->get() as $child) {
            $translation = $child->translation($locale);
            if ($translation) {
                self::move($translation);
            }
        }
    }

    private static function move(PageTranslation $translation): void
    {
        $translation->unsetRelation('page');
        $oldPath = $translation->path;
        $newPath = self::compose($translation->page, $translation->locale, $translation->slug);
        if ($newPath === $oldPath) {
            return;
        }
        $translation->path = $newPath;
        self::assertFree($translation);
        $translation->save();
        self::claim($translation);
        self::remember($translation, $oldPath);
        self::moveChildren($translation->page, $translation->locale);
    }

    private static function assertFree(PageTranslation $translation): void
    {
        $taken = PageTranslation::query()
            ->where('locale', $translation->locale)
            ->where('path', $translation->path)
            ->when($translation->exists, fn ($q) => $q->whereKeyNot($translation->id))
            ->first();
        if ($taken) {
            throw ValidationException::withMessages([
                'slug' => "{$translation->url()} adresi zaten “{$taken->title}” sayfasında kullanılıyor.",
            ]);
        }
    }

    /** The old address now leads to the translation. */
    private static function remember(PageTranslation $translation, string $oldPath): void
    {
        Redirect::query()->updateOrCreate(
            ['from_path' => PageTranslation::urlFor($translation->locale, $oldPath)],
            ['translation_id' => $translation->id, 'target' => null, 'status_code' => 301, 'automatic' => true],
        );
    }

    /** A live address is no longer redirected anywhere. */
    private static function claim(PageTranslation $translation): void
    {
        Redirect::query()->where('from_path', $translation->url())->delete();
    }
}
