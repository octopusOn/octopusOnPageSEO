<?php

namespace App\Http\Controllers\Admin;

use App\Cms\Blocks;
use App\Cms\Paths;
use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Http\Controllers\Controller;
use App\Models\Language;
use App\Models\Page;
use App\Models\PageTranslation;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use Illuminate\View\View;

/** The page tree and each page's language-independent settings. */
class PageController extends Controller
{
    public function index(Request $request): View
    {
        $languages = Language::ordered();
        $type = PageType::tryFrom((string) $request->query('type'));
        $query = mb_strtolower(trim((string) $request->query('q')));
        $rows = collect(self::flatten(Page::query()->with('translations')->get()))
            ->filter(fn (array $row) => ! $type || $row['page']->type === $type)
            ->filter(fn (array $row) => $query === '' || $row['page']->translations->contains(
                fn (PageTranslation $t) => str_contains(mb_strtolower($t->title.' '.$t->url()), $query),
            ));

        return view('admin.pages.index', [
            'rows' => $rows,
            'languages' => $languages,
            'contentLocale' => Language::defaultCode(),
            'type' => $type,
            'query' => $request->query('q'),
        ]);
    }

    public function create(Request $request): View
    {
        $parent = Page::query()->find($request->query('parent'));
        $type = PageType::tryFrom((string) $request->query('type'))
            ?? ($parent?->type === PageType::Category ? PageType::Post : PageType::Page);

        return view('admin.pages.create', [
            'page' => new Page(['type' => $type, 'parent_id' => $parent?->id]),
            'parents' => self::parentOptions(),
            'languages' => Language::ordered(),
            'defaultLocale' => Language::defaultCode(),
        ]);
    }

    public function store(Request $request): RedirectResponse
    {
        $settings = $this->validateSettings($request);
        $first = $request->validate([
            'locale' => ['required', Rule::exists('languages', 'code')],
            'title' => ['required', 'string', 'max:255'],
            'slug' => ['nullable', 'string', 'max:160'],
        ]);
        $page = DB::transaction(function () use ($settings, $first) {
            $page = new Page($settings);
            $page->position = (int) Page::query()->where('parent_id', $settings['parent_id'])->max('position') + 1;
            $this->assertStructure($page);
            $page->save();
            $translation = $page->translations()->make(['locale' => $first['locale']]);
            $translation->setRelation('page', $page);
            Paths::save($translation, [
                'title' => $first['title'],
                'slug' => self::slug($first['slug'] ?? '', $first['title'], $first['locale'], $page),
                'blocks' => self::starterBlocks($page->type),
                'status' => PublishStatus::Draft,
                'updated_by' => auth()->id(),
            ]);

            return $page;
        });

        return redirect()
            ->route('admin.translations.edit', [$page, $first['locale']])
            ->with('status', 'Sayfa oluşturuldu. İçeriğini düzenleyip yayına alın.');
    }

    public function edit(Page $page): View
    {
        $page->load('translations.editor', 'parent.translations', 'children.translations');

        return view('admin.pages.edit', [
            'page' => $page,
            'parents' => self::parentOptions($page),
            'languages' => Language::ordered(),
            'contentLocale' => Language::defaultCode(),
        ]);
    }

    public function update(Request $request, Page $page): RedirectResponse
    {
        $settings = $this->validateSettings($request, $page);
        DB::transaction(function () use ($page, $settings) {
            $moved = (int) $page->parent_id !== (int) $settings['parent_id'] || $page->type->value !== $settings['type'];
            $page->fill($settings);
            $this->assertStructure($page);
            if ($moved) {
                $missing = Paths::missingParentLanguages($page, $page->parent_id ? Page::query()->find($page->parent_id) : null);
                if ($missing) {
                    throw ValidationException::withMessages([
                        'parent_id' => 'Yeni üst sayfanın şu dillerde çevirisi yok: '.strtoupper(implode(', ', $missing)).'. Önce o çevirileri oluşturun.',
                    ]);
                }
                if ($page->parent_id) {
                    $page->position = (int) Page::query()->where('parent_id', $page->parent_id)->max('position') + 1;
                }
            }
            $page->save();
            if ($moved) {
                Paths::rebuild($page);
            }
        });

        return back()->with('status', 'Sayfa ayarları kaydedildi.');
    }

    public function destroy(Page $page): RedirectResponse
    {
        if ($page->children()->exists()) {
            return back()->with('error', 'Alt sayfaları olan bir sayfa silinemez. Önce alt sayfaları silin ya da taşıyın.');
        }
        $title = $page->adminTitle();
        $page->delete();

        return redirect()->route('admin.pages.index')->with('status', "“{$title}” ve tüm çevirileri silindi.");
    }

    /** Moves a page one place up or down among its siblings. */
    public function move(Request $request, Page $page): RedirectResponse
    {
        $step = $request->input('direction') === 'up' ? -1 : 1;
        DB::transaction(function () use ($page, $step) {
            $siblings = Page::query()->where('parent_id', $page->parent_id)->orderBy('position')->orderBy('id')->get()->values();
            $index = $siblings->search(fn (Page $p) => $p->id === $page->id);
            $target = $index + $step;
            if ($target < 0 || $target >= $siblings->count()) {
                return;
            }
            $order = $siblings->all();
            [$order[$index], $order[$target]] = [$order[$target], $order[$index]];
            foreach ($order as $position => $sibling) {
                if ($sibling->position !== $position) {
                    $sibling->position = $position;
                    $sibling->save();
                }
            }
        });

        return back();
    }

