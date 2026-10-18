<?php

namespace Database\Seeders;

use App\Cms\Paths;
use App\Enums\PageType;
use App\Enums\PublishStatus;
use App\Models\Language;
use App\Models\Page;
use Illuminate\Database\Seeder;
use Illuminate\Support\Facades\DB;
use Illuminate\Support\Str;

/**
 * The account site's legal texts (legal/<key>.<language>.md; slugs and short labels in legal/index.json) as
 * `legal` pages in every language the site has. A text whose key already exists is skipped, so
 * edits made in the admin are never overwritten. Runs from its migration on existing sites and
 * from DatabaseSeeder on new ones, where the languages arrive with the website's content.
 */
class LegalSeeder extends Seeder
{
    public function run(): void
    {
        $languages = Language::query()->pluck('code')->all();
        if ($languages === []) {
            return;
        }
        $index = json_decode((string) file_get_contents(__DIR__.'/legal/index.json'), true, flags: JSON_THROW_ON_ERROR);

        DB::transaction(function () use ($index, $languages) {
            $position = (int) Page::query()->whereNull('parent_id')->max('position');
            foreach ($index as $document) {
                if (Page::query()->where('key', $document['key'])->exists()) {
                    continue;
                }
                $page = Page::query()->create([
                    'type' => PageType::Legal,
                    'key' => $document['key'],
                    'position' => ++$position,
                    'in_header' => false,
                ]);
                foreach ($document['slugs'] as $locale => $slug) {
                    $file = __DIR__."/legal/{$document['key']}.{$locale}.md";
                    if (! in_array($locale, $languages, true) || ! is_file($file)) {
                        continue;
                    }
                    [$title, $body] = self::split((string) file_get_contents($file));
                    $translation = $page->translations()->make(['locale' => $locale]);
                    $translation->setRelation('page', $page);
                    Paths::save($translation, [
                        'slug' => $slug,
                        'title' => $title,
                        'nav_title' => $document['labels'][$locale] ?? null,
                        'blocks' => [['id' => 'b'.Str::lower(Str::random(10)), 'type' => 'rich_text', 'data' => ['body' => $body]]],
                        'noindex' => true,
                        'status' => PublishStatus::Published,
                        'published_at' => now(),
                    ]);
                }
            }
        });
    }

    /** The `# Title` line and the Markdown after it. */
    private static function split(string $markdown): array
    {
        $markdown = trim(str_replace("\r\n", "\n", $markdown));
        [$first, $rest] = array_pad(explode("\n", $markdown, 2), 2, '');

        return [trim(ltrim($first, '# ')), trim($rest)];
    }
}
