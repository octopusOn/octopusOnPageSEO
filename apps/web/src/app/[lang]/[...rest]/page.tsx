import { notFound } from "next/navigation";

/** Unknown paths inside a language render the localized layout with the not-found page. */
export default function CatchAll() {
  notFound();
}
