@if ($translation->isLive())
    <span class="oc-badge good"><span class="oc-dot"></span> Yayında</span>
@elseif ($translation->status === \App\Enums\PublishStatus::Published)
    <span class="oc-badge info" title="{{ $translation->published_at }}"><span class="oc-dot"></span> Zamanlandı</span>
@else
    <span class="oc-badge warning"><span class="oc-dot"></span> Taslak</span>
@endif
