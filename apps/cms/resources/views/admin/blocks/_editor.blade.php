{{-- The block editor: ordered, collapsible blocks with schema-driven forms (app/Cms/Blocks.php). --}}
<div x-data="blockEditor({
        definitions: @js($definitions),
        blocks: @js($blocks),
        icons: @js(config('cms.icons')),
        products: @js(config('cms.products')),
        media: @js($media),
     })" class="flex flex-col gap-3">
    <input type="hidden" name="blocks" :value="json">

    <div class="flex items-center gap-2">
        <h2 class="flex-1 text-xl font-semibold">İçerik blokları</h2>
        <button type="button" class="oc-btn small ghost" @click="expandAll(true)">Tümünü aç</button>
        <button type="button" class="oc-btn small ghost" @click="expandAll(false)">Tümünü kapat</button>
    </div>

    <template x-for="(block, index) in blocks" :key="block.id">
        <article class="block-card" :class="{ open: open[block.id] }" :id="`block-${block.id}`">
            <header class="block-head" @click="toggle(block)">
                <span class="oc-tile blue" x-html="icon(def(block).icon, 16)"></span>
                <div class="min-w-0 flex-1">
                    <div class="font-semibold" x-text="def(block).label"></div>
                    <div class="truncate text-sm text-ink-muted" x-text="summary(block)"></div>
                </div>
                <div class="flex gap-1" @click.stop>
                    <button type="button" class="oc-btn icon small ghost" :disabled="index === 0" @click="move(blocks, index, -1)" aria-label="Yukarı" x-html="icon('arrow-up', 14)"></button>
                    <button type="button" class="oc-btn icon small ghost" :disabled="index === blocks.length - 1" @click="move(blocks, index, 1)" aria-label="Aşağı" x-html="icon('arrow-down', 14)"></button>
                    <button type="button" class="oc-btn icon small ghost" @click="duplicate(index)" aria-label="Çoğalt" title="Çoğalt" x-html="icon('copy', 14)"></button>
                    <button type="button" class="oc-btn icon small ghost" @click="showAdd(index + 1)" aria-label="Altına blok ekle" title="Altına blok ekle" x-html="icon('plus', 14)"></button>
                    <button type="button" class="oc-btn icon small danger" @click="remove(index)" aria-label="Sil" x-html="icon('trash-2', 14)"></button>
                </div>
                <span class="text-ink-faint transition-transform" :class="{ 'rotate-180': open[block.id] }" x-html="icon('chevron-down', 18)"></span>
            </header>
            <div class="block-body" x-show="open[block.id]" x-cloak>
                <p class="oc-field-hint" x-text="def(block).hint"></p>
                <p class="text-sm text-ink-muted" x-show="def(block).fields.length === 0">Bu bloğun ayarı yok; içeriği site kendisi doldurur.</p>
                <div class="grid gap-4 md:grid-cols-2">
                    <template x-for="field in def(block).fields" :key="field.name">
                        <div :class="{ 'md:col-span-2': ['textarea', 'markdown', 'code', 'strings', 'items'].includes(field.type) }">
                            <template x-if="field.type !== 'items'">
                                <div>@include('admin.blocks._field', ['f' => 'field', 'm' => 'block.data'])</div>
                            </template>
                            <template x-if="field.type === 'items'">
                                <div class="oc-field">
                                    <span x-text="field.label"></span>
                                    <div class="flex flex-col gap-3">
                                        <template x-for="(item, itemIndex) in block.data[field.name]" :key="itemIndex">
                                            <div class="item-card">
                                                <div class="flex items-center gap-2">
                                                    <span class="flex-1 text-sm font-semibold text-ink-muted" x-text="itemTitle(item, itemIndex, field.item_label)"></span>
                                                    <button type="button" class="oc-btn icon small ghost" :disabled="itemIndex === 0" @click="move(block.data[field.name], itemIndex, -1)" aria-label="Yukarı" x-html="icon('arrow-up', 14)"></button>
                                                    <button type="button" class="oc-btn icon small ghost" :disabled="itemIndex === block.data[field.name].length - 1" @click="move(block.data[field.name], itemIndex, 1)" aria-label="Aşağı" x-html="icon('arrow-down', 14)"></button>
                                                    <button type="button" class="oc-btn icon small danger" @click="removeItem(block.data[field.name], itemIndex)" aria-label="Kaldır" x-html="icon('x', 14)"></button>
                                                </div>
                                                <div class="grid gap-3 md:grid-cols-2">
                                                    <template x-for="sub in field.fields" :key="sub.name">
                                                        <div :class="{ 'md:col-span-2': ['textarea', 'markdown', 'code', 'strings'].includes(sub.type) }">
                                                            @include('admin.blocks._field', ['f' => 'sub', 'm' => 'item'])
                                                        </div>
                                                    </template>
                                                </div>
                                            </div>
                                        </template>
                                        <button type="button" class="oc-btn small self-start" @click="addItem(block.data[field.name], field.fields)">
                                            <span x-html="icon('plus', 14)"></span> <span x-text="`${field.item_label} ekle`"></span>
                                        </button>
                                    </div>
                                </div>
                            </template>
                        </div>
                    </template>
                </div>
            </div>
        </article>
    </template>

    <div class="oc-empty rounded-2xl border border-dashed border-line-strong" x-show="blocks.length === 0">
        <p>Bu sayfada henüz blok yok.</p>
    </div>

    <button type="button" class="oc-btn large self-start" @click="showAdd(null)"><span x-html="icon('plus', 18)"></span> Blok ekle</button>

    <div class="oc-scrim" x-show="adding" x-cloak @click.self="adding = false" @keydown.escape.window="adding = false">
        <div class="oc-modal wide" role="dialog" aria-modal="true" aria-label="Blok ekle">
            <div class="flex items-center"><h2 class="flex-1">Blok ekle</h2><button type="button" class="oc-btn icon small ghost" @click="adding = false" aria-label="Kapat" x-html="icon('x', 16)"></button></div>
            <div class="grid gap-2 sm:grid-cols-2">
                <template x-for="(definition, type) in defs" :key="type">
                    <button type="button" class="flex items-start gap-3 rounded-xl border border-line bg-surface-1 p-3 text-left hover:border-line-strong hover:bg-surface-2" @click="add(type)">
                        <span class="oc-tile blue" x-html="icon(definition.icon, 16)"></span>
                        <span><span class="block font-semibold" x-text="definition.label"></span><span class="block text-sm text-ink-muted" x-text="definition.hint"></span></span>
                    </button>
                </template>
            </div>
        </div>
    </div>
</div>
