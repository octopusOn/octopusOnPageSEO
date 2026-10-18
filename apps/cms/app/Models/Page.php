<?php

namespace App\Models;

use App\Enums\PageType;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\BelongsTo;
use Illuminate\Database\Eloquent\Relations\HasMany;

/**
 * A node of the site tree: a page, category, product page, post or doc. Language-independent;
 * its texts are in `translations`, one per language.
 *
 * @property int $id
 * @property int|null $parent_id
 * @property PageType $type
 * @property string|null $key
 * @property string|null $product
 * @property string|null $icon
 * @property int $position
 * @property bool $in_header
 * @property string|null $footer_column
 * @property-read Page|null $parent
 * @property-read Collection<int, Page> $children
 * @property-read Collection<int, PageTranslation> $translations
 */
class Page extends Model
{
    protected $fillable = ['parent_id', 'type', 'key', 'product', 'icon', 'position', 'in_header', 'footer_column'];

    protected function casts(): array
    {
        return [
            'type' => PageType::class,
            'position' => 'integer',
            'in_header' => 'boolean',
        ];
    }

    public function parent(): BelongsTo
    {
        return $this->belongsTo(self::class, 'parent_id');
    }

    public function children(): HasMany
    {
        return $this->hasMany(self::class, 'parent_id')->orderBy('position')->orderBy('id');
    }

    public function translations(): HasMany
    {
        return $this->hasMany(PageTranslation::class);
    }

    public function translation(string $locale): ?PageTranslation
    {
        return $this->relationLoaded('translations')
            ? $this->translations->firstWhere('locale', $locale)
            : $this->translations()->where('locale', $locale)->first();
    }

    /** The translation in `$locale`, else the default language's, else any. */
    public function preferredTranslation(?string $locale = null): ?PageTranslation
    {
        $locale ??= Language::defaultCode();

        return $this->translation($locale)
            ?? $this->translation(Language::defaultCode())
            ?? $this->translations->first();
    }

    /** Title for admin lists, in the admin's content language. */
    public function adminTitle(?string $locale = null): string
    {
        return $this->preferredTranslation($locale)?->title ?? "#{$this->id}";
    }

    /** Ids of this page and every page below it. */
    public function descendantIds(): array
    {
        $ids = [$this->id];
        $level = [$this->id];
        while ($level) {
            $level = self::query()->whereIn('parent_id', $level)->pluck('id')->all();
            $ids = [...$ids, ...$level];
        }

        return $ids;
    }

    /** Parents from the root down to (excluding) this page. */
    public function ancestors(): array
    {
        $chain = [];
        $node = $this->parent;
        while ($node && count($chain) < 32) {
            array_unshift($chain, $node);
            $node = $node->parent;
        }

        return $chain;
    }
}
