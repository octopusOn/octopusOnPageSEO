<?php

namespace App\Cms;

use Illuminate\Support\Facades\Cache;
use Illuminate\Support\Facades\Http;
use Illuminate\Support\Facades\Log;
use Throwable;

/**
 * Tells the website that content changed. Model events call `touch()`; one request per admin
 * request goes out after the response, posting the `cms` cache tag to apps/web, which expires
 * every page built from the API.
 */
class Revalidator
{
    public const TAG = 'cms';

    public const STATUS_KEY = 'cms.revalidate.last';

    private static bool $pending = false;

    public static function touch(): void
    {
        if (self::$pending) {
            return;
        }
        self::$pending = true;
        app()->terminating(fn () => self::send());
    }

    /** Posts the tag now; returns whether the website accepted it (true when no URL is set). */
    public static function send(): bool
    {
        self::$pending = false;
        $url = config('cms.revalidate_url');
        if (! $url) {
            return true;
        }
        try {
            $response = Http::timeout(5)
                ->withToken((string) config('cms.revalidate_secret'))
                ->acceptJson()
                ->post($url, ['tags' => [self::TAG]]);
            $ok = $response->successful();
            $error = $ok ? null : "HTTP {$response->status()}";
        } catch (Throwable $e) {
            $ok = false;
            $error = $e->getMessage();
        }
        if (! $ok) {
            Log::warning('Website revalidation failed', ['url' => $url, 'error' => $error]);
        }
        Cache::forever(self::STATUS_KEY, ['ok' => $ok, 'error' => $error, 'at' => now()->toIso8601String()]);

        return $ok;
    }

    /** @return array{ok: bool, error: string|null, at: string}|null */
    public static function lastStatus(): ?array
    {
        return Cache::get(self::STATUS_KEY);
    }
}
