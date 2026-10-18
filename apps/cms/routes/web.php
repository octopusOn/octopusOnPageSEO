<?php

use App\Http\Controllers\Admin\DashboardController;
use App\Http\Controllers\Admin\LanguageController;
use App\Http\Controllers\Admin\MediaController;
use App\Http\Controllers\Admin\PageController;
use App\Http\Controllers\Admin\RedirectController;
use App\Http\Controllers\Admin\TranslationController;
use App\Http\Controllers\Admin\UserController;
use App\Http\Controllers\Auth\LoginController;
use Illuminate\Support\Facades\Route;

Route::redirect('/', '/admin');

Route::middleware('guest')->group(function () {
    Route::get('login', [LoginController::class, 'show'])->name('login');
    Route::post('login', [LoginController::class, 'login'])->middleware('throttle:login');
});
Route::post('logout', [LoginController::class, 'logout'])->middleware('auth')->name('logout');

Route::middleware('auth')->prefix('admin')->name('admin.')->group(function () {
    Route::get('/', DashboardController::class)->name('dashboard');

    Route::resource('pages', PageController::class)->except('show');
    Route::post('pages/{page}/move', [PageController::class, 'move'])->name('pages.move');
    Route::get('pages/{page}/{language}', [TranslationController::class, 'edit'])->name('translations.edit');
    Route::put('pages/{page}/{language}', [TranslationController::class, 'update'])->name('translations.update');
    Route::delete('pages/{page}/{language}', [TranslationController::class, 'destroy'])->name('translations.destroy');

    Route::get('media', [MediaController::class, 'index'])->name('media.index');
    Route::post('media', [MediaController::class, 'store'])->name('media.store');
    Route::put('media/{media}', [MediaController::class, 'update'])->name('media.update');
    Route::delete('media/{media}', [MediaController::class, 'destroy'])->name('media.destroy');

    Route::resource('languages', LanguageController::class)->except('show');
    Route::resource('redirects', RedirectController::class)->only(['index', 'store', 'destroy']);
    Route::resource('users', UserController::class)->except('show');
});
