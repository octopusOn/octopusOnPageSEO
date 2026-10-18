{{-- One pill per language: green published, amber draft, dashed missing (a click creates it). --}}
<div class="flex gap-1">
    @foreach ($languages as $language)
        @php($t = $page->translations->firstWhere('locale', $language->code))
        <a href="{{ route('admin.translations.edit', [$page, $language]) }}"
           @class(['lang-pill', 'published' => $t?->isLive(), 'draft' => $t && ! $t->isLive(), 'missing' => ! $t])
           title="{{ $language->native_name }}: {{ $t ? ($t->isLive() ? 'yayında' : 'taslak') : 'çeviri yok, oluşturmak için tıklayın' }}">{{ $language->code }}</a>
    @endforeach
</div>
