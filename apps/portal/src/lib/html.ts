const ENTITIES: Record<string, string> = { amp: "&", lt: "<", gt: ">", quot: '"', "#39": "'", nbsp: " " };

/** HTML elements GitHub release notes use; anything else in angle brackets (`Vec<u8>`) is text. */
const TAGS =
  "a|abbr|b|blockquote|br|code|dd|del|details|div|dl|dt|em|h[1-6]|hr|i|img|ins|kbd|li|ol|p|picture|pre|s|source|span|strong|sub|summary|sup|table|tbody|td|th|thead|tr|u|ul|video";
const TAG = new RegExp(`</?(?:${TAGS})(?:\\s[^<>]*)?/?>`, "gi");

/** Text of a line with HTML tags and comments removed and common entities decoded. */
export function stripHtml(line: string): string {
  return line
    .replace(/<!--.*?-->/g, "")
    .replace(TAG, "")
    .replace(/&(amp|lt|gt|quot|#39|nbsp);/g, (_, name: string) => ENTITIES[name] ?? "");
}
