<x-layouts.admin title="Yeni sayfa">
    <x-slot:head>
        <x-pagehead title="Yeni sayfa" lead="Önce türünü, yerini ve ilk dildeki başlığını seçin; içeriği bir sonraki adımda bloklarla kurarsınız." />
    </x-slot:head>

    <form method="post" action="{{ route('admin.pages.store') }}" class="cms-body grid max-w-5xl gap-6"
          x-data="{ type: @js(old('type', $page->type->value)), descriptions: @js(collect(\App\Enums\PageType::cases())->mapWithKeys(fn ($c) => [$c->value => $c->description()])) }">
        @csrf
        <section class="oc-panel">
            <div class="oc-panel-head"><h2>Sayfa</h2></div>
            @include('admin.pages._settings')
        </section>
        <section class="oc-panel" x-data="slugField({ locale: @js(old('locale', $defaultLocale)), title: @js(old('title', '')), slug: @js(old('slug', '')) })">
            <div class="oc-panel-head"><h2>İlk dil</h2></div>
            <div class="grid gap-4 md:grid-cols-[160px_minmax(0,1fr)_minmax(0,1fr)]">
                <x-field name="locale" label="Dil">
                    <select class="oc-select" name="locale" x-model="locale">
                        @foreach ($languages as $language)
                            <option value="{{ $language->code }}">{{ $language->native_name }}</option>
                        @endforeach
                    </select>
                </x-field>
                <x-field name="title" label="Başlık">
                    <input class="oc-input" name="title" x-model="title" @input="titleChanged()" required maxlength="255">
                </x-field>
                <x-field name="slug" label="Adres (kısa ad)" hint="Boş bırakılırsa başlıktan üretilir.">
                    <input class="oc-input font-mono" name="slug" x-model="slug" @input="touched = true" @change="slugChanged()" :disabled="type === 'home'" maxlength="160">
                </x-field>
            </div>
        </section>
        <div class="flex gap-3">
            <button type="submit" class="oc-btn primary large">Oluştur ve düzenle</button>
            <a href="{{ route('admin.pages.index') }}" class="oc-btn large ghost">Vazgeç</a>
        </div>
    </form>
</x-layouts.admin>
