<?php

namespace App\View\Components;

use Illuminate\Support\HtmlString;
use Illuminate\View\Component;

/** `<x-icon name="plus" :size="16" />`: an inline Lucide icon from resources/icons.json. */
class Icon extends Component
{
    private static ?array $icons = null;

    public function __construct(public string $name, public int $size = 18) {}

    public function render(): HtmlString
    {
        self::$icons ??= json_decode((string) file_get_contents(resource_path('icons.json')), true);
        $node = self::$icons[$this->name] ?? null;
        if (! $node) {
            return new HtmlString('');
        }
        $inner = '';
        foreach ($node as [$tag, $attributes]) {
            $attrs = collect($attributes)->map(fn ($value, $key) => e($key).'="'.e($value).'"')->implode(' ');
            $inner .= "<{$tag} {$attrs}/>";
        }

        return new HtmlString(sprintf(
            '<svg class="icon" xmlns="http://www.w3.org/2000/svg" width="%1$d" height="%1$d" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">%2$s</svg>',
            $this->size,
            $inner,
        ));
    }
}
