<?php

namespace Database\Seeders;

use App\Models\Media;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;

/** Seed images (media/) on the media disk, and `@media:<key>` values in blocks. */
trait SeedsMedia
{
    /**
     * Copies the seed's images to the media disk.
     *
     * @param  array<int, array{key: string, file: string, alt?: string}>  $files
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
