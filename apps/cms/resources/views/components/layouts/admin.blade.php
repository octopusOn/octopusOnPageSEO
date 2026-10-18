@props(['title' => 'Panel'])
@php
    $nav = [
        ['admin.dashboard', 'admin.dashboard', 'Panel', 'layout-dashboard', 'violet'],
        ['admin.pages.index', 'admin.pages.*|admin.translations.*', 'Sayfalar', 'folder-tree', 'blue'],
        ['admin.media.index', 'admin.media.*', 'Medya', 'image', 'teal'],
        ['admin.languages.index', 'admin.languages.*', 'Diller', 'languages', 'orange'],
        ['admin.redirects.index', 'admin.redirects.*', 'Yönlendirmeler', 'arrow-right-left', 'pink'],
        ['admin.users.index', 'admin.users.*', 'Kullanıcılar', 'users', 'slate'],
    ];
@endphp
<!doctype html>
<html lang="tr">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="csrf-token" content="{{ csrf_token() }}">
    <meta name="robots" content="noindex, nofollow">
    <title>{{ $title }} · OctopusOn SEO İçerik</title>
    <link rel="icon" href="/favicon.svg" type="image/svg+xml">
    @vite(['resources/css/app.css', 'resources/js/app.js'])
</head>
<body class="oc-ground">
<div class="oc-shell cms-shell">
    <aside class="oc-rail">
        <a href="{{ route('admin.dashboard') }}" class="oc-brand no-underline">
            <x-brand-mark />
            <span>
                <span class="oc-brand-name block">OctopusOn SEO</span>
                <span class="oc-brand-product block">İçerik yönetimi</span>
            </span>
        </a>
        <nav class="oc-nav" aria-label="Yönetim">
            @foreach ($nav as [$route, $pattern, $label, $icon, $tile])
                <a href="{{ route($route) }}" @class(['oc-nav-item no-underline', 'active' => request()->routeIs(...explode('|', $pattern))])
                   @if (request()->routeIs(...explode('|', $pattern))) aria-current="page" @endif>
                    <span class="oc-tile {{ $tile }}"><x-icon :name="$icon" :size="16" /></span>
                    {{ $label }}
                </a>
            @endforeach
        </nav>
        <div class="oc-rail-foot">
            <a href="{{ config('cms.web_url') }}" target="_blank" rel="noopener" class="oc-nav-item no-underline">
                <span class="oc-tile slate"><x-icon name="external-link" :size="16" /></span> Siteyi aç
            </a>
            <form method="post" action="{{ route('logout') }}">
                @csrf
                <button type="submit" class="oc-nav-item" title="Çıkış yap">
                    <span class="oc-tile slate"><x-icon name="log-out" :size="16" /></span>
                    <span class="min-w-0 flex-1 truncate">{{ auth()->user()->name }}</span>
                </button>
            </form>
        </div>
    </aside>
    <div class="oc-work">
        <main class="cms-main" id="main">
            @isset($head)
                {{ $head }}
            @endisset
            @include('partials.flash')
            {{ $slot }}
        </main>
    </div>
</div>
@include('partials.media-picker')
</body>
</html>
