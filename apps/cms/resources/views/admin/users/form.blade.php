@php($editing = $user->exists)
<x-layouts.admin :title="$editing ? $user->name : 'Kullanıcı ekle'">
    <x-slot:head>
        <x-pagehead :title="$editing ? $user->name : 'Kullanıcı ekle'" />
    </x-slot:head>
    <form method="post" action="{{ $editing ? route('admin.users.update', $user) : route('admin.users.store') }}" class="cms-body grid max-w-2xl gap-6">
        @csrf
        @if ($editing) @method('put') @endif
        <section class="oc-panel grid gap-4">
            <x-field name="name" label="Ad"><input class="oc-input" name="name" value="{{ old('name', $user->name) }}" required maxlength="255"></x-field>
            <x-field name="email" label="E-posta"><input class="oc-input" type="email" name="email" value="{{ old('email', $user->email) }}" required maxlength="255" autocomplete="off"></x-field>
            <x-field name="password" :label="$editing ? 'Yeni parola' : 'Parola'" :hint="$editing ? 'Değiştirmeyecekseniz boş bırakın. En az 10 karakter.' : 'En az 10 karakter.'">
                <input class="oc-input" type="password" name="password" @required(! $editing) autocomplete="new-password">
            </x-field>
            <x-field name="password_confirmation" label="Parola (tekrar)"><input class="oc-input" type="password" name="password_confirmation" autocomplete="new-password"></x-field>
        </section>
        <div class="flex gap-3">
            <button type="submit" class="oc-btn primary large"><x-icon name="save" :size="18" /> Kaydet</button>
            <a href="{{ route('admin.users.index') }}" class="oc-btn large ghost">Vazgeç</a>
        </div>
    </form>
</x-layouts.admin>