    /** @return array<string, mixed> */
    private function validateSettings(Request $request, ?Page $page = null): array
    {
        $data = $request->validate([
            'type' => ['required', Rule::enum(PageType::class)],
            'parent_id' => ['nullable', 'integer', Rule::exists('pages', 'id')],
            'product' => ['nullable', Rule::in(array_keys(config('cms.products')))],
            'icon' => ['nullable', Rule::in(config('cms.icons'))],
            'in_header' => ['boolean'],
            'footer_column' => ['nullable', Rule::in(array_keys(config('cms.footer_columns')))],
            'key' => ['nullable', 'string', 'max:40', 'regex:/^[a-z0-9]+(-[a-z0-9]+)*$/', Rule::unique('pages', 'key')->ignore($page?->id)],
        ], [
            'key.regex' => 'Anahtar yalnızca küçük harf, rakam ve tire içerebilir: privacy, credit-terms.',
            'key.unique' => 'Bu anahtar başka bir sayfada kullanılıyor.',
        ]);
        $data['parent_id'] = $data['parent_id'] ?? null;
        $data['key'] = $data['key'] ?? null;
        if ($data['type'] === PageType::Legal->value && $data['parent_id']) {
            throw ValidationException::withMessages(['parent_id' => 'Yasal metinler en üst düzeyde durur; üst sayfa seçmeyin.']);
        }
        $data['in_header'] = $request->boolean('in_header');
        if ($page && $data['parent_id'] && in_array((int) $data['parent_id'], $page->descendantIds(), true)) {
            throw ValidationException::withMessages(['parent_id' => 'Bir sayfa kendisinin ya da alt sayfasının altına taşınamaz.']);
        }

        return $data;
    }

    private function assertStructure(Page $page): void
    {
        if ($page->type !== PageType::Home) {
            return;
        }
        if ($page->parent_id) {
            throw ValidationException::withMessages(['parent_id' => 'Ana sayfanın üst sayfası olamaz.']);
        }
        $other = Page::query()->where('type', PageType::Home)->when($page->exists, fn ($q) => $q->whereKeyNot($page->id))->exists();
        if ($other) {
            throw ValidationException::withMessages(['type' => 'Sitede zaten bir ana sayfa var.']);
        }
    }

    private static function slug(string $slug, string $title, string $locale, Page $page): string
    {
        if ($page->type === PageType::Home) {
            return '';
        }
        $slug = Paths::slugify($slug !== '' ? $slug : $title, $locale);
        if ($slug === '' || in_array($slug, Paths::RESERVED, true)) {
            throw ValidationException::withMessages(['slug' => 'Bu adres kullanılamaz; başka bir kısa ad yazın.']);
        }

        return $slug;
    }

    /** First blocks of a new page, so editors start from a sensible layout. */
    public static function starterBlocks(PageType $type): array
    {
        $types = match ($type) {
            PageType::Home => ['hero', 'feature_grid', 'cta'],
            PageType::Category, PageType::Page => ['page_header'],
            PageType::Product => ['page_header', 'sections', 'product_downloads'],
            PageType::Post, PageType::Doc, PageType::Legal => ['rich_text'],
        };
        $blocks = array_map(fn (string $t) => ['id' => 'b'.strtolower(str()->random(10)), 'type' => $t, 'data' => Blocks::blank($t)], $types);
        foreach ($blocks as &$block) {
            if ($block['type'] === 'page_header' && $type === PageType::Product) {
                $block['data']['download'] = true;
            }
        }

        return $blocks;
    }

    /**
     * Pages in tree order with their depth.
     *
     * @param  Collection<int, Page>  $pages
     * @return list<array{page: Page, depth: int}>
     */
    public static function flatten(Collection $pages): array
    {
        $byParent = $pages->groupBy(fn (Page $p) => $p->parent_id ?? 0);
        $rows = [];
        $walk = function (int $parent, int $depth) use (&$walk, &$rows, $byParent) {
            foreach (($byParent[$parent] ?? collect())->sortBy([['position', 'asc'], ['id', 'asc']]) as $page) {
                $rows[] = ['page' => $page, 'depth' => $depth];
                $walk($page->id, $depth + 1);
            }
        };
        $walk(0, 0);

        return $rows;
    }

    /** Pages a page can be placed under: every page but `$except` and its descendants. */
    private static function parentOptions(?Page $except = null): array
    {
        $exclude = $except ? $except->descendantIds() : [];
        $locale = Language::defaultCode();

        return collect(self::flatten(Page::query()->with('translations')->get()))
            ->reject(fn (array $row) => in_array($row['page']->id, $exclude, true))
            ->map(fn (array $row) => [
                'id' => $row['page']->id,
                'label' => str_repeat('— ', $row['depth']).$row['page']->adminTitle($locale),
                'type' => $row['page']->type,
            ])->values()->all();
    }
}
