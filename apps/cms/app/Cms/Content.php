<?php

namespace App\Cms;

use App\Enums\PageType;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Support\Collection;

/**
 * What the website reads: languages, menus, a page by its address, the sitemap; and the account
 * site's legal texts, which the website never shows. Only live translations of active languages
 * are visible, unless `$drafts` is set for a preview.
 */
class Content
{
    public function __construct(private readonly bool $drafts = false) {}

    /** @return list<array<string, mixed>> */
    public function languages(): array
    {
        return Language::query()->active()->ordered()->get()
            ->map(fn (Language $language) => [
                'code' => $language->code,
                'name' => $language->name,
                'native_name' => $language->native_name,
                'locale_tag' => $language->locale_tag,
                'is_default' => $language->is_default,
            ])->values()->all();
    }

    /** Header links and footer columns of a language. */
    public function navigation(string $locale): array
    {
        $translations = $this->website()
            ->where('locale', $locale)
            ->whereHas('page', fn (Builder $q) => $q->where(fn (Builder $q) => $q->where('in_header', true)->orWhereNotNull('footer_column')))
            ->with('page')
            ->get()
            ->sortBy(fn (PageTranslation $t) => [$t->page->position, $t->page_id])
            ->values();

        $link = fn (PageTranslation $t, bool $footer = false) => [
            'label' => $footer && $t->page->footer_column === 'product' && $t->page->product
                ? config('cms.products')[$t->page->product] ?? $t->label()
                : $t->label(),
            'url' => $t->url(),
            'product' => $t->page->product,
        ];
        $footer = [];
        foreach (array_keys(config('cms.footer_columns')) as $column) {
            $footer[$column] = $translations->filter(fn ($t) => $t->page->footer_column === $column)->map(fn ($t) => $link($t, true))->values()->all();
        }

        return [
            'account' => config('cms.account_url'),
            'header' => $translations->filter(fn ($t) => $t->page->in_header)->map(fn ($t) => $link($t))->values()->all(),
            'footer' => $footer,
            // Every program's page, for the header's program menu.
            'programs' => $this->products($locale),
            // The header's download button leads to the page with every program's downloads.
            'download' => $this->website()->where('locale', $locale)->get(['locale', 'path', 'blocks'])
                ->first(fn (PageTranslation $t) => collect($t->blocks)->contains('type', 'downloads'))?->url(),
        ];
    }

    /**
     * The page at `/{locale}/{path}`: `['page' => …]`, `['redirect' => …]` for an old address,
     * or null. `blog/page/2` is the second page of the `blog` category's listing.
     */
    public function resolve(string $locale, string $path): ?array
    {
        if (! $this->languageVisible($locale)) {
            return null;
        }
        $path = trim($path, '/');
        $translation = $this->find($locale, $path);
        if ($translation) {
            return ['page' => $this->page($translation)];
        }
        if (preg_match('~^(?:(.*)/)?page/([1-9][0-9]{0,4})$~', $path, $match) && (int) $match[2] >= 2) {
            $listing = $this->find($locale, $match[1]);
            if ($listing && $this->lists($listing)) {
                $page = $this->page($listing, (int) $match[2]);

                return $page['children'] && $page['children']['page'] <= $page['children']['last_page'] ? ['page' => $page] : null;
            }
        }

        return $this->redirect($locale, $path);
    }

    /** Every indexable live address with its language versions. */
    public function sitemap(): array
    {
        $active = Language::query()->active()->pluck('code')->all();
        $all = $this->website()->whereIn('locale', $active)->with('page')->get()->groupBy('page_id');
        $urls = [];
        foreach ($all as $translations) {
            $alternates = $translations->mapWithKeys(fn (PageTranslation $t) => [$t->locale => $t->url()])->all();
            foreach ($translations as $translation) {
                if ($translation->noindex || ($translation->canonical_url && $translation->canonical_url !== $translation->url())) {
                    continue;
                }
                $urls[] = [
                    'url' => $translation->url(),
                    'locale' => $translation->locale,
                    'type' => $translation->page->type->value,
                    'updated_at' => $translation->updated_at?->toIso8601String(),
                    'alternates' => $alternates,
                ];
            }
        }

        return ['default_locale' => Language::defaultCode(), 'urls' => $urls];
    }

