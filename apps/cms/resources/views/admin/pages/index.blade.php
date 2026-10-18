<x-layouts.admin title="Sayfalar">
    <x-slot:head>
        <x-pagehead title="Sayfalar" lead="Sitenin ağacı. Kategoriler, ürünler, yazılar ve dokümanlar da birer sayfadır; dil rozetine tıklayarak o dildeki sürümü açın.">
            <x-slot:actions>
                <a href="{{ route('admin.pages.create') }}" class="oc-btn primary"><x-icon name="plus" :size="16" /> Yeni sayfa</a>
            </x-slot:actions>
        </x-pagehead>
    </x-slot:head>

    <div class="cms-body flex flex-col gap-4">
        <form method="get" class="flex flex-wrap items-center gap-3">
            <label class="oc-search flex-1">
                <x-icon name="search" :size="16" />
                <input type="search" name="q" value="{{ $query }}" placeholder="Başlık ya da adres ara" aria-label="Ara">
            </label>
            <div class="oc-segmented" role="group" aria-label="Tür">
                <a href="{{ request()->fullUrlWithQuery(['type' => null]) }}" @class(['active' => ! $type])>Tümü</a>
                @foreach (\App\Enums\PageType::cases() as $case)
                    <a href="{{ request()->fullUrlWithQuery(['type' => $case->value]) }}" @class(['active' => $type === $case])>{{ $case->label() }}</a>
                @endforeach
            </div>
        </form>

        <section class="oc-panel p-0">
            <div class="oc-table-wrap">
                <table class="oc-table">
                    <thead>
                    <tr><th>Başlık</th><th>Diller</th><th>Menü</th><th>Adres ({{ strtoupper($contentLocale) }})</th><th class="num">Sıra</th><th></th></tr>
                    </thead>
                    <tbody>
                    @forelse ($rows as $row)
                        @php($page = $row['page'])
                        @php($t = $page->preferredTranslation($contentLocale))
                        <tr>
                            <td>
                                <div class="tree-title" style="--depth: {{ $row['depth'] }}">
                                    @if ($row['depth'])<span class="text-ink-faint"><x-icon name="corner-down-right" :size="14" /></span>@endif
                                    @if ($page->icon)<span class="text-ink-muted"><x-icon :name="$page->icon" :size="16" /></span>@endif
                                    <a href="{{ route('admin.translations.edit', [$page, $t?->locale ?? $contentLocale]) }}">{{ $page->adminTitle($contentLocale) }}</a>
                                    <span class="oc-badge {{ $page->type->badge() }}">{{ $page->type->label() }}</span>
                                    @if ($page->product)<span class="oc-badge product">{{ config('cms.products')[$page->product] ?? $page->product }}</span>@endif
                                </div>
                            </td>
                            <td>@include('admin.pages._langs', ['page' => $page])</td>
                            <td class="text-ink-muted">
                                {{ collect([$page->in_header ? 'Üst' : null, $page->footer_column ? 'Alt: '.(config('cms.footer_columns')[$page->footer_column] ?? '') : null])->filter()->implode(' · ') ?: '—' }}
                            </td>
                            <td class="max-w-72 truncate font-mono text-xs text-ink-muted">{{ $t?->url() }}</td>
                            <td class="num">
                                <form method="post" action="{{ route('admin.pages.move', $page) }}" class="inline-flex gap-1">
                                    @csrf
                                    <button class="oc-btn icon small ghost" name="direction" value="up" aria-label="Yukarı taşı"><x-icon name="arrow-up" :size="14" /></button>
                                    <button class="oc-btn icon small ghost" name="direction" value="down" aria-label="Aşağı taşı"><x-icon name="arrow-down" :size="14" /></button>
                                </form>
                            </td>
                            <td class="text-right">
                                <a href="{{ route('admin.pages.create', ['parent' => $page->id]) }}" class="oc-btn small ghost" title="Alt sayfa ekle"><x-icon name="plus" :size="14" /> Alt sayfa</a>
                                <a href="{{ route('admin.pages.edit', $page) }}" class="oc-btn icon small ghost" title="Sayfa ayarları" aria-label="Sayfa ayarları"><x-icon name="settings" :size="15" /></a>
                            </td>
                        </tr>
                    @empty
                        <tr><td colspan="6"><div class="oc-empty"><h2>Sayfa bulunamadı</h2><p>Aramayı değiştirin ya da yeni bir sayfa oluşturun.</p></div></td></tr>
                    @endforelse
                    </tbody>
                </table>
            </div>
        </section>
    </div>
</x-layouts.admin>
