<x-layouts.admin title="Diller">
    <x-slot:head>
        <x-pagehead title="Diller" lead="Sitenin dilleri. Kod adresin ön ekidir (/tr/…) ve hreflang değeridir; pasif bir dil sitede görünmez ama çevirileri korunur.">
            <x-slot:actions><a href="{{ route('admin.languages.create') }}" class="oc-btn primary"><x-icon name="plus" :size="16" /> Dil ekle</a></x-slot:actions>
        </x-pagehead>
    </x-slot:head>
    <div class="cms-body">
        <section class="oc-panel p-0">
            <div class="oc-table-wrap">
                <table class="oc-table">
                    <thead><tr><th>Kod</th><th>Dil</th><th>Yerel ayar</th><th>Durum</th><th class="num">Çeviri</th><th class="num">Sıra</th><th></th></tr></thead>
                    <tbody>
                    @foreach ($languages as $language)
                        <tr>
                            <td><span class="lang-pill {{ $language->is_active ? 'published' : '' }}">{{ $language->code }}</span></td>
                            <td><b>{{ $language->native_name }}</b> <span class="text-ink-muted">{{ $language->name }}</span></td>
                            <td class="font-mono text-xs">{{ $language->locale_tag }}</td>
                            <td>
                                @if ($language->is_default)<span class="oc-badge accent">Varsayılan</span>@endif
                                <span class="oc-badge {{ $language->is_active ? 'good' : '' }}">{{ $language->is_active ? 'Etkin' : 'Pasif' }}</span>
                            </td>
                            <td class="num">{{ $language->translations_count }}</td>
                            <td class="num">{{ $language->position }}</td>
                            <td class="text-right">
                                <a href="{{ route('admin.languages.edit', $language) }}" class="oc-btn icon small ghost" aria-label="Düzenle"><x-icon name="pencil" :size="14" /></a>
                                @unless ($language->is_default || $language->translations_count)
                                    <form method="post" action="{{ route('admin.languages.destroy', $language) }}" class="inline" data-confirm="{{ $language->native_name }} silinsin mi?">
                                        @csrf @method('delete')
                                        <button class="oc-btn icon small danger" aria-label="Sil"><x-icon name="trash-2" :size="14" /></button>
                                    </form>
                                @endunless
                            </td>
                        </tr>
                    @endforeach
                    </tbody>
                </table>
            </div>
        </section>
    </div>
</x-layouts.admin>
