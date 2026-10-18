@props(['title', 'lead' => null])
<header class="oc-pagehead cms-pagehead">
    <div class="oc-pagehead-text">
        @isset($eyebrow)
            <div class="mb-2 flex flex-wrap items-center gap-2 text-sm text-ink-faint">{{ $eyebrow }}</div>
        @endisset
        <h1 class="oc-pagehead-title">{{ $title }}</h1>
        @if ($lead)
            <p class="oc-pagehead-lead">{{ $lead }}</p>
        @endif
    </div>
    @isset($actions)
        <div class="oc-pagehead-actions">{{ $actions }}</div>
    @endisset
</header>
