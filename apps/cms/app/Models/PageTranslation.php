<?php

namespace App\Models;

use App\Enums\PublishStatus;
use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;
use Illuminate\Support\Carbon;

/**
 * A page in one language: its slug and full path, texts, content blocks and SEO fields.
 *
 * @property int $id
 * @property int $page_id
 * @property string $locale
 * @property string $slug
 * @property string $path
 * @property string $title
 * @property string|null $nav_title
 * @property string|null $excerpt
 * @property array $blocks
 * @property string|null $seo_title
 * @property string|null $seo_description
 * @property string|null $canonical_url
 * @property bool $noindex
 * @property int|null $image_id
 * @property PublishStatus $status
 * @property Carbon|null $published_at
 * @property-read Page $page
 * @property-read Media|null $image
 * @property-read Language $language
 */
class PageTranslation extends Model
{
    protected $fillable = [
        'locale', 'slug', 'path', 'title', 'nav_title', 'excerpt', 'blocks', 'seo_title', 'seo_description',
        'canonical_url', 'noindex', 'image_id', 'status', 'published_at', 'updated_by',
    ];

    protected $attributes = ['blocks' => '[]'];

    protected function casts(): array
    {
        return [
            'blocks' => 'array',
            'noindex' => 'boolean',
            'status' => PublishStatus::class,
            'published_at' => 'datetime',
        ];
    }

    public function page(): BelongsTo
    {
        return $this->belongsTo(Page::class);
    }

    public function language(): BelongsTo
    {
        return $this->belongsTo(Language::class, 'locale', 'code');
    }

    public function image(): BelongsTo
    {
        return $this->belongsTo(Media::class, 'image_id');
    }

    public function editor(): BelongsTo
    {
        return $this->belongsTo(User::class, 'updated_by');
    }

    public function redirects(): HasMany
    {
        return $this->hasMany(Redirect::class, 'translation_id');
    }

    /** Published, with a publication time that has come. */
    public function scopeLive(Builder $query): void
    {
        $query->where('status', PublishStatus::Published)
            ->where(fn (Builder $q) => $q->whereNull('published_at')->orWhere('published_at', '<=', now()));
    }

    public function isLive(): bool
    {
        return $this->status === PublishStatus::Published && (! $this->published_at || $this->published_at->isPast());
    }

    /** Site-relative URL: `/tr`, `/tr/blog/log-analizi`. */
    public function url(): string
    {
        return self::urlFor($this->locale, $this->path);
    }

    public static function urlFor(string $locale, string $path): string
    {
        return '/'.$locale.($path === '' ? '' : '/'.$path);
    }

    /** Menu and card label. */
    public function label(): string
    {
        return $this->nav_title ?: $this->title;
    }
}
