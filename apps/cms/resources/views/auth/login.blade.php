<!doctype html>
<html lang="tr">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <meta name="robots" content="noindex, nofollow">
    <title>Giriş · OctopusOn SEO İçerik</title>
    <link rel="icon" href="/favicon.svg" type="image/svg+xml">
    @vite(['resources/css/app.css', 'resources/js/app.js'])
</head>
<body class="oc-ground grid min-h-screen place-items-center p-4">
<main class="flex w-full max-w-sm flex-col items-center gap-8">
    <div class="flex flex-col items-center gap-4 text-center">
        <x-brand-mark :size="72" />
        <div>
            <h1 class="text-3xl font-semibold tracking-tight">OctopusOn <b>SEO</b></h1>
            <p class="mt-1 text-ink-muted">Web sitesi içerik yönetimi</p>
        </div>
    </div>
    <form method="post" action="{{ route('login') }}" class="oc-card w-full gap-4">
        @csrf
        <x-field name="email" label="E-posta">
            <input class="oc-input" type="email" name="email" value="{{ old('email') }}" required autofocus autocomplete="username">
        </x-field>
        <x-field name="password" label="Parola">
            <input class="oc-input" type="password" name="password" required autocomplete="current-password">
        </x-field>
        <label class="oc-check text-sm text-ink-muted"><input type="checkbox" name="remember" value="1"> Beni hatırla</label>
        <button type="submit" class="oc-btn primary large w-full">Giriş yap</button>
    </form>
</main>
</body>
</html>
