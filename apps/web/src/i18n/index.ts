import { de } from "./dictionaries/de";
import { en, type Dictionary } from "./dictionaries/en";
import { es } from "./dictionaries/es";
import { tr } from "./dictionaries/tr";
import { hasLocale } from "./config";

const dictionaries = { en, tr, de, es };

/** Interface texts of a language; English for languages without their own dictionary. */
export function getDictionary(code: string): Dictionary {
  return hasLocale(code) ? dictionaries[code] : en;
}

export type { Dictionary };
