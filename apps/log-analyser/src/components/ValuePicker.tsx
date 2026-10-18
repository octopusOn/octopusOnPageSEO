// Value input with a suggestion menu: popular values with their hit counts, narrowed while
// typing. `multi` keeps a list of chips (any of them matches); `single` is an autocompleting
// text input. Suggestions are only a shortcut: any typed value is accepted, and typed text is
// kept when the input loses focus, so nothing is lost without pressing Enter.

import { useEffect, useRef, useState, type ClipboardEvent, type KeyboardEvent } from "react";
import { Check, Plus, X } from "lucide-react";

import { formatNumber } from "../lib/format";
import { useI18n } from "../i18n";

export interface Suggestion {
  value: string;
  /** Display text when it differs from the value (e.g. a bot's name). */
  label?: string;
  hits?: number;
}

interface Props {
  mode: "multi" | "single";
  values: string[];
  onChange: (values: string[]) => void;
  suggestions: Suggestion[];
  loading?: boolean;
  /** Called with the typed text so the caller can fetch matching suggestions. */
  onSearch?: (text: string) => void;
  /** Called the first time the menu opens (lazy loading). */
  onOpen?: () => void;
  /** Accept values that are not suggested. */
  allowCustom?: boolean;
  /** Marks chips the query cannot use (e.g. text in a number field). */
  isValid?: (value: string) => boolean;
  /** Footer under the menu explaining what may be typed; defaults to the text-field hint. */
  freeTextHint?: string;
  /** Values are long technical strings (URLs, user agents). */
  monospace?: boolean;
  placeholder?: string;
}

