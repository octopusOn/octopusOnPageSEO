<?php

namespace App\Cms;

use App\Models\PageTranslation;

/**
 * Draft previews. The admin links to the website's `/api/preview` with the page's address, an
 * expiry time and an HMAC of both; the website checks the signature, turns on draft mode and
 * then reads drafts from the API with the same secret as a bearer token.
 */
class Preview
{
    public const LIFETIME = 3600;

    public static function enabled(): bool
    {
        return (string) config('cms.preview_secret') !== '';
    }

    public static function url(PageTranslation $translation): string
    {
        $base = config('cms.web_url');
        if (! self::enabled()) {
            return $base.$translation->url();
        }
        $path = $translation->url();
        $expires = time() + self::LIFETIME;

        return $base.'/api/preview?'.http_build_query([
            'path' => $path,
            'expires' => $expires,
            'signature' => self::sign($path, $expires),
        ]);
    }

    public static function sign(string $path, int $expires): string
    {
        return hash_hmac('sha256', $path."\n".$expires, (string) config('cms.preview_secret'));
    }

    /** Whether a request carries the preview secret as its bearer token. */
    public static function authorizes(?string $token): bool
    {
        return self::enabled() && $token !== null && hash_equals((string) config('cms.preview_secret'), $token);
    }
}
