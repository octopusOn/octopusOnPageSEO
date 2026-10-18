@php($title = $page->adminTitle($contentLocale))
<x-layouts.admin :title="$title.' · Ayarlar'">
    <x-slot:head>
        <x-pagehead :title="$title" lead="Sayfanın tüm dillerde ortak ayarları.">
            <x-slot:eyebrow>
                <a href="{{ route('admin.pages.index') }}" class="text-ink-faint">Sayfalar</a> <x-icon name="chevron-right" :size="14" />
                <span class="oc-badge {{ $page->type->badge() }}">{{ $page->type->label() }}</span>
            </x-slot:eyebrow>
        </x-pagehead>
        @include('admin.translations._tabs', ['active' => 'settings'])
    </x-slot:head>

    <div class="cms-body grid max-w-5xl gap-6">
        <form method="post" action="{{ route('admin.pages.update', $page) }}" class="oc-panel"
              x-data="{ type: @js(old('type', $page->type->value)), descriptions: @js(collect(\App\Enums\PageType::cases())->mapWithKeys(fn ($c) => [$c->value => $c->description()])) }">
            @csrf @method('put')
            <div class="oc-panel-head"><h2>Ayarlar</h2></div>
            @include('admin.pages._settings')
            <div class="mt-5 flex gap-3"><button type="submit" class="oc-btn primary"><x-icon name="save" :size="16" /> Kaydet</button></div>
        </form>

        <section class="oc-panel">
            <div class="oc-panel-head"><h2>Diller</h2></div>
            <div class="oc-table-wrap">
                <table class="oc-table">
                    <thead><tr><th>Dil</th><th>Başlık</th><th>Adres</th><th>Durum</th><th>Son değişiklik</th><th></th></tr></thead>
                    <tbody>
                    @foreach ($languages as $language)
                        @php($t = $page->translations->firstWhere('locale', $language->code))
                        <tr>
                            <td><span class="lang-pill">{{ $language->code }}</span> {{ $language->native_name }}</td>
                            <td>{{ $t?->title ?? '—' }}</td>
                            <td class="font-mono text-xs text-ink-muted">{{ $t?->url() ?? '—' }}</td>
                            <td>@if ($t) @include('admin.pages._status', ['translation' => $t]) @else <span class="oc-badge">Yok</span> @endif</td>
                            <td class="text-ink-muted">{{ $t ? $t->updated_at?->diffForHumans().($t->editor ? ' · '.$t->editor->name : '') : '' }}</td>
                            <td class="text-right">
                                <a href="{{ route('admin.translations.edit', [$page, $language]) }}" class="oc-btn small">{{ $t ? 'Düzenle' : 'Çeviri oluştur' }}</a>
                                @if ($t && $page->translations->count() > 1)
                                    <form method="post" action="{{ route('admin.translations.destroy', [$page, $language]) }}" class="inline" data-confirm="{{ $language->native_name }} çevirisi silinsin mi?">
                                        @csrf @method('delete')
                                        <button class="oc-btn icon small danger" aria-label="Çeviriyi sil"><x-icon name="trash-2" :size="14" /></button>
                                    </form>
                                @endif
                            </td>
                        </tr>
                    @endforeach
                    </tbody>
                </table>
            </div>
        </section>

        @if ($page->children->isNotEmpty())
            <section class="oc-panel">
                <div class="oc-panel-head"><h2>Alt sayfalar</h2><a href="{{ route('admin.pages.create', ['parent' => $page->id]) }}" class="oc-btn small"><x-icon name="plus" :size="14" /> Ekle</a></div>
                <ul class="flex flex-col gap-2">
                    @foreach ($page->children as $child)
                        <li class="flex items-center gap-3">
                            <span class="oc-badge {{ $child->type->badge() }}">{{ $child->type->label() }}</span>
                            <a href="{{ route('admin.pages.edit', $child) }}" class="text-ink">{{ $child->adminTitle($contentLocale) }}</a>
                        </li>
                    @endforeach
                </ul>
            </section>
        @endif

        <section class="oc-panel border-critical/30">
            <div class="oc-panel-head"><h2>Sayfayı sil</h2></div>
            <p class="mb-4 text-ink-muted">Sayfa tüm dillerdeki çevirileri ve otomatik yönlendirmeleriyle birlikte silinir. Alt sayfası olan bir sayfa silinemez.</p>
            <form method="post" action="{{ route('admin.pages.destroy', $page) }}" data-confirm="“{{ $title }}” tüm dillerde silinsin mi? Bu geri alınamaz.">
                @csrf @method('delete')
                <button class="oc-btn danger" @disabled($page->children->isNotEmpty())><x-icon name="trash-2" :size="16" /> Sayfayı sil</button>
            </form>
        </section>
    </div>
</x-layouts.admin>
