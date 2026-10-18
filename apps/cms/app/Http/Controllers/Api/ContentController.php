<?php

namespace App\Http\Controllers\Api;

use App\Cms\Content;
use App\Cms\Preview;
use App\Http\Controllers\Controller;
use Illuminate\Http\JsonResponse;
use Illuminate\Http\Request;

/**
 * Read-only JSON for apps/web and, for the legal texts, apps/portal. Public, except that a
 * request with the preview secret as its bearer token also sees drafts.
 */
class ContentController extends Controller
{
    public function languages(Request $request): JsonResponse
    {
        return response()->json(['data' => $this->content($request)->languages()]);
    }

    /** Languages and the menus of one language: what every page's layout needs. */
    public function site(Request $request): JsonResponse
    {
        $locale = (string) $request->query('locale', '');
        $content = $this->content($request);

        return response()->json(['data' => [
            'languages' => $content->languages(),
            'navigation' => $content->navigation($locale),
        ]]);
    }

    public function resolve(Request $request): JsonResponse
    {
        $request->validate([
            'locale' => ['required', 'string', 'max:12'],
            'path' => ['nullable', 'string', 'max:460'],
        ]);
        $result = $this->content($request)->resolve($request->query('locale'), (string) $request->query('path', ''));
        if (! $result) {
            return response()->json(['message' => 'Not found'], 404);
        }

        return response()->json(isset($result['redirect']) ? ['redirect' => $result['redirect']] : ['data' => $result['page']]);
    }

    /** The account site's legal texts in a language: keys, slugs and titles. */
    public function legal(Request $request): JsonResponse
    {
        $request->validate(['locale' => ['required', 'string', 'max:12']]);

        return response()->json(['data' => $this->content($request)->legal($request->query('locale'))]);
    }

    /** One legal text by its slug (or key) in a language. */
    public function legalDocument(Request $request, string $slug): JsonResponse
    {
        $request->validate(['locale' => ['required', 'string', 'max:12']]);
        $document = $this->content($request)->legalDocument($request->query('locale'), $slug);

        return $document ? response()->json(['data' => $document]) : response()->json(['message' => 'Not found'], 404);
    }

    public function sitemap(Request $request): JsonResponse
    {
        return response()->json(['data' => $this->content($request)->sitemap()]);
    }

    private function content(Request $request): Content
    {
        return new Content(drafts: Preview::authorizes($request->bearerToken()));
    }
}