export function ValuePicker({
  mode,
  values,
  onChange,
  suggestions,
  loading,
  onSearch,
  onOpen,
  allowCustom = true,
  isValid,
  freeTextHint,
  monospace,
  placeholder,
}: Props) {
  const { t, locale } = useI18n();
  const root = useRef<HTMLDivElement>(null);
  const input = useRef<HTMLInputElement>(null);
  const [open, setOpen] = useState(false);
  const [text, setText] = useState(mode === "single" ? values[0] ?? "" : "");
  const [active, setActive] = useState(0);
  // A value added on blur but still shown as typed text, see `onBlur`.
  const [held, setHeld] = useState<string | null>(null);

  // Single mode mirrors external changes (e.g. the form re-read from query text).
  const external = values[0] ?? "";
  useEffect(() => {
    if (mode === "single") setText(external);
  }, [mode, external]);

  useEffect(() => {
    if (!open) return undefined;
    const close = (event: MouseEvent) => {
      if (root.current && !root.current.contains(event.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", close);
    return () => document.removeEventListener("mousedown", close);
  }, [open]);

  const typed = text.trim();
  const freeText = allowCustom && mode === "multi";
  const showCustom = freeText && typed.length > 0 && !suggestions.some((s) => s.value === typed) && !values.includes(typed);
  const customLabel = typed.includes("*") && !isValid ? t("picker.addPattern", { value: typed }) : t("picker.add", { value: typed });
  const options: Suggestion[] = showCustom ? [{ value: typed, label: customLabel }, ...suggestions] : suggestions;

  const openMenu = () => {
    if (!open) {
      setOpen(true);
      onOpen?.();
    }
  };

  const pick = (option: Suggestion) => {
    if (mode === "single") {
      setText(option.value);
      onChange([option.value]);
      setOpen(false);
      return;
    }
    onChange(values.includes(option.value) ? values.filter((v) => v !== option.value) : [...values, option.value]);
    if (option.value === typed) {
      setText("");
      onSearch?.("");
    }
    input.current?.focus();
  };

  /** Adds typed values that are not selected yet and clears the input. */
  const commit = (items: string[]) => {
    const next = addValues(values, items);
    if (next !== values) onChange(next);
    setText("");
    onSearch?.("");
  };

  // A pasted list (one value per line, e.g. URLs from a spreadsheet) becomes one chip per line.
  const onPaste = (event: ClipboardEvent<HTMLInputElement>) => {
    if (!freeText) return;
    const lines = pastedLines(event.clipboardData.getData("text"));
    if (lines.length < 2) return;
    event.preventDefault();
    commit([text, ...lines]);
  };

  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    switch (event.key) {
      case "ArrowDown":
        event.preventDefault();
        openMenu();
        setActive((index) => Math.min(index + 1, Math.max(options.length - 1, 0)));
        break;
      case "ArrowUp":
        event.preventDefault();
        setActive((index) => Math.max(index - 1, 0));
        break;
      case "Enter": {
        const option = open ? options[active] : undefined;
        if (option) {
          event.preventDefault();
          event.stopPropagation();
          pick(option);
        } else if (freeText && typed) {
          event.preventDefault();
          event.stopPropagation();
          commit([typed]);
        }
        break;
      }
      case "Tab":
        if (freeText && typed) commit([typed]);
        setOpen(false);
        break;
      case "Escape":
        setOpen(false);
        break;
      case "Backspace":
        if (mode === "multi" && !text && values.length > 0) onChange(values.slice(0, -1));
        break;
      default:
        break;
    }
  };

  return (
    <div className="picker" ref={root}>
      <div className={`picker-box ${open ? "focused" : ""}`} onMouseDown={(e) => {
        if (e.target === e.currentTarget) {
          e.preventDefault();
          input.current?.focus();
          openMenu();
        }
      }}>
        {mode === "multi" &&
          values.filter((value) => value !== held).map((value) => (
            <span
              className={`oc-chip ${isValid && !isValid(value) ? "invalid" : ""}`}
              key={value}
              title={isValid && !isValid(value) ? t("picker.invalid", { value }) : value}
            >
              <span className={monospace ? "mono" : undefined}>{labelOf(value, suggestions)}</span>
              <button type="button" onClick={() => onChange(values.filter((v) => v !== value))}>
                <X size={12} />
              </button>
            </span>
          ))}
        <input
          ref={input}
          className={`picker-input ${monospace ? "mono" : ""}`}
          value={text}
          placeholder={mode === "multi" && values.length > 0 ? "" : placeholder ?? t("picker.placeholder")}
          onFocus={openMenu}
          onChange={(e) => {
            setText(e.target.value);
            setActive(0);
            openMenu();
            onSearch?.(e.target.value);
            if (mode === "single") onChange([e.target.value]);
          }}
          onKeyDown={onKeyDown}
          onPaste={onPaste}
          onBlur={() => {
            if (!freeText || !typed) return;
            // Clicking e.g. "Run" blurs the input first. The value is added right away so the
            // click sees it, but turns into a chip only once the button is released: the chip
            // can grow this box and move the button from under the pointer, losing the click.
            const next = addValues(values, [typed]);
            if (next !== values) onChange(next);
            setHeld(typed);
            afterPointerRelease(() => {
              setHeld(null);
              setText("");
              onSearch?.("");
            });
          }}
        />
        {mode === "multi" && values.length > 1 && (
          <button type="button" className="picker-clear" title={t("picker.clear")} onClick={() => onChange([])}>
            <X size={13} />
          </button>
        )}
      </div>
      {open && (options.length > 0 || loading || freeText) && (
        <div className="picker-menu" role="listbox">
          {!showCustom && suggestions.some((s) => s.hits !== undefined) && <div className="picker-hint">{t("picker.hint")}</div>}
          {options.map((option, index) => {
            const selected = values.includes(option.value);
            const custom = showCustom && index === 0;
            return (
              <div
                key={`${custom ? "+" : ""}${option.value}`}
                role="option"
                aria-selected={selected}
                className={`picker-option ${index === active ? "active" : ""}`}
                onMouseEnter={() => setActive(index)}
                onMouseDown={(e) => {
                  e.preventDefault();
                  pick(option);
                }}
                title={option.value}
              >
                {mode === "multi" && (
                  <span className={`picker-check ${selected ? "on" : ""}`}>
                    {custom ? <Plus size={12} /> : selected ? <Check size={12} /> : null}
                  </span>
                )}
                <span className="picker-text">
                  <span className={monospace && !custom && !option.label ? "mono" : undefined}>{option.label ?? option.value}</span>
                  {option.label && !custom && <span className="picker-sub mono">{option.value}</span>}
                </span>
                {option.hits !== undefined && <span className="picker-hits">{formatNumber(option.hits, locale)}</span>}
              </div>
            );
          })}
          {loading && <div className="picker-hint">{t("common.loading")}</div>}
          {freeText && <div className="picker-hint picker-footer">{freeTextHint ?? t("picker.freeText")}</div>}
        </div>
      )}
    </div>
  );
}

/** `values` plus the trimmed, non-empty `items` it lacks; the same array when nothing is new. */
export function addValues(values: string[], items: string[]): string[] {
  const fresh = [...new Set(items.map((item) => item.trim()))].filter((item) => item && !values.includes(item));
  return fresh.length > 0 ? [...values, ...fresh] : values;
}

/** Non-empty lines of pasted text; a single line is typed into the input as usual. */
export function pastedLines(text: string): string[] {
  return text.split(/\r?\n|\r/).map((line) => line.trim()).filter((line) => line.length > 0);
}

/** Runs `release` after the pressed pointer, if any, is released (and its click delivered). */
function afterPointerRelease(release: () => void) {
  const done = () => {
    window.removeEventListener("pointerup", done, true);
    window.removeEventListener("pointercancel", done, true);
    window.clearTimeout(fallback);
    window.setTimeout(release, 0);
  };
  window.addEventListener("pointerup", done, true);
  window.addEventListener("pointercancel", done, true);
  // Blur without a pressed pointer (e.g. switching windows) gets no pointerup.
  const fallback = window.setTimeout(done, 1500);
}

function labelOf(value: string, suggestions: Suggestion[]): string {
  return suggestions.find((s) => s.value === value)?.label ?? value;
}
