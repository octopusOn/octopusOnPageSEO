<?php

namespace App\Models;

use Illuminate\Database\Eloquent\Model;
use Illuminate\Support\Facades\Storage;

/**
 * An uploaded file (usually an image) that pages and blocks refer to by id.
 *
 * @property int $id
 * @property string $disk
 * @property string $path
 * @property string $filename
 * @property string $mime
 * @property int $size
 * @property int|null $width
 * @property int|null $height
 * @property string|null $alt
 */
class Media extends Model
{
    protected $fillable = ['disk', 'path', 'filename', 'mime', 'size', 'width', 'height', 'alt'];

    protected function casts(): array
    {
        return ['size' => 'integer', 'width' => 'integer', 'height' => 'integer'];
    }

    public function url(): string
    {
        return Storage::disk($this->disk)->url($this->path);
    }

    public function isImage(): bool
    {
        return str_starts_with($this->mime, 'image/');
    }

    /** @return array{id: int, url: string, alt: string|null, width: int|null, height: int|null, mime: string} */
    public function toApi(): array
    {
        return [
            'id' => $this->id,
            'url' => $this->url(),
            'alt' => $this->alt,
            'width' => $this->width,
            'height' => $this->height,
            'mime' => $this->mime,
        ];
    }

    /** Human-readable size: `48 KB`. */
    public function humanSize(): string
    {
        $units = ['B', 'KB', 'MB', 'GB'];
        $size = $this->size;
        $unit = 0;
        while ($size >= 1024 && $unit < count($units) - 1) {
            $size /= 1024;
            $unit++;
        }

        return round($size, $unit ? 1 : 0).' '.$units[$unit];
    }
}
