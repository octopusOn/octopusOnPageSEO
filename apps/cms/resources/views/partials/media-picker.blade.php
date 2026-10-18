{{-- The media picker: opened by $store.picker.choose(callback) from the block editor and the SEO image field. --}}
<div x-data="mediaPicker(@js(route('admin.media.index')))" x-cloak
     x-show="$store.picker.open" @picker-open.window="load(1)" @keydown.escape.window="$store.picker.open = false"
     class="oc-scrim" @click.self="$store.picker.open = false">
    <div class="oc-modal wide" role="dialog" aria-modal="true" aria-labelledby="picker-title">
        <div class="flex items-center gap-3">
            <h2 id="picker-title" class="flex-1">Medya seç</h2>
            <label class="oc-btn primary small cursor-pointer">
                <x-icon name="upload" :size="15" /> <span x-text="uploading ? 'Yükleniyor…' : 'Dosya yükle'"></span>
                <input type="file" class="sr-only" multiple accept="image/*,application/pdf" @change="upload">
            </label>
            <button type="button" class="oc-btn icon small ghost" @click="$store.picker.open = false" aria-label="Kapat"><x-icon name="x" :size="16" /></button>
        </div>
        <div class="oc-search">
            <x-icon name="search" :size="16" />
            <input type="search" placeholder="Dosya adı ya da açıklama" x-model="query" @input.debounce.300ms="load(1)">
        </div>
        <p class="field-error" x-show="error" x-text="error"></p>
        <div class="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <template x-for="file in items" :key="file.id">
                <button type="button" class="flex flex-col gap-2 rounded-xl border border-line bg-surface-1 p-2 text-left hover:border-line-strong hover:bg-surface-2" @click="$store.picker.pick(file)">
                    <span class="media-thumb">
                        <template x-if="file.mime.startsWith('image/')"><img :src="file.url" :alt="file.alt || ''" loading="lazy"></template>
                        <template x-if="!file.mime.startsWith('image/')"><span class="text-xs text-ink-muted" x-text="file.mime"></span></template>
                    </span>
                    <span class="truncate text-xs text-ink-muted" x-text="file.filename"></span>
                </button>
            </template>
        </div>
        <p class="py-8 text-center text-ink-muted" x-show="!loading && items.length === 0">Henüz dosya yok. Sağ üstten yükleyin.</p>
        <div class="flex items-center justify-between" x-show="lastPage > 1">
            <button type="button" class="oc-btn small" :disabled="page <= 1" @click="load(page - 1)">Önceki</button>
            <span class="text-sm text-ink-muted" x-text="`${page} / ${lastPage}`"></span>
            <button type="button" class="oc-btn small" :disabled="page >= lastPage" @click="load(page + 1)">Sonraki</button>
        </div>
    </div>
</div>
