<x-layouts.admin title="Panel">
    <x-slot:head>
        <x-pagehead title="Panel" lead="Web sitesinin sayfaları, dilleri ve yayın durumu.">
            <x-slot:actions>
                <a href="{{ route('admin.pages.create') }}" class="oc-btn primary"><x-icon name="plus" :size="16" /> Yeni sayfa</a>
            </x-slot:actions>
        </x-pagehead>
    </x-slot:head>

    <div class="cms-body flex flex-col gap-6">
        @if ($revalidateConfigured && $revalidate && ! $revalidate['ok'])
            <div class="oc-banner warning" role="status">
                <x-icon name="circle-alert" />
                <div>Site son değişiklikten sonra yenilenemedi ({{ $revalidate['error'] }}). Sayfalar en geç bir saat içinde kendiliğinden güncellenir; CMS_REVALIDATE_URL ve CMS_REVALIDATE_SECRET ayarlarını kontrol edin.</div>
            </div>
        @elseif (! $revalidateConfigured)
            <div class="oc-banner info" role="status">
                <x-icon name="refresh-cw" />
                <div>Anında yenileme kapalı: CMS_REVALIDATE_URL tanımlı değil, değişiklikler sitede en geç bir saat sonra görünür.</div>
            </div>
        @endif

        <div class="oc-stats">
            <div class="oc-stat"><span class="oc-stat-label">Sayfalar</span><span class="oc-stat-value">{{ $pages }}</span><span class="oc-stat-hint">kategoriler ve ürünler dahil</span></div>
            <div class="oc-stat good"><span class="oc-stat-label">Yayında</span><span class="oc-stat-value">{{ $live }}</span><span class="oc-stat-hint">dil sürümü</span></div>
            <div class="oc-stat"><span class="oc-stat-label">Taslak</span><span class="oc-stat-value">{{ $drafts }}</span><span class="oc-stat-hint">dil sürümü</span></div>
            <div @class(['oc-stat', 'critical' => $missing > 0])><span class="oc-stat-label">Eksik çeviri</span><span class="oc-stat-value">{{ $missing }}</span><span class="oc-stat-hint">etkin dillerde</span></div>
        </div>

        <div class="grid gap-6 xl:grid-cols-[minmax(0,2fr)_minmax(0,3fr)]">
            <section class="oc-panel">
                <div class="oc-panel-head"><h2>Diller</h2><a href="{{ route('admin.languages.index') }}" class="oc-btn small ghost">Yönet</a></div>
                <div class="flex flex-col gap-4">
                    @foreach ($coverage as $row)
                        @php($total = max(1, $pages))
                        <div>
                            <div class="mb-2 flex items-center gap-2">
                                <span class="lang-pill {{ $row['language']->is_active ? 'published' : '' }}">{{ $row['language']->code }}</span>
                                <span class="flex-1 font-semibold">{{ $row['language']->native_name }}</span>
                                <span class="text-sm text-ink-muted">{{ $row['published'] }} yayında · {{ $row['drafts'] }} taslak · {{ $row['missing'] }} eksik</span>
                            </div>
                            <div class="oc-progress"><div style="width: {{ round($row['published'] / $total * 100) }}%"></div></div>
                        </div>
                    @endforeach
                </div>
            </section>
            <section class="oc-panel">
                <div class="oc-panel-head"><h2>Son değişiklikler</h2></div>
                <div class="oc-table-wrap">
                    <table class="oc-table">
                        <thead><tr><th>Sayfa</th><th>Dil</th><th>Durum</th><th>Değiştiren</th><th class="num">Zaman</th></tr></thead>
                        <tbody>
                        @forelse ($recent as $translation)
                            <tr>
                                <td><a href="{{ route('admin.translations.edit', [$translation->page_id, $translation->locale]) }}" class="font-semibold text-ink no-underline hover:underline">{{ $translation->title }}</a></td>
                                <td><span class="lang-pill">{{ $translation->locale }}</span></td>
                                <td>@include('admin.pages._status', ['translation' => $translation])</td>
                                <td class="text-ink-muted">{{ $translation->editor?->name ?? '—' }}</td>
                                <td class="num text-ink-muted" title="{{ $translation->updated_at }}">{{ $translation->updated_at?->diffForHumans() }}</td>
                            </tr>
                        @empty
                            <tr><td colspan="5" class="text-ink-muted">Henüz içerik yok.</td></tr>
                        @endforelse
                        </tbody>
                    </table>
                </div>
            </section>
        </div>
    </div>
</x-layouts.admin>
