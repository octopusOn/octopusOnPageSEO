<x-layouts.admin title="Yönlendirmeler">
    <x-slot:head>
        <x-pagehead title="Yönlendirmeler" lead="Eski adresler. Bir sayfanın adresi değiştiğinde 301 kendiliğinden eklenir ve sayfa nereye taşınırsa onu izler; taşınan siteden kalan adresleri burada elle ekleyin." />
    </x-slot:head>
    <div class="cms-body flex flex-col gap-6">
        <form method="post" action="{{ route('admin.redirects.store') }}" class="oc-panel grid items-end gap-4 md:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_140px_auto]">
            @csrf
            <x-field name="from_path" label="Eski adres" hint="Sitedeki yol: /tr/eski-sayfa ya da /eski-blog/yazi">
                <input class="oc-input font-mono" name="from_path" value="{{ old('from_path') }}" required placeholder="/tr/eski-adres">
            </x-field>
            <x-field name="target" label="Yeni adres" hint="/tr/yeni-adres ya da https://…">
                <input class="oc-input font-mono" name="target" value="{{ old('target') }}" required placeholder="/tr/yeni-adres">
            </x-field>
            <x-field name="status_code" label="Kod">
                <select class="oc-select" name="status_code">
                    @foreach ([301 => '301 kalıcı', 302 => '302 geçici', 307 => '307 geçici', 308 => '308 kalıcı'] as $code => $label)
                        <option value="{{ $code }}" @selected((int) old('status_code', 301) === $code)>{{ $label }}</option>
                    @endforeach
                </select>
            </x-field>
            <button type="submit" class="oc-btn primary mb-[2px]"><x-icon name="plus" :size="16" /> Ekle</button>
        </form>

        <form method="get" class="flex">
            <label class="oc-search flex-1"><x-icon name="search" :size="16" /><input type="search" name="q" value="{{ $query }}" placeholder="Adres ara" aria-label="Ara"></label>
        </form>

        <section class="oc-panel p-0">
            <div class="oc-table-wrap">
                <table class="oc-table">
                    <thead><tr><th>Eski adres</th><th>Hedef</th><th>Kod</th><th>Tür</th><th class="num">İstek</th><th></th></tr></thead>
                    <tbody>
                    @forelse ($redirects as $redirect)
                        @php($to = $redirect->destination(true))
                        <tr>
                            <td class="font-mono text-xs">{{ $redirect->from_path }}</td>
                            <td class="font-mono text-xs">
                                @if ($redirect->translation)
                                    <a href="{{ route('admin.translations.edit', [$redirect->translation->page_id, $redirect->translation->locale]) }}">{{ $to }}</a>
                                @else
                                    {{ $to ?? '—' }}
                                @endif
                            </td>
                            <td>{{ $redirect->status_code }}</td>
                            <td><span class="oc-badge {{ $redirect->automatic ? 'info' : '' }}">{{ $redirect->automatic ? 'Otomatik' : 'Elle' }}</span></td>
                            <td class="num">{{ $redirect->hits }}</td>
                            <td class="text-right">
                                <form method="post" action="{{ route('admin.redirects.destroy', $redirect) }}" class="inline" data-confirm="{{ $redirect->from_path }} yönlendirmesi silinsin mi?">
                                    @csrf @method('delete')
                                    <button class="oc-btn icon small danger" aria-label="Sil"><x-icon name="trash-2" :size="14" /></button>
                                </form>
                            </td>
                        </tr>
                    @empty
                        <tr><td colspan="6" class="text-ink-muted">Yönlendirme yok.</td></tr>
                    @endforelse
                    </tbody>
                </table>
            </div>
        </section>
        {{ $redirects->links('partials.pagination') }}
    </div>
</x-layouts.admin>
