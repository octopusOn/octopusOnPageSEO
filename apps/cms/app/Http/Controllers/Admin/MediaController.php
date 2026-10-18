<?php

namespace App\Http\Controllers\Admin;

use App\Cms\Blocks;
use App\Http\Controllers\Controller;
use App\Models\Media;
use App\Models\PageTranslation;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Http\UploadedFile;
use Illuminate\Support\Facades\Storage;
use Illuminate\Support\Str;
use Illuminate\View\View;

/** Uploaded images and documents. SVG is refused: it can carry scripts. */
class MediaController extends Controller
{
    public const TYPES = 'jpg,jpeg,png,webp,avif,gif,pdf';

    public function index(Request $request): View|JsonResponse
    {
        $query = Media::query()->latest()
            ->when($request->query('q'), fn ($q, $term) => $q->where(fn ($q) => $q->whereLike('filename', "%{$term}%")->orWhereLike('alt', "%{$term}%")));
        if ($request->wantsJson()) {
            $page = $query->paginate(24);

            return response()->json([
                'data' => collect($page->items())->map(fn (Media $m) => self::json($m))->values(),
                'current_page' => $page->currentPage(),
                'last_page' => $page->lastPage(),
            ]);
        }

        return view('admin.media.index', [
            'media' => $query->paginate(36)->withQueryString(),
            'query' => $request->query('q'),
            'types' => self::TYPES,
        ]);
    }

    public function store(Request $request): RedirectResponse|JsonResponse
    {
        $request->validate([
            'files' => ['required', 'array', 'max:20'],
            'files.*' => ['file', 'max:10240', 'mimes:'.self::TYPES],
        ], [
            'files.*.mimes' => 'Yalnızca şu dosyalar yüklenebilir: '.str_replace(',', ', ', self::TYPES).'.',
            'files.*.max' => 'Bir dosya en fazla 10 MB olabilir.',
        ]);
        $created = collect($request->file('files'))->map(fn (UploadedFile $file) => self::save($file));
        if ($request->wantsJson()) {
            return response()->json(['data' => $created->map(fn (Media $m) => self::json($m))->values()], 201);
        }

        return back()->with('status', $created->count().' dosya yüklendi.');
    }

    public function update(Request $request, Media $media): RedirectResponse
    {
        $data = $request->validate(['alt' => ['nullable', 'string', 'max:255']]);
        $media->update(['alt' => $data['alt'] ?? null]);

        return back()->with('status', 'Açıklama kaydedildi.');
    }

    public function destroy(Media $media): RedirectResponse
    {
        $users = self::usedBy($media);
        if ($users) {
            return back()->with('error', "“{$media->filename}” şu sayfalarda kullanılıyor: ".implode(', ', $users).'. Önce oradan kaldırın.');
        }
        Storage::disk($media->disk)->delete($media->path);
        $media->delete();

        return back()->with('status', "“{$media->filename}” silindi.");
    }

    public static function save(UploadedFile $file): Media
    {
        $disk = config('cms.media_disk');
        $extension = strtolower($file->guessExtension() ?: $file->getClientOriginalExtension());
        $name = Str::slug(pathinfo($file->getClientOriginalName(), PATHINFO_FILENAME)) ?: 'file';
        $path = $file->storeAs('media/'.now()->format('Y/m'), Str::lower(Str::random(8)).'-'.Str::limit($name, 60, '').'.'.$extension, $disk);
        $size = str_starts_with((string) $file->getMimeType(), 'image/') ? @getimagesize($file->getRealPath()) : false;

        return Media::query()->create([
            'disk' => $disk,
            'path' => $path,
            'filename' => $file->getClientOriginalName(),
            'mime' => (string) $file->getMimeType(),
            'size' => $file->getSize(),
            'width' => $size[0] ?? null,
            'height' => $size[1] ?? null,
        ]);
    }

    /** Titles of the translations that show the file. */
    public static function usedBy(Media $media): array
    {
        return PageTranslation::query()->get(['id', 'locale', 'title', 'blocks', 'image_id'])
            ->filter(fn (PageTranslation $t) => $t->image_id === $media->id || in_array($media->id, Blocks::mediaIds($t->blocks ?? []), true))
            ->map(fn (PageTranslation $t) => "{$t->title} (".strtoupper($t->locale).')')
            ->values()->all();
    }

    private static function json(Media $media): array
    {
        return $media->toApi() + ['filename' => $media->filename, 'size' => $media->humanSize()];
    }
}
