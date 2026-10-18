<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\PageTranslation;
use App\Models\Redirect;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use Illuminate\View\View;

/** Old addresses: the automatic 301s of moved pages and hand-made ones. */
class RedirectController extends Controller
{
    public function index(Request $request): View
    {
        $redirects = Redirect::query()->with('translation')
            ->when($request->query('q'), fn ($q, $term) => $q->where(fn ($q) => $q->whereLike('from_path', "%{$term}%")->orWhereLike('target', "%{$term}%")))
            ->orderByDesc('automatic')->orderBy('from_path')
            ->paginate(50)->withQueryString();

        return view('admin.redirects.index', ['redirects' => $redirects, 'query' => $request->query('q')]);
    }

    public function store(Request $request): RedirectResponse
    {
        $data = $request->validate([
            'from_path' => ['required', 'string', 'max:450'],
            'target' => ['required', 'string', 'max:255', 'regex:~^(/|https?://)~i'],
            'status_code' => ['required', Rule::in([301, 302, 307, 308])],
        ], ['target.regex' => 'Hedef / ya da https:// ile başlamalı.']);
        $from = Redirect::normalize($data['from_path']);
        if ($from === '/') {
            throw ValidationException::withMessages(['from_path' => 'Kök adres yönlendirilemez.']);
        }
        $live = PageTranslation::query()->get(['locale', 'path'])->first(fn (PageTranslation $t) => $t->url() === $from);
        if ($live) {
            throw ValidationException::withMessages(['from_path' => "$from yayındaki bir sayfanın adresi; yönlendirilemez."]);
        }
        if (Redirect::normalize($data['target']) === $from) {
            throw ValidationException::withMessages(['target' => 'Bir adres kendisine yönlendirilemez.']);
        }
        Redirect::query()->updateOrCreate(['from_path' => $from], [
            'target' => $data['target'],
            'translation_id' => null,
            'status_code' => (int) $data['status_code'],
            'automatic' => false,
        ]);

        return back()->with('status', "$from yönlendirmesi kaydedildi.");
    }

    public function destroy(Redirect $redirect): RedirectResponse
    {
        $redirect->delete();

        return back()->with('status', "{$redirect->from_path} yönlendirmesi silindi.");
    }
}
