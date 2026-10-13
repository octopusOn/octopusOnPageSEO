import { de } from "./dictionaries/de";
import { en, type Dictionary } from "./dictionaries/en";
import { es } from "./dictionaries/es";
import { tr } from "./dictionaries/tr";
import type { Locale } from "./config";

const dictionaries: Record<Locale, Dictionary> = { en, tr, de, es };

export function getDictionary(locale: Locale): Dictionary {
  return dictionaries[locale];
}

export type { Dictionary };
