@php($editing = $language->exists)
<x-layouts.admin :title="$editing ? $language->native_name : 'Dil ekle'">
    <x-slot:head>
        <x-pagehead :title="$editing ? $language->native_name : 'Dil ekle'" lead="Yeni bir dil eklediğinizde sayfaların o dildeki çevirilerini sayfa ekranından oluşturursunuz. Sitenin arayüz metinleri (düğmeler, etiketler) bu dilde yoksa İngilizce görünür." />
    </x-slot:head>
    <form method="post" action="{{ $editing ? route('admin.languages.update', $language) : route('admin.languages.store') }}" class="cms-body grid max-w-3xl gap-6">
        @csrf
        @if ($editing) @method('put') @endif
        <section class="oc-panel grid gap-4 md:grid-cols-2">
            <x-field name="code" label="Kod" hint="Küçük harf: tr, en, pt-br. Çevirisi olan dilin kodu değiştirilemez.">
                <input class="oc-input font-mono" name="code" value="{{ old('code', $language->code) }}" required maxlength="12">
            </x-field>
            <x-field name="locale_tag" label="Yerel ayar etiketi" hint="Tarih ve sayı biçimleri ile Open Graph için: tr-TR.">
                <input class="oc-input font-mono" name="locale_tag" value="{{ old('locale_tag', $language->locale_tag) }}" required maxlength="20">
            </x-field>
            <x-field name="native_name" label="Kendi dilinde adı" hint="Dil seçicide görünür: Türkçe, Deutsch.">
                <input class="oc-input" name="native_name" value="{{ old('native_name', $language->native_name) }}" required maxlength="60">
            </x-field>
            <x-field name="name" label="Türkçe adı">
                <input class="oc-input" name="name" value="{{ old('name', $language->name) }}" required maxlength="60">
            </x-field>
            <x-field name="position" label="Sıra">
                <input class="oc-input" type="number" min="0" max="999" name="position" value="{{ old('position', $language->position) }}">
            </x-field>
            <div class="flex flex-col justify-end gap-3">
                <label class="oc-check"><input type="hidden" name="is_active" value="0"><input type="checkbox" name="is_active" value="1" @checked(old('is_active', $language->is_active))> Sitede etkin</label>
                <label class="oc-check"><input type="hidden" name="is_default" value="0"><input type="checkbox" name="is_default" value="1" @checked(old('is_default', $language->is_default))> Varsayılan dil (x-default)</label>
                @error('is_default')<small class="field-error">{{ $message }}</small>@enderror
            </div>
        </section>
        <div class="flex gap-3">
            <button type="submit" class="oc-btn primary large"><x-icon name="save" :size="18" /> Kaydet</button>
            <a href="{{ route('admin.languages.index') }}" class="oc-btn large ghost">Vazgeç</a>
        </div>
    </form>
</x-layouts.admin>
