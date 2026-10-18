# octopusSEO CMS

The content admin of the octopusSEO website: pages (categories, products, posts and docs included) in every language, with blocks, SEO fields, media, menus and redirects. Laravel 13 with Blade, Tailwind 4 and Alpine, styled with `@octopusseo/design`. `apps/web` reads it through `/api/v1`.

- Running it, the data model, the API and deployment: [docs/cms.md](../../docs/cms.md)
- Editor's guide (Turkish): [docs/cms-kilavuz.md](../../docs/cms-kilavuz.md)
- Design decisions: [ADR 0073](../../docs/adr/0073-website-cms.md)

```bash
composer install && cp .env.example .env && php artisan key:generate
touch database/database.sqlite && php artisan migrate --seed && php artisan storage:link
pnpm --filter @octopusseo/cms build && php artisan serve
php artisan test
```