    /** Full API representation of a translation; `$listingPage` pages through its children. */
    public function page(PageTranslation $translation, int $listingPage = 1): array
    {
        $translation->loadMissing('page', 'image');
        $page = $translation->page;
        $siblings = $this->visible()->where('page_id', $page->id)
            ->whereIn('locale', Language::query()->active()->pluck('code'))
            ->get();
        $default = Language::defaultCode();
        $xDefault = $siblings->firstWhere('locale', $default);

        return [
            'id' => $translation->id,
            'page_id' => $page->id,
            'type' => $page->type->value,
            'product' => $page->product,
            'icon' => $page->icon,
            'locale' => $translation->locale,
            'path' => $translation->path,
            'url' => $translation->url(),
            'title' => $translation->title,
            'nav_title' => $translation->nav_title,
            'excerpt' => $translation->excerpt,
            'status' => $translation->status->value,
            'published_at' => $translation->published_at?->toIso8601String(),
            'updated_at' => $translation->updated_at?->toIso8601String(),
            'seo' => [
                'title' => $translation->seo_title ?: $translation->title,
                'description' => $translation->seo_description ?: $translation->excerpt,
                'canonical' => $translation->canonical_url,
                'noindex' => $translation->noindex,
                'image' => $translation->image?->toApi(),
            ],
            'alternates' => $siblings->map(fn (PageTranslation $t) => ['locale' => $t->locale, 'url' => $t->url()])->values()->all(),
            'x_default' => $xDefault?->url(),
            'breadcrumbs' => $this->breadcrumbs($translation),
            'blocks' => $this->blocks($translation->blocks ?? [], $translation->locale),
            'children' => $this->lists($translation) ? $this->children($translation, $listingPage) : null,
        ];
    }

    /**
     * The account site's legal texts, in page order: each in `$locale`, else in the default
     * language, else in any language it has.
     *
     * @return list<array<string, mixed>>
     */
    public function legal(string $locale): array
    {
        return $this->legalTranslations()
            ->map(fn (Collection $translations) => $this->legalSummary($this->pick($translations, $locale)))
            ->values()->all();
    }

    /**
     * One legal text for `/{locale}/…/{slug}`: found by its slug in `$locale`, by its key, or by
     * its slug in another language, and returned in `$locale` when it exists there.
     */
    public function legalDocument(string $locale, string $slug): ?array
    {
        $groups = $this->legalTranslations();
        $translations = $groups->first(fn (Collection $ts) => $ts->contains(fn (PageTranslation $t) => $t->locale === $locale && $t->slug === $slug))
            ?? $groups->first(fn (Collection $ts) => $ts->first()?->page->key === $slug)
            ?? $groups->first(fn (Collection $ts) => $ts->contains('slug', $slug));
        if (! $translations) {
            return null;
        }
        $translation = $this->pick($translations, $locale);

        return $this->legalSummary($translation) + [
            'excerpt' => $translation->excerpt,
            'alternates' => $translations->map(fn (PageTranslation $t) => ['locale' => $t->locale, 'slug' => $t->slug])->values()->all(),
            'blocks' => $this->blocks($translation->blocks ?? [], $translation->locale),
        ];
    }

    /** @return Collection<int, Collection<int, PageTranslation>> visible translations of the legal pages, by page */
    private function legalTranslations(): Collection
    {
        return $this->visible()
            ->whereIn('locale', Language::query()->active()->pluck('code'))
            ->whereHas('page', fn (Builder $q) => $q->where('type', PageType::Legal))
            ->with('page')->get()
            ->sortBy(fn (PageTranslation $t) => [$t->page->position, $t->page_id])
            ->groupBy('page_id')
            ->values();
    }

    /** @param  Collection<int, PageTranslation>  $translations */
    private function pick(Collection $translations, string $locale): PageTranslation
    {
        return $translations->firstWhere('locale', $locale)
            ?? $translations->firstWhere('locale', Language::defaultCode())
            ?? $translations->first();
    }

    private function legalSummary(PageTranslation $translation): array
    {
        return [
            'key' => $translation->page->key,
            'locale' => $translation->locale,
            'slug' => $translation->slug,
            'title' => $translation->title,
            // The short name for menus: the translation's menu title, else its title.
            'label' => $translation->label(),
            'updated_at' => $translation->updated_at?->toIso8601String(),
        ];
    }

    private function find(string $locale, string $path): ?PageTranslation
    {
        return $this->website()->where('locale', $locale)->where('path', $path)->first();
    }

    private function visible(): Builder
    {
        $query = PageTranslation::query();
        if (! $this->drafts) {
            $query->live();
        }

        return $query;
    }

    /** Visible translations of the website's pages: everything but the account site's legal texts. */
    private function website(): Builder
    {
        return $this->visible()->whereHas('page', fn (Builder $q) => $q->where('type', '!=', PageType::Legal));
    }

    private function languageVisible(string $locale): bool
    {
        return Language::query()->where('code', $locale)->when(! $this->drafts, fn ($q) => $q->where('is_active', true))->exists();
    }

    private function lists(PageTranslation $translation): bool
    {
        return $translation->page->type === PageType::Category
            || collect($translation->blocks ?? [])->contains(fn ($block) => ($block['type'] ?? null) === 'children');
    }

