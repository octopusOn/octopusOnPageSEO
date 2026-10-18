<?php

namespace Database\Seeders;

use App\Cms\Blocks;
use App\Cms\Paths;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Illuminate\Database\Seeder;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;

/**
 * The website's first content (content.json): the languages, every page in four languages,
 * the programs' screenshots (media/) and redirects from the site's earlier addresses. Blocks
 * refer to a screenshot as `@media:<key>`. Runs only on an empty site.
 */
class ContentSeeder extends Seeder
{
    public function run(): void
    {
        if (Page::query()->exists()) {
            $this->command?->warn('Sayfalar zaten var; içerik tohumu atlandı.');

            return;
        }
        $seed = json_decode((string) file_get_contents(__DIR__.'/content.json'), true, flags: JSON_THROW_ON_ERROR);

        DB::transaction(function () use ($seed) {
            foreach ($seed['languages'] as $language) {
                Language::query()->updateOrCreate(['code' => $language['code']], $language + ['is_active' => true]);
            }
            $media = $this->media($seed['media'] ?? []);
            $pages = [];
            foreach ($seed['pages'] as $position => $data) {
                $page = Page::query()->create([
                    'parent_id' => isset($data['parent']) ? $pages[$data['parent']]->id : null,
                    'type' => $data['type'],
                    'product' => $data['product'],
                    'icon' => $data['icon'],
                    'position' => $position,
                    'in_header' => $data['in_header'],
                    'footer_column' => $data['footer_column'],
                ]);
                foreach ($data['translations'] as $locale => $t) {
                    $translation = $page->translations()->make(['locale' => $locale]);
                    $translation->setRelation('page', $page);
                    $status = PublishStatus::from($t['status'] ?? 'published');
                    Paths::save($translation, [
                        'slug' => $t['slug'],
                        'title' => $t['title'],
                        'nav_title' => $t['nav_title'],
                        'excerpt' => $t['excerpt'],
                        'seo_title' => $t['seo_title'],
                        'seo_description' => $t['seo_description'],
                        'blocks' => Blocks::clean($this->withMedia($t['blocks'], $media)),
                        'status' => $status,
                        'published_at' => $status === PublishStatus::Published ? now() : null,
                    ]);
                }
                $pages[$data['key']] = $page->load('translations');
            }
            foreach ($seed['redirects'] as $redirect) {
                $target = $pages[$redirect['page']]->translations->firstWhere('locale', $redirect['locale']);
                Redirect::query()->updateOrCreate(
                    ['from_path' => $redirect['from']],
                    ['translation_id' => $target->id, 'status_code' => 301, 'automatic' => true],
                );
            }
        });

        $this->command?->info(Page::query()->count().' sayfa, '.Media::query()->count().' görsel, '.PageTranslation::query()->count().' çeviri, '.Redirect::query()->count().' yönlendirme eklendi.');
    }

    /**
     * Copies the seed's images to the media disk.
     *
     * @return array<string, int> media ids by key
     */
    private function media(array $files): array
    {
        $ids = [];
        $disk = config('cms.media_disk');
        foreach ($files as $file) {
            $source = __DIR__.'/'.$file['file'];
            if (! is_file($source)) {
                $this->command?->warn("Görsel yok, atlandı: {$file['file']}");

                continue;
            }
            $path = 'media/seed/'.Str::slug($file['key']).'.'.pathinfo($source, PATHINFO_EXTENSION);
            Storage::disk($disk)->put($path, (string) file_get_contents($source));
            $size = @getimagesize($source);
            $ids[$file['key']] = Media::query()->create([
                'disk' => $disk,
                'path' => $path,
                'filename' => basename($source),
                'mime' => $size['mime'] ?? 'image/jpeg',
                'size' => filesize($source),
                'width' => $size[0] ?? null,
                'height' => $size[1] ?? null,
                'alt' => $file['alt'] ?? null,
            ])->id;
        }

        return $ids;
    }

    /** Replaces `@media:<key>` values with media ids and drops image blocks whose file is missing. */
    private function withMedia(array $blocks, array $media): array
    {
        $resolve = function (mixed $value) use (&$resolve, $media): mixed {
            if (is_string($value) && str_starts_with($value, '@media:')) {
                return $media[substr($value, 7)] ?? null;
            }

            return is_array($value) ? array_map($resolve, $value) : $value;
        };
        $blocks = array_map($resolve, $blocks);

        return array_values(array_filter($blocks, fn (array $block) => $block['type'] !== 'image' || ! empty($block['data']['image'])));
    }
}
