<x-layouts.admin title="Medya">
    <x-slot:head>
        <x-pagehead title="Medya" lead="Sayfalarda ve bloklarda kullanılan görseller ve belgeler. Kullanımdaki bir dosya silinemez.">
            <x-slot:actions>
                <form method="post" action="{{ route('admin.media.store') }}" enctype="multipart/form-data" x-data>
                    @csrf
                    <label class="oc-btn primary cursor-pointer">
                        <x-icon name="upload" :size="16" /> Dosya yükle
                        <input type="file" name="files[]" multiple accept="image/*,application/pdf" class="sr-only" @change="$el.form.submit()">
                    </label>
                </form>
            </x-slot:actions>
        </x-pagehead>
    </x-slot:head>

    <div class="cms-body flex flex-col gap-4">
        <form method="get" class="flex">
            <label class="oc-search flex-1">
                <x-icon name="search" :size="16" />
                <input type="search" name="q" value="{{ $query }}" placeholder="Dosya adı ya da açıklama" aria-label="Ara">
            </label>
        </form>
        <p class="text-sm text-ink-faint">İzin verilen türler: {{ str_replace(',', ', ', $types) }} · en fazla 10 MB. SVG güvenlik nedeniyle kabul edilmez.</p>

        @if ($media->isEmpty())
            <div class="oc-empty oc-dropzone"><h2>Henüz dosya yok</h2><p>Sağ üstteki düğmeyle görsel ya da PDF yükleyin.</p></div>
        @else
            <div class="grid grid-cols-2 gap-4 md:grid-cols-3 2xl:grid-cols-5">
                @foreach ($media as $file)
                    <article class="oc-card gap-3 p-3">
                        <a href="{{ $file->url() }}" target="_blank" rel="noopener" class="media-thumb">
                            @if ($file->isImage())
                                <img src="{{ $file->url() }}" alt="{{ $file->alt }}" loading="lazy">
                            @else
                                <span class="text-sm text-ink-muted">{{ strtoupper(pathinfo($file->path, PATHINFO_EXTENSION)) }}</span>
                            @endif
                        </a>
                        <div class="min-w-0">
                            <div class="truncate text-sm font-semibold" title="{{ $file->filename }}">{{ $file->filename }}</div>
                            <div class="text-xs text-ink-faint">{{ $file->humanSize() }}@if ($file->width) · {{ $file->width }}×{{ $file->height }}@endif</div>
                        </div>
                        <form method="post" action="{{ route('admin.media.update', $file) }}" class="flex gap-2">
                            @csrf @method('put')
                            <input class="oc-input h-8 text-sm" name="alt" value="{{ $file->alt }}" placeholder="Alternatif metin" aria-label="Alternatif metin" maxlength="255">
                            <button class="oc-btn icon small" aria-label="Kaydet"><x-icon name="check" :size="14" /></button>
                        </form>
                        <div class="flex gap-2">
                            <button type="button" class="oc-btn small flex-1" data-copy="{{ $file->url() }}"><x-icon name="copy" :size="14" /> <span>Adresi kopyala</span></button>
                            <form method="post" action="{{ route('admin.media.destroy', $file) }}" data-confirm="{{ $file->filename }} silinsin mi?">
                                @csrf @method('delete')
                                <button class="oc-btn icon small danger" aria-label="Sil"><x-icon name="trash-2" :size="14" /></button>
                            </form>
                        </div>
                    </article>
                @endforeach
            </div>
            {{ $media->links('partials.pagination') }}
        @endif
    </div>
</x-layouts.admin>
