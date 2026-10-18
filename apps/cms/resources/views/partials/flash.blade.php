<div class="cms-body mb-6 flex flex-col gap-3 empty:hidden">
    @if (session('status'))
        <div class="oc-banner success" role="status"><x-icon name="check" /> <div>{{ session('status') }}</div></div>
    @endif
    @if (session('error'))
        <div class="oc-banner error" role="alert"><x-icon name="circle-alert" /> <div>{{ session('error') }}</div></div>
    @endif
    @if ($errors->any())
        <div class="oc-banner error" role="alert">
            <x-icon name="circle-alert" />
            <div>
                <strong>Kaydedilemedi.</strong>
                <ul class="mt-1 list-disc pl-5">
                    @foreach (collect($errors->all())->unique()->take(8) as $message)
                        <li>{{ $message }}</li>
                    @endforeach
                </ul>
            </div>
        </div>
    @endif
</div>
