@php
    $isHome = $page->type === \App\Enums\PageType::Home;
    $exists = $translation->exists;
    $status = old('status', $translation->status?->value ?? 'draft');
    $publishedAt = old('published_at', $translation->published_at?->format('Y-m-d\TH:i'));
    $pageTitle = $exists ? $translation->title : $page->adminTitle();
@endphp
<x-layouts.admin :title="$pageTitle.' · '.strtoupper($language->code)">
    <x-slot:head>
        <x-pagehead :title="$pageTitle">
            <x-slot:eyebrow>
                <a href="{{ route('admin.pages.index') }}" class="text-ink-faint">Sayfalar</a> <x-icon name="chevron-right" :size="14" />
                <span class="oc-badge {{ $page->type->badge() }}">{{ $page->type->label() }}</span>
                @if ($page->product)<span class="oc-badge product">{{ config('cms.products')[$page->product] ?? $page->product }}</span>@endif
                @if ($exists) @include('admin.pages._status', ['translation' => $translation]) @else <span class="oc-badge">Yeni çeviri</span> @endif
            </x-slot:eyebrow>
            <x-slot:actions>
                @if ($previewUrl)
                    <a href="{{ $previewUrl }}" target="_blank" rel="noopener" class="oc-btn"><x-icon name="eye" :size="16" /> Önizle</a>
                @endif
                @if ($liveUrl)
                    <a href="{{ $liveUrl }}" target="_blank" rel="noopener" class="oc-btn ghost"><x-icon name="external-link" :size="16" /> Sitede aç</a>
                @endif
            </x-slot:actions>
        </x-pagehead>
        @include('admin.translations._tabs', ['active' => $language->code])
    </x-slot:head>

    @if (! $exists)
        <div class="cms-body mb-6">
            <div class="oc-banner info"><x-icon name="languages" /><div>Bu sayfanın {{ $language->native_name }} sürümü yok. Alanlar {{ strtoupper($page->preferredTranslation()?->locale ?? '') }} sürümünden kopyalandı; çevirip kaydedin.</div></div>
        </div>
    @endif
    @if ($parentUrl === null)
        <div class="cms-body mb-6">
            <div class="oc-banner warning"><x-icon name="circle-alert" /><div>Üst sayfanın ({{ $page->parent?->adminTitle() }}) {{ $language->native_name }} çevirisi yok. Bu çeviriyi kaydetmeden önce <a href="{{ route('admin.translations.edit', [$page->parent, $language]) }}">üst sayfayı çevirin</a>.</div></div>
        </div>
    @endif

    <form method="post" action="{{ route('admin.translations.update', [$page, $language]) }}"
          class="cms-body grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]"
          x-data="translationForm({
              locale: @js($language->code),
              exists: @js($exists),
              isHome: @js($isHome),
              title: @js(old('title', $translation->title)),
              slug: @js(old('slug', $translation->slug)),
              seoTitle: @js(old('seo_title', $translation->seo_title ?? '')),
              seoDescription: @js(old('seo_description', $translation->seo_description ?? '')),
              excerpt: @js(old('excerpt', $translation->excerpt ?? '')),
              imageId: @js(old('image_id', $translation->image_id)),
              parentUrl: @js($parentUrl ?? '/'.$language->code),
              suffix: ' · OctopusOn SEO',
              media: @js($media),
          })"
          @input="dirty = true" @change="dirty = true" @submit="dirty = false">
        @csrf @method('put')

        <div class="flex min-w-0 flex-col gap-6">
            <section class="oc-panel grid gap-4">
                <x-field name="title" label="Başlık">
                    <input class="oc-input text-lg font-semibold" name="title" x-model="title" @input="titleChanged()" required maxlength="255">
                </x-field>
                <div class="grid gap-4 md:grid-cols-2">
                    <x-field name="slug" label="Adres">
                        <div class="flex items-center gap-2">
                            <span class="max-w-[50%] shrink-0 truncate font-mono text-xs text-ink-faint" x-text="isHome ? parentUrl : parentUrl + '/'"></span>
                            <input class="oc-input font-mono" name="slug" x-model="slug" @input="slugTouched = true" @change="slugChanged()" @disabled($isHome) maxlength="160" placeholder="{{ $isHome ? 'ana sayfa' : '' }}">
                        </div>
                        <small class="oc-field-hint">Adres değişirse eski adres yeni adrese otomatik 301 ile yönlenir.</small>
                    </x-field>
                    <x-field name="nav_title" label="Menü adı" hint="Menülerde, kartlarda ve breadcrumb'da başlık yerine kullanılır.">
                        <input class="oc-input" name="nav_title" value="{{ old('nav_title', $translation->nav_title) }}" maxlength="255">
                    </x-field>
                </div>
                <x-field name="excerpt" label="Özet" hint="Kategori listelerinde ve ürün kartlarında görünür; SEO açıklaması boşsa o da budur.">
                    <textarea class="oc-textarea prose" name="excerpt" rows="3" x-model="excerpt" maxlength="1000"></textarea>
                </x-field>
            </section>

            @include('admin.blocks._editor', ['blocks' => old('blocks') ? json_decode(old('blocks'), true) : ($translation->blocks ?? [])])
        </div>

        <aside class="flex min-w-0 flex-col gap-6 xl:sticky xl:top-6 xl:max-h-[calc(100vh-3rem)] xl:self-start xl:overflow-y-auto">
            <section class="oc-panel grid gap-4">
                <div class="oc-panel-head mb-0"><h2>Yayın</h2></div>
                <x-field name="status" label="Durum">
                    <select class="oc-select" name="status">
                        @foreach (\App\Enums\PublishStatus::cases() as $case)
                            <option value="{{ $case->value }}" @selected($status === $case->value)>{{ $case->label() }}</option>
                        @endforeach
                    </select>
                </x-field>
                <x-field name="published_at" label="Yayın tarihi" hint="İleri bir tarih seçerseniz sayfa o an yayına girer. Boşsa ilk yayında doldurulur.">
                    <input class="oc-input" type="datetime-local" name="published_at" value="{{ $publishedAt }}">
                </x-field>
                <button type="submit" class="oc-btn primary large w-full" @disabled($parentUrl === null)><x-icon name="save" :size="18" /> Kaydet</button>
                @if ($translation->editor)
                    <p class="text-center text-sm text-ink-faint">Son kaydeden {{ $translation->editor->name }}, {{ $translation->updated_at?->diffForHumans() }}</p>
                @endif
            </section>

            <section class="oc-panel grid gap-4">
                <div class="oc-panel-head mb-0"><h2>SEO</h2></div>
                <div class="serp-preview" aria-label="Google önizlemesi">
                    <div class="serp-url" x-text="'{{ parse_url(config('cms.web_url'), PHP_URL_HOST) }}' + url.replaceAll('/', ' › ').replace(/^ › /, ' › ')"></div>
                    <div class="serp-title" x-text="previewTitle"></div>
                    <div class="serp-desc" x-text="previewDescription || 'Açıklama yok: Google sayfadan bir parça seçer.'"></div>
                </div>
                <x-field name="seo_title" label="SEO başlığı">
                    <input class="oc-input" name="seo_title" x-model="seoTitle" maxlength="255" :placeholder="title">
                    <small class="oc-field-hint"><span class="count" :class="count(seoTitle || title, 30, 60).cls" x-text="count(seoTitle || title, 30, 60).length"></span> karakter · 30–60 önerilir. Boşsa başlık kullanılır.</small>
                </x-field>
                <x-field name="seo_description" label="Meta açıklama">
                    <textarea class="oc-textarea prose" name="seo_description" rows="4" x-model="seoDescription" maxlength="320" :placeholder="excerpt"></textarea>
                    <small class="oc-field-hint"><span class="count" :class="count(seoDescription || excerpt, 70, 160).cls" x-text="count(seoDescription || excerpt, 70, 160).length"></span> karakter · 70–160 önerilir. Boşsa özet kullanılır.</small>
                </x-field>
                <x-field name="image_id" label="Paylaşım görseli" hint="Open Graph ve liste kartları; 1200×630 önerilir.">
                    <input type="hidden" name="image_id" :value="imageId ?? ''">
                    <div class="flex items-center gap-3">
                        <span class="media-thumb w-24" x-show="imageId"><img :src="$store.media.url(imageId)" alt=""></span>
                        <button type="button" class="oc-btn small" @click="chooseImage()">Seç</button>
                        <button type="button" class="oc-btn small ghost" x-show="imageId" @click="imageId = null; dirty = true">Kaldır</button>
                    </div>
                </x-field>
                <x-field name="canonical_url" label="Kanonik adres" hint="Yalnızca içerik başka bir adreste de varsa doldurun.">
                    <input class="oc-input font-mono" name="canonical_url" value="{{ old('canonical_url', $translation->canonical_url) }}" maxlength="255" placeholder="Bu sayfanın kendi adresi">
                </x-field>
                <label class="oc-check text-sm"><input type="hidden" name="noindex" value="0"><input type="checkbox" name="noindex" value="1" @checked(old('noindex', $translation->noindex))> Arama motorlarında gösterme (noindex)</label>
            </section>
        </aside>
    </form>
</x-layouts.admin>
