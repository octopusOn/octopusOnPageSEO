<?php

namespace App\Http\Controllers\Admin;

use App\Http\Controllers\Controller;
use App\Models\Language;
use Illuminate\Http\RedirectResponse;
use Illuminate\Http\Request;
use Illuminate\Support\Facades\DB;
use Illuminate\Validation\Rule;
use Illuminate\Validation\ValidationException;
use Illuminate\View\View;

/** Website languages. A language's code is its URL prefix, so it is fixed once pages use it. */
class LanguageController extends Controller
{
    public function index(): View
    {
        return view('admin.languages.index', [
            'languages' => Language::query()->ordered()->withCount('translations')->get(),
        ]);
    }

    public function create(): View
    {
        return view('admin.languages.form', ['language' => new Language(['is_active' => true, 'position' => Language::query()->max('position') + 1])]);
    }

    public function store(Request $request): RedirectResponse
    {
        $language = DB::transaction(fn () => $this->persist(new Language, $this->validated($request)));

        return redirect()->route('admin.languages.index')->with('status', "{$language->native_name} eklendi. Sayfaların çevirilerini oluşturabilirsiniz.");
    }

    public function edit(Language $language): View
    {
        return view('admin.languages.form', ['language' => $language]);
    }

    public function update(Request $request, Language $language): RedirectResponse
    {
        $data = $this->validated($request, $language);
        if ($data['code'] !== $language->code && $language->translations()->exists()) {
            throw ValidationException::withMessages(['code' => 'Çevirisi olan bir dilin kodu değiştirilemez; adresleri bozulur.']);
        }
        if ($language->is_default && ! $data['is_default']) {
            throw ValidationException::withMessages(['is_default' => 'Varsayılan dili değiştirmek için başka bir dili varsayılan yapın.']);
        }
        DB::transaction(fn () => $this->persist($language, $data));

        return redirect()->route('admin.languages.index')->with('status', "{$language->native_name} kaydedildi.");
    }

    public function destroy(Language $language): RedirectResponse
    {
        if ($language->is_default) {
            return back()->with('error', 'Varsayılan dil silinemez.');
        }
        if ($language->translations()->exists()) {
            return back()->with('error', "{$language->native_name} dilinde çeviriler var. Silmek yerine pasif yapın ya da önce çevirileri silin.");
        }
        $language->delete();

        return back()->with('status', "{$language->native_name} silindi.");
    }

    private function validated(Request $request, ?Language $language = null): array
    {
        $data = $request->validate([
            'code' => ['required', 'string', 'max:12', 'regex:/^[a-z]{2,3}(-[a-z0-9]{2,8})?$/', Rule::unique('languages', 'code')->ignore($language?->id)],
            'name' => ['required', 'string', 'max:60'],
            'native_name' => ['required', 'string', 'max:60'],
            'locale_tag' => ['required', 'string', 'max:20', 'regex:/^[a-z]{2,3}(-[A-Za-z0-9]{2,8})*$/'],
            'position' => ['nullable', 'integer', 'min:0', 'max:999'],
        ], [
            'code.regex' => 'Kod küçük harflerle yazılır: tr, en, pt-br.',
            'locale_tag.regex' => 'Yerel ayar etiketi BCP 47 biçiminde olmalı: tr-TR, en-US.',
        ]);
        $data['is_default'] = $request->boolean('is_default');
        $data['is_active'] = $request->boolean('is_active') || $data['is_default'];
        $data['position'] = (int) ($data['position'] ?? 0);

        return $data;
    }

    private function persist(Language $language, array $data): Language
    {
        if ($data['is_default']) {
            Language::query()->whereKeyNot($language->id ?? 0)->where('is_default', true)->get()->each->update(['is_default' => false]);
        }
        $language->fill($data)->save();

        return $language;
    }
}
