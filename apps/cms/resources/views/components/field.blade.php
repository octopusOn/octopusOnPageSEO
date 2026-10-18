@props(['name' => null, 'label' => null, 'hint' => null, 'error' => null])
{{-- A labelled form control with its hint and validation message. --}}
@php($key = $error ?? $name)
<div {{ $attributes->class(['oc-field', 'invalid' => $key && $errors->has($key)]) }}>
    @if ($label)
        <span>{{ $label }}</span>
    @endif
    {{ $slot }}
    @if ($key && $errors->has($key))
        <small class="field-error">{{ $errors->first($key) }}</small>
    @elseif ($hint)
        <small class="oc-field-hint">{{ $hint }}</small>
    @endif
</div>
