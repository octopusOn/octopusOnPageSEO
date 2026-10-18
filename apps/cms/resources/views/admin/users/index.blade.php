<x-layouts.admin title="Kullanıcılar">
    <x-slot:head>
        <x-pagehead title="Kullanıcılar" lead="İçerik yönetimine girebilen hesaplar. Her hesap tüm içeriği düzenleyebilir.">
            <x-slot:actions><a href="{{ route('admin.users.create') }}" class="oc-btn primary"><x-icon name="plus" :size="16" /> Kullanıcı ekle</a></x-slot:actions>
        </x-pagehead>
    </x-slot:head>
    <div class="cms-body">
        <section class="oc-panel p-0">
            <div class="oc-table-wrap">
                <table class="oc-table">
                    <thead><tr><th>Ad</th><th>E-posta</th><th>Eklendi</th><th></th></tr></thead>
                    <tbody>
                    @foreach ($users as $user)
                        <tr>
                            <td><b>{{ $user->name }}</b> @if ($user->is(auth()->user()))<span class="oc-badge accent">Siz</span>@endif</td>
                            <td>{{ $user->email }}</td>
                            <td class="text-ink-muted">{{ $user->created_at?->format('d.m.Y') }}</td>
                            <td class="text-right">
                                <a href="{{ route('admin.users.edit', $user) }}" class="oc-btn icon small ghost" aria-label="Düzenle"><x-icon name="pencil" :size="14" /></a>
                                @unless ($user->is(auth()->user()))
                                    <form method="post" action="{{ route('admin.users.destroy', $user) }}" class="inline" data-confirm="{{ $user->name }} silinsin mi?">
                                        @csrf @method('delete')
                                        <button class="oc-btn icon small danger" aria-label="Sil"><x-icon name="trash-2" :size="14" /></button>
                                    </form>
                                @endunless
                            </td>
                        </tr>
                    @endforeach
                    </tbody>
                </table>
            </div>
        </section>
    </div>
</x-layouts.admin>
