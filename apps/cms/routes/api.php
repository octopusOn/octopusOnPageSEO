<?php

use App\Http\Controllers\Api\ContentController;
use Illuminate\Support\Facades\Route;

Route::prefix('v1')->middleware('throttle:cms-api')->group(function () {
    Route::get('languages', [ContentController::class, 'languages']);
    Route::get('site', [ContentController::class, 'site']);
    Route::get('pages/resolve', [ContentController::class, 'resolve']);
    Route::get('sitemap', [ContentController::class, 'sitemap']);
    Route::get('legal', [ContentController::class, 'legal']);
    Route::get('legal/{slug}', [ContentController::class, 'legalDocument'])->where('slug', '[a-z0-9-]{1,160}');
});
