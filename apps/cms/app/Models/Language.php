<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Builder;
use Illuminate\Database\Eloquent\Collection;
use Illuminate\Database\Eloquent\Model;
use Illuminate\Database\Eloquent\Relations\HasMany;

/**
 * A website language. Its code is the URL prefix (`/tr/…`) and the hreflang value.
 *
 * @property int $id
 * @property string $code
 * @property string $name
 * @property string $native_name
 * @property string $locale_tag
 * @property bool $is_default
 * @property bool $is_active
 * @property int $position
 */
class Language extends Model
{
    protected $fillable = ['code', 'name', 'native_name', 'locale_tag', 'is_default', 'is_active', 'position'];

    protected function casts(): array
    {
        return ['is_default' => 'boolean', 'is_active' => 'boolean', 'position' => 'integer'];
    }

    public function getRouteKeyName(): string
    {
        return 'code';
    }

    public function translations(): HasMany
    {
        return $this->hasMany(PageTranslation::class, 'locale', 'code');
    }

    public function scopeActive(Builder $query): void
    {
        $query->where('is_active', true);
    }

    public function scopeOrdered(Builder $query): void
    {
        $query->orderByDesc('is_default')->orderBy('position')->orderBy('code');
    }

    /** All languages, the default first. */
    public static function ordered(): Collection
    {
        return static::query()->ordered()->get();
    }

    public static function default(): ?self
    {
        return static::query()->where('is_default', true)->first() ?? static::query()->ordered()->first();
    }

    public static function defaultCode(): string
    {
        return static::default()?->code ?? 'en';
    }
}
