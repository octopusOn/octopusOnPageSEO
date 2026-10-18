<?php

namespace App\Providers;

use App\Cms\Revalidator;
use App\Models\Language;
use App\Models\Media;
use App\Models\Page;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Illuminate\Cache\RateLimiting\Limit;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\RateLimiter;
use Illuminate\Support\ServiceProvider;
use Illuminate\Validation\Rules\Password;

class AppServiceProvider extends ServiceProvider
{
    public function register(): void
    {
        //
    }

    public function boot(): void
    {
        // The website builds every page from the API, a few hundred requests at once.
        RateLimiter::for('cms-api', fn (Request $request) => Limit::perMinute(1200)->by($request->ip()));
        RateLimiter::for('login', fn (Request $request) => [
            Limit::perMinute(5)->by(mb_strtolower((string) $request->input('email')).'|'.$request->ip()),
            Limit::perMinute(20)->by($request->ip()),
        ]);

        Password::defaults(fn () => Password::min(10));

        // Any content change expires the website's cached pages (see Revalidator).
        foreach ([Language::class, Media::class, Page::class, PageTranslation::class, Redirect::class] as $model) {
            $model::saved(fn () => Revalidator::touch());
            $model::deleted(fn () => Revalidator::touch());
        }
    }
}
