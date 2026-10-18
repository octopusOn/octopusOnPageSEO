{{-- Language-independent fields of a page: kind, place in the tree, program, icon, menus. --}}
<div class="grid gap-4 md:grid-cols-2">
    <x-field name="type" label="Tür" :hint="null">
        <select class="oc-select" name="type" x-model="type">
            @foreach (\App\Enums\PageType::cases() as $case)
                <option value="{{ $case->value }}" @selected(old('type', $page->type?->value) === $case->value)>{{ $case->label() }}</option>
            @endforeach
        </select>
        <small class="oc-field-hint" x-text="descriptions[type]"></small>
    </x-field>
    <x-field name="parent_id" label="Üst sayfa" hint="Kategori altına koyduğunuz sayfa o kategoride listelenir ve adresi kategorinin altına girer.">
        <select class="oc-select" name="parent_id" :disabled="type === 'home' || type === 'legal'">
            <option value="">— En üst düzey —</option>
            @foreach ($parents as $option)
                <option value="{{ $option['id'] }}" @selected((int) old('parent_id', $page->parent_id) === $option['id'])>{{ $option['label'] }} ({{ $option['type']->label() }})</option>
            @endforeach
        </select>
    </x-field>
    <x-field name="key" label="Anahtar" hint="Hesap sitesi bu yasal metni bu adla bulur ve bağlantı verir: credit-terms, pre-contract-information, distance-sales, privacy, cookies." x-show="type === 'legal'">
        <input class="oc-input font-mono" name="key" value="{{ old('key', $page->key) }}" maxlength="40" placeholder="privacy">
    </x-field>
    <x-field name="product" label="Program" hint="Ürün kartlarında ve indirmelerde bu sayfa o programı temsil eder.">
        <select class="oc-select" name="product">
            <option value="">—</option>
            @foreach (config('cms.products') as $id => $name)
                <option value="{{ $id }}" @selected(old('product', $page->product) === $id)>{{ $name }}</option>
            @endforeach
        </select>
    </x-field>
    <x-field name="icon" label="İkon" hint="Menü, kart ve listelerde başlığın yanında görünür.">
        <div class="flex items-center gap-2" x-data="{ icon: @js(old('icon', $page->icon) ?? '') }">
            <span class="oc-tile violet" x-html="window.cmsIcon(icon, 16)"></span>
            <select class="oc-select" name="icon" x-model="icon">
                <option value="">—</option>
                @foreach (config('cms.icons') as $name)
                    <option value="{{ $name }}">{{ $name }}</option>
                @endforeach
            </select>
        </div>
    </x-field>
    <x-field name="in_header" label="Üst menü">
        <label class="oc-check"><input type="hidden" name="in_header" value="0"><input type="checkbox" name="in_header" value="1" @checked(old('in_header', $page->in_header))> Üst menüde göster</label>
    </x-field>
    <x-field name="footer_column" label="Alt menü sütunu">
        <select class="oc-select" name="footer_column">
            <option value="">— Gösterme —</option>
            @foreach (config('cms.footer_columns') as $id => $label)
                <option value="{{ $id }}" @selected(old('footer_column', $page->footer_column) === $id)>{{ $label }}</option>
            @endforeach
        </select>
    </x-field>
</div>
