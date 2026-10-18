<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;

/**
 * An old address that sends visitors on. Automatic redirects are made when a slug or parent
 * changes and point at the translation itself, so they never chain; manual ones name a target.
 *
 * @property int $id
 * @property string $from_path
 * @property int|null $translation_id
 * @property string|null $target
 * @property int $status_code
 * @property bool $automatic
 * @property int $hits
 */
class Redirect extends Model
{
    protected $fillable = ['from_path', 'translation_id', 'target', 'status_code', 'automatic'];

    protected function casts(): array
    {
        return [
            'status_code' => 'integer',
            'automatic' => 'boolean',
            'hits' => 'integer',
            'last_hit_at' => 'datetime',
        ];
    }

    public function translation(): BelongsTo
    {
        return $this->belongsTo(PageTranslation::class, 'translation_id');
    }

    /** Where the redirect leads now, or null when its translation is gone or unpublished. */
    public function destination(bool $drafts = false): ?string
    {
        if ($this->translation_id) {
            $translation = $this->translation;

            return $translation && ($drafts || $translation->isLive()) ? $translation->url() : null;
        }

        return $this->target;
    }

    /** `/tr/features/` and `tr/features` become `/tr/features`. */
    public static function normalize(string $path): string
    {
        $path = '/'.trim(parse_url($path, PHP_URL_PATH) ?: '', '/');

        return $path === '/' ? '/' : $path;
    }
}
