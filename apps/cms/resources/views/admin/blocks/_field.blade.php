{{--
    One field of a block, as Alpine markup. $f is the JS expression of the field definition and
    $m that of the object holding its value: the value is {{ $m }}[{{ $f }}.name].
--}}
@php($v = $m.'['.$f.'.name]')
<div class="oc-field">
    <span x-text="{{ $f }}.label + ({{ $f }}.required ? ' *' : '')"></span>
    <template x-if="{{ $f }}.type === 'text'">
        <input class="oc-input" type="text" x-model="{{ $v }}" :maxlength="{{ $f }}.max || 255">
    </template>
    <template x-if="{{ $f }}.type === 'url'">
        <input class="oc-input font-mono" type="text" x-model="{{ $v }}" placeholder="/tr/… ya da https://…">
    </template>
    <template x-if="{{ $f }}.type === 'textarea'">
        <textarea class="oc-textarea prose" rows="3" x-model="{{ $v }}"></textarea>
    </template>
    <template x-if="{{ $f }}.type === 'markdown'">
        <textarea class="oc-textarea prose" rows="14" x-model="{{ $v }}"></textarea>
    </template>
    <template x-if="{{ $f }}.type === 'code'">
        <textarea class="oc-textarea" rows="6" spellcheck="false" x-model="{{ $v }}"></textarea>
    </template>
    <template x-if="{{ $f }}.type === 'strings'">
        <textarea class="oc-textarea prose" rows="4" :value="({{ $v }} || []).join('\n')" @input="{{ $v }} = $event.target.value.split('\n')" placeholder="Her satıra bir madde"></textarea>
    </template>
    <template x-if="{{ $f }}.type === 'select'">
        <select class="oc-select" x-model="{{ $v }}">
            <template x-for="(label, value) in {{ $f }}.options" :key="value">
                <option :value="value" x-text="label" :selected="value === {{ $v }}"></option>
            </template>
        </select>
    </template>
    <template x-if="{{ $f }}.type === 'number'">
        <input class="oc-input" type="number" step="any" min="0" x-model.number="{{ $v }}">
    </template>
    <template x-if="{{ $f }}.type === 'toggle'">
        <label class="oc-check"><input type="checkbox" x-model="{{ $v }}"> <span class="text-ink-muted">Açık</span></label>
    </template>
    <template x-if="{{ $f }}.type === 'icon'">
        <div class="flex items-center gap-2">
            <span class="oc-tile violet" x-html="icon({{ $v }}, 16)"></span>
            <select class="oc-select" x-model="{{ $v }}">
                <option value="">—</option>
                <template x-for="name in icons" :key="name"><option :value="name" x-text="name" :selected="name === {{ $v }}"></option></template>
            </select>
        </div>
    </template>
    <template x-if="{{ $f }}.type === 'product'">
        <select class="oc-select" x-model="{{ $v }}">
            <option value="">—</option>
            <template x-for="(name, id) in products" :key="id"><option :value="id" x-text="name" :selected="id === {{ $v }}"></option></template>
        </select>
    </template>
    <template x-if="{{ $f }}.type === 'media'">
        <div class="flex items-center gap-3">
            <span class="media-thumb w-28" x-show="{{ $v }}"><img :src="$store.media.url({{ $v }})" alt=""></span>
            <span class="min-w-0 flex-1 truncate text-sm text-ink-muted" x-text="{{ $v }} ? $store.media.name({{ $v }}) : 'Dosya seçilmedi'"></span>
            <button type="button" class="oc-btn small" @click="$store.picker.choose((id) => { {{ $v }} = id; $dispatch('input') })">Seç</button>
            <button type="button" class="oc-btn small ghost" x-show="{{ $v }}" @click="{{ $v }} = null">Kaldır</button>
        </div>
    </template>
    <small class="oc-field-hint" x-show="{{ $f }}.hint" x-text="{{ $f }}.hint"></small>
</div>