    private function redirect(string $locale, string $path): ?array
    {
        $candidates = [PageTranslation::urlFor($locale, $path), Redirect::normalize($path)];
        $redirect = Redirect::query()->whereIn('from_path', $candidates)->with('translation')->get()
            ->sortBy(fn (Redirect $r) => array_search($r->from_path, $candidates, true))->first();
        $to = $redirect?->destination($this->drafts);
        if (! $to || $redirect->translation?->page?->type === PageType::Legal) {
            return null;
        }
        if (! $this->drafts) {
            Redirect::query()->whereKey($redirect->id)->update(['hits' => $redirect->hits + 1, 'last_hit_at' => now()]);
        }

        return ['redirect' => ['to' => $to, 'status' => $redirect->status_code]];
    }

    /** Home, the ancestors and the page itself, in the page's language. */
    private function breadcrumbs(PageTranslation $translation): array
    {
        $crumbs = [];
        $page = $translation->page;
        if ($page->type !== PageType::Home) {
            $home = $this->visible()->where('locale', $translation->locale)
                ->whereHas('page', fn (Builder $q) => $q->where('type', PageType::Home))->first();
            if ($home) {
                $crumbs[] = ['title' => $home->label(), 'url' => $home->url()];
            }
        }
        foreach ($page->ancestors() as $ancestor) {
            if ($ancestor->type === PageType::Home) {
                continue;
            }
            $t = $this->visible()->where('page_id', $ancestor->id)->where('locale', $translation->locale)->first();
            if ($t) {
                $crumbs[] = ['title' => $t->label(), 'url' => $t->url()];
            }
        }
        $crumbs[] = ['title' => $translation->label(), 'url' => $translation->url()];

        return $crumbs;
    }

    private function children(PageTranslation $translation, int $page): array
    {
        $perPage = (int) config('cms.per_page', 12);
        $query = $this->visible()
            ->where('page_translations.locale', $translation->locale)
            ->join('pages', 'pages.id', '=', 'page_translations.page_id')
            ->where('pages.parent_id', $translation->page_id)
            ->where('pages.type', '!=', PageType::Legal->value)
            ->select('page_translations.*')
            ->orderBy('pages.position')
            ->orderByDesc('page_translations.published_at')
            ->orderBy('pages.id')
            ->with('page', 'image');
        $total = (clone $query)->count();
        $items = $query->forPage($page, $perPage)->get();

        return [
            'items' => $items->map(fn (PageTranslation $t) => [
                'title' => $t->title,
                'excerpt' => $t->excerpt,
                'url' => $t->url(),
                'type' => $t->page->type->value,
                'icon' => $t->page->icon,
                'product' => $t->page->product,
                'published_at' => $t->published_at?->toIso8601String(),
                'image' => $t->image?->toApi(),
            ])->values()->all(),
            'page' => $page,
            'last_page' => max(1, (int) ceil($total / $perPage)),
            'total' => $total,
            'base_url' => $translation->url(),
        ];
    }

    /** Blocks with media ids replaced by files and product cards filled in. */
    private function blocks(array $blocks, string $locale): array
    {
        $definitions = Blocks::definitions();
        $media = Media::query()->whereIn('id', Blocks::mediaIds($blocks))->get()->keyBy('id');
        $products = null;
        $out = [];
        foreach ($blocks as $block) {
            $definition = $definitions[$block['type'] ?? ''] ?? null;
            if (! $definition) {
                continue;
            }
            $item = [
                'id' => $block['id'] ?? null,
                'type' => $block['type'],
                'data' => $this->expandMedia($definition['fields'], $block['data'] ?? [], $media),
            ];
            if (in_array($block['type'], ['products', 'downloads'], true)) {
                $item['products'] = $products ??= $this->products($locale);
            }
            $out[] = $item;
        }

        return $out;
    }

    private function expandMedia(array $fields, array $data, Collection $media): array
    {
        foreach ($fields as $field) {
            $name = $field['name'];
            if ($field['type'] === 'media') {
                $data[$name] = isset($data[$name]) ? $media->get($data[$name])?->toApi() : null;
            } elseif ($field['type'] === 'items' && is_array($data[$name] ?? null)) {
                $data[$name] = array_map(fn ($item) => $this->expandMedia($field['fields'], is_array($item) ? $item : [], $media), $data[$name]);
            }
        }

        return $data;
    }

    /** Pages that present a program, in this language: the product cards and download sections. */
    private function products(string $locale): array
    {
        return $this->website()->where('locale', $locale)
            ->whereHas('page', fn (Builder $q) => $q->whereNotNull('product'))
            ->with('page')->get()
            ->sortBy(fn (PageTranslation $t) => [$t->page->position, $t->page_id])
            ->map(fn (PageTranslation $t) => [
                'product' => $t->page->product,
                'name' => config('cms.products')[$t->page->product] ?? $t->title,
                'title' => $t->title,
                'excerpt' => $t->excerpt,
                'url' => $t->url(),
                'icon' => $t->page->icon,
            ])->values()->all();
    }
}
