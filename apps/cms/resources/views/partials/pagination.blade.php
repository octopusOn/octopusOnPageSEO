@if ($paginator->hasPages())
    <nav class="flex items-center justify-between gap-3" aria-label="Sayfalar">
        @if ($paginator->onFirstPage())
            <span class="oc-btn small" aria-disabled="true" style="opacity: .45">Önceki</span>
        @else
            <a class="oc-btn small no-underline" href="{{ $paginator->previousPageUrl() }}" rel="prev">Önceki</a>
        @endif
        <span class="text-sm text-ink-muted">{{ $paginator->currentPage() }} / {{ $paginator->lastPage() }}</span>
        @if ($paginator->hasMorePages())
            <a class="oc-btn small no-underline" href="{{ $paginator->nextPageUrl() }}" rel="next">Sonraki</a>
        @else
            <span class="oc-btn small" aria-disabled="true" style="opacity: .45">Sonraki</span>
        @endif
    </nav>
@endif
