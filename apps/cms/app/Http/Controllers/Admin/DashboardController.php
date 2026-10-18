<?php

namespace App\Http\Controllers\Admin;

use App\Cms\Revalidator;
use App\Enums\PublishStatus;
use App\Http\Controllers\Controller;
use App\Models\Language;
use App\Models\Page;
use App\Models\PageTranslation;
use Illuminate\View\View;

class DashboardController extends Controller
{
    public function __invoke(): View
    {
        $languages = Language::ordered();
        $pages = Page::query()->count();
        $coverage = $languages->map(fn (Language $language) => [
            'language' => $language,
            'published' => PageTranslation::query()->where('locale', $language->code)->live()->count(),
            'drafts' => PageTranslation::query()->where('locale', $language->code)->where('status', PublishStatus::Draft)->count(),
            'missing' => $pages - PageTranslation::query()->where('locale', $language->code)->count(),
        ]);

        return view('admin.dashboard', [
            'pages' => $pages,
            'live' => PageTranslation::query()->live()->count(),
            'drafts' => PageTranslation::query()->where('status', PublishStatus::Draft)->count(),
            'missing' => $coverage->where('language.is_active', true)->sum('missing'),
            'coverage' => $coverage,
            'recent' => PageTranslation::query()->with('page', 'editor')->latest('updated_at')->limit(8)->get(),
            'revalidate' => Revalidator::lastStatus(),
            'revalidateConfigured' => (bool) config('cms.revalidate_url'),
        ]);
    }
}
