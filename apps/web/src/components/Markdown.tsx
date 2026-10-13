import type { ReactNode } from "react";

import { stripHtml } from "@/lib/html";

/**
 * Renders the small Markdown subset used in release notes (headings, lists, paragraphs, code
 * blocks, inline code, bold, links) as React elements — no raw HTML is ever injected: HTML tags
 * are dropped and only their text is kept (a `<summary>` becomes a heading).
 */
export function Markdown({ text }: { text: string }) {
  const blocks: ReactNode[] = [];
  let list: string[] = [];
  let fence: string[] | null = null;
  const flush = () => {
    if (list.length) {
      blocks.push(
        <ul key={blocks.length}>
          {list.map((item, i) => (
            <li key={i}>{inline(item)}</li>
          ))}
        </ul>,
      );
      list = [];
    }
  };
  for (const raw of text.replace(/\r/g, "").split("\n")) {
    if (/^\s*```/.test(raw)) {
      if (fence) {
        blocks.push(
          <pre key={blocks.length}>
            <code>{fence.join("\n")}</code>
          </pre>,
        );
        fence = null;
      } else {
        flush();
        fence = [];
      }
      continue;
    }
    if (fence) {
      fence.push(raw);
      continue;
    }
    const summary = /<summary\b/i.test(raw);
    const line = stripHtml(raw).trimEnd();
    if (summary && line.trim()) {
      flush();
      blocks.push(<h3 key={blocks.length}>{inline(line.replace(/\*\*/g, "").trim())}</h3>);
      continue;
    }
    const bullet = /^\s*[-*]\s+(.*)$/.exec(line);
    if (bullet) {
      list.push(bullet[1] ?? "");
      continue;
    }
    flush();
    const heading = /^(#{1,4})\s+(.*)$/.exec(line);
    if (heading) {
      blocks.push(<h3 key={blocks.length}>{inline(heading[2] ?? "")}</h3>);
    } else if (line.trim()) {
      blocks.push(<p key={blocks.length}>{inline(line)}</p>);
    }
  }
  flush();
  if (fence?.length) {
    blocks.push(
      <pre key={blocks.length}>
        <code>{fence.join("\n")}</code>
      </pre>,
    );
  }
  return <div className="markdown">{blocks}</div>;
}

function inline(text: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const pattern = /(`[^`]+`)|(\*\*[^*]+\*\*)|(\[([^\]]+)\]\((https?:\/\/[^)\s]+)\))/g;
  let last = 0;
  for (const match of text.matchAll(pattern)) {
    const index = match.index ?? 0;
    if (index > last) parts.push(text.slice(last, index));
    if (match[1]) parts.push(<code key={index}>{match[1].slice(1, -1)}</code>);
    else if (match[2]) parts.push(<strong key={index}>{match[2].slice(2, -2)}</strong>);
    else if (match[3])
      parts.push(
        <a key={index} href={match[5]} rel="noopener nofollow">
          {match[4]}
        </a>,
      );
    last = index + match[0].length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}
