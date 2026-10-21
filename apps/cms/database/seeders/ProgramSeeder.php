<?php

namespace Database\Seeders;

use App\Cms\Blocks;
use App\Cms\Paths;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Page;
use Illuminate\Database\Seeder;
use Illuminate\Support\Facades\DB;

/**
 * Pages of programs that came after the website's first content (programs/<product>.json, a page
 * as content.json has them), with their screenshot (media/<product>.jpg, `@media:screenshot-<product>`).
 * A program that already has a page is skipped, so edits made in the admin are never
 * overwritten. New pages arrive as drafts in every language the site has: publish them in the
 * admin once the program is out. Runs from its migration on existing sites and from
 * DatabaseSeeder on new ones.
 */
class ProgramSeeder extends Seeder
{
    use SeedsMedia;

    public function run(): void
    {
        $languages = Language::query()->pluck('code')->all();
        if ($languages === []) {
            return;
        }
        foreach (glob(__DIR__.'/programs/*.json') ?: [] as $file) {
            $data = json_decode((string) file_get_contents($file), true, flags: JSON_THROW_ON_ERROR);
            if (Page::query()->where('product', $data['product'])->exists()) {
                continue;
            }
            DB::transaction(function () use ($data, $languages) {
                $media = $this->media([[
                    'key' => 'screenshot-'.$data['product'],
                    'file' => 'media/'.$data['product'].'.jpg',
                    'alt' => ($data['translations']['en']['title'] ?? $data['product']).' window',
                ]]);
                // After the other programs, before the pages that follow them.
                $after = (int) Page::query()->whereNull('parent_id')->whereNotNull('product')->max('position');
                Page::query()->whereNull('parent_id')->where('position', '>', $after)->increment('position');
                $page = Page::query()->create([
                    'type' => $data['type'],
                    'product' => $data['product'],
                    'icon' => $data['icon'],
                    'position' => $after + 1,
                    'in_header' => $data['in_header'],
                    'footer_column' => $data['footer_column'],
                ]);
                foreach ($data['translations'] as $locale => $t) {
                    if (! in_array($locale, $languages, true)) {
                        continue;
                    }
                    $translation = $page->translations()->make(['locale' => $locale]);
                    $translation->setRelation('page', $page);
                    Paths::save($translation, [
                        'slug' => $t['slug'],
                        'title' => $t['title'],
                        'nav_title' => $t['nav_title'] ?? null,
                        'excerpt' => $t['excerpt'] ?? null,
                        'seo_title' => $t['seo_title'] ?? null,
                        'seo_description' => $t['seo_description'] ?? null,
                        'blocks' => Blocks::clean($this->withMedia($t['blocks'], $media)),
                        'status' => PublishStatus::Draft,
                        'published_at' => null,
                    ]);
                }
            });
            $this->command?->info("Program sayfası taslak olarak eklendi: {$data['product']}");
        }
    }
}
