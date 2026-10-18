{{-- Language versions of the page, then its shared settings. --}}
<nav class="cms-body mb-6 oc-tabs" aria-label="Diller">
    @foreach ($languages as $tabLanguage)
        @php($tabTranslation = $page->translations->firstWhere('locale', $tabLanguage->code))
        <a href="{{ route('admin.translations.edit', [$page, $tabLanguage]) }}"
           @class(['oc-tab no-underline', 'active' => $active === $tabLanguage->code])
           @if ($active === $tabLanguage->code) aria-current="page" @endif>
            <span @class(['lang-pill', 'published' => $tabTranslation?->isLive(), 'draft' => $tabTranslation && ! $tabTranslation->isLive(), 'missing' => ! $tabTranslation])>{{ $tabLanguage->code }}</span>
            {{ $tabLanguage->native_name }}
            @unless ($tabTranslation)<x-icon name="plus" :size="14" />@endunless
        </a>
    @endforeach
    <a href="{{ route('admin.pages.edit', $page) }}" @class(['oc-tab no-underline ml-auto', 'active' => $active === 'settings'])>
        <x-icon name="settings" :size="15" /> Sayfa ayarları
    </a>
</nav>
