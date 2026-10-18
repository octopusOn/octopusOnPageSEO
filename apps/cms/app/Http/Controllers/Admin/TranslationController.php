<?php

namespace App\Http\Controllers\Admin;

use App\Cms\Blocks;
use App\Cms\Paths;
use App\Cms\Preview;
use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Http\Controllers\Controller;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\PageTranslation;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Carbon;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use Illuminate\View\View;

/** A page in one language: texts, blocks, SEO and publishing. */
class TranslationController extends Controller
{
    public function edit(Page $page, Language $language): View
    {
        $page->load('translations', 'parent.translations');
        $translation = $page->translation($language->code) ?? $this->draftFrom($page, $language);
        $parentTranslation = $page->parent?->translation($language->code);
        $parentUrl = $page->type === PageType::Home || ! $page->parent
            ? '/'.$language->code
            : ($parentTranslation ? $parentTranslation->url() : null);
        $mediaIds = array_filter([...Blocks::mediaIds($translation->blocks ?? []), $translation->image_id]);

        return view('admin.translations.edit', [
            'page' => $page,
            'language' => $language,
            'translation' => $translation,
            'languages' => Language::ordered(),
            'parentUrl' => $parentUrl,
            'definitions' => Blocks::definitions(),
            'media' => Media::query()->whereIn('id', $mediaIds)->get()->map(fn (Media $m) => $m->toApi() + ['filename' => $m->filename])->values(),
            'previewUrl' => $translation->exists ? Preview::url($translation) : null,
            'liveUrl' => $translation->exists && $translation->isLive() ? config('cms.web_url').$translation->url() : null,
        ]);
    }

    public function update(Request $request, Page $page, Language $language): RedirectResponse
    {
        $data = $request->validate([
            'title' => ['required', 'string', 'max:255'],
            'slug' => ['nullable', 'string', 'max:160'],
            'nav_title' => ['nullable', 'string', 'max:255'],
            'excerpt' => ['nullable', 'string', 'max:1000'],
            'blocks' => ['nullable', 'string', 'max:500000'],
            'seo_title' => ['nullable', 'string', 'max:255'],
            'seo_description' => ['nullable', 'string', 'max:320'],
            'canonical_url' => ['nullable', 'string', 'max:255', 'regex:~^(/|https?://)~i'],
            'noindex' => ['boolean'],
            'image_id' => ['nullable', 'integer', Rule::exists('media', 'id')],
            'status' => ['required', Rule::enum(PublishStatus::class)],
            'published_at' => ['nullable', 'date'],
        ], [
            'canonical_url.regex' => 'Kanonik adres / ya da https:// ile başlamalı.',
        ]);
        $translation = $page->translation($language->code) ?? $page->translations()->make(['locale' => $language->code]);
        $translation->setRelation('page', $page);

        $slug = $page->type === PageType::Home ? '' : Paths::slugify(($data['slug'] ?? '') ?: $data['title'], $language->code);
        if ($page->type !== PageType::Home && ($slug === '' || in_array($slug, Paths::RESERVED, true))) {
            throw ValidationException::withMessages(['slug' => 'Bu adres kullanılamaz; başka bir kısa ad yazın.']);
        }
        $status = PublishStatus::from($data['status']);
        $publishedAt = ($data['published_at'] ?? null) ? Carbon::parse($data['published_at']) : null;
        if ($status === PublishStatus::Published && ! $publishedAt) {
            $publishedAt = $translation->published_at ?? now();
        }

        Paths::save($translation, [
            'title' => $data['title'],
            'slug' => $slug,
            'nav_title' => $data['nav_title'] ?? null,
            'excerpt' => $data['excerpt'] ?? null,
            'blocks' => Blocks::clean(json_decode($data['blocks'] ?? '[]', true)),
            'seo_title' => $data['seo_title'] ?? null,
            'seo_description' => $data['seo_description'] ?? null,
            'canonical_url' => $data['canonical_url'] ?? null,
            'noindex' => $request->boolean('noindex'),
            'image_id' => $data['image_id'] ?? null,
            'status' => $status,
            'published_at' => $publishedAt,
            'updated_by' => $request->user()->id,
        ]);

        $message = $status === PublishStatus::Published
            ? ($translation->isLive() ? 'Kaydedildi ve yayında.' : 'Kaydedildi; '.$translation->published_at->format('d.m.Y H:i').' tarihinde yayına girecek.')
            : 'Taslak kaydedildi.';

        return redirect()->route('admin.translations.edit', [$page, $language])->with('status', $message);
    }

    public function destroy(Page $page, Language $language): RedirectResponse
    {
        $translation = $page->translation($language->code);
        abort_unless($translation, 404);
        if ($page->translations()->count() === 1) {
            return back()->with('error', 'Sayfanın son çevirisi silinemez; sayfanın tamamını silin.');
        }
        $childIds = $page->children()->pluck('id');
        if (PageTranslation::query()->whereIn('page_id', $childIds)->where('locale', $language->code)->exists()) {
            return back()->with('error', 'Alt sayfaların '.strtoupper($language->code).' çevirileri bu sayfanın adresine bağlı. Önce onları silin.');
        }
        $translation->delete();

        return redirect()->route('admin.pages.edit', $page)->with('status', strtoupper($language->code).' çevirisi silindi.');
    }

    /** A new translation, prefilled from the default language's so translators start from the content. */
    private function draftFrom(Page $page, Language $language): PageTranslation
    {
        $source = $page->preferredTranslation();
        $translation = new PageTranslation([
            'locale' => $language->code,
            'title' => $source?->title ?? '',
            'slug' => $source?->slug ?? '',
            'nav_title' => $source?->nav_title,
            'excerpt' => $source?->excerpt,
            'blocks' => $source?->blocks ?? PageController::starterBlocks($page->type),
            'seo_title' => $source?->seo_title,
            'seo_description' => $source?->seo_description,
            'image_id' => $source?->image_id,
            'status' => PublishStatus::Draft,
        ]);
        $translation->setRelation('page', $page);

        return $translation;
    }
}
