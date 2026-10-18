<?php

// Settings of the OctopusOn SEO content admin (see .env.example).

return [

    /*
    | Public website (apps/web). Used for "view on site" links and draft previews.
    */
    'web_url' => rtrim((string) env('CMS_WEB_URL', 'http://localhost:3000'), '/'),

    /*
    | The website's revalidation endpoint and the secret both sides share. After every content
    | change the admin posts the `cms` cache tag there, so the site shows the change at once.
    | Leave the URL empty to rely on the site's hourly revalidation alone.
    */
    'revalidate_url' => env('CMS_REVALIDATE_URL'),
    'revalidate_secret' => env('CMS_REVALIDATE_SECRET'),

    /*
    | Secret for draft previews: the admin signs preview links with it, and the website sends it
    | as a bearer token to read unpublished translations through the API.
    */
    'preview_secret' => env('CMS_PREVIEW_SECRET'),

    /*
    | Disk of uploaded media (config/filesystems.php). Its files must have public URLs.
    */
    'media_disk' => env('CMS_MEDIA_DISK', 'public'),

    /*
    | First admin account, created by the database seeder and `php artisan cms:admin`.
    */
    'admin' => [
        'name' => env('CMS_ADMIN_NAME', 'Admin'),
        'email' => env('CMS_ADMIN_EMAIL', 'admin@octopuson.test'),
        'password' => env('CMS_ADMIN_PASSWORD'),
    ],

    /*
    | Children shown per page of a category listing; later pages live at `<category>/page/2`.
    */
    'per_page' => 12,

    /*
    | The OctopusOn SEO programs a page can belong to. The ids match apps/web/src/lib/products.json,
    | which knows their releases, colors and command-line names; the names are brand names and
    | the same in every language. A new program goes in both lists.
    */
    'products' => [
        'seo-crawler' => 'SEO Crawler',
        'log-analyser' => 'Log Analyser',
        'serp-checker' => 'SERP Checker',
        'seo-keyword-research' => 'SEO Keyword Research',
        'index-checker' => 'Index Checker',
    ],

    /*
    | The account site where people sign in, buy credits and connect their programs.
    */
    'account_url' => env('CMS_ACCOUNT_URL', 'https://my.octopuson.com'),

    /*
    | Footer columns a page can be listed in. The website names the columns.
    */
    'footer_columns' => [
        'product' => 'Ürün',
        'resources' => 'Kaynaklar',
        'company' => 'Şirket',
    ],

    /*
    | Icons pages and block items can use (Lucide names; apps/web/src/components/blocks/icons.tsx
    | maps each one to its component).
    */
    'icons' => [
        'bell', 'book-open', 'bot', 'bot-message-square', 'chart-column-big', 'chart-line', 'circle-check',
        'code', 'database', 'download', 'earth', 'file-search', 'file-text', 'folder', 'gauge', 'git-compare',
        'layers', 'link', 'lock', 'network', 'newspaper', 'radar', 'rocket', 'scroll-text', 'search',
        'shield-check', 'shield-x', 'sparkles', 'target', 'terminal', 'zap',
    ],

];
