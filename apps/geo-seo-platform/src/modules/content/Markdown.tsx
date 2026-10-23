// Markdown an assistant wrote, drawn as elements (never as HTML): headings, lists, quotes,
// rules, paragraphs, bold, italics and code.

import { Fragment } from "react";
import { inlineParts, markdownBlocks, type Block } from "./brief";

function Inline({ text }: { text: string }) {
  return (
    <>
      {inlineParts(text).map((part, index) =>
        part.code ? <code key={index}>{part.text}</code> : part.strong ? <strong key={index}>{part.text}</strong> : part.em ? <em key={index}>{part.text}</em> : <Fragment key={index}>{part.text}</Fragment>,
      )}
    </>
  );
}

function BlockView({ block }: { block: Block }) {
  switch (block.kind) {
    case "heading": {
      const Tag = (block.level <= 1 ? "h3" : block.level === 2 ? "h4" : "h5") as "h3" | "h4" | "h5";
      return (
        <Tag>
          <Inline text={block.text} />
        </Tag>
      );
    }
    case "item":
    case "number":
      return (
        <p className={`md-item ${block.kind === "number" ? "numbered" : "bulleted"} depth-${block.level}`}>
          <Inline text={block.text} />
        </p>
      );
    case "quote":
      return (
        <blockquote>
          <Inline text={block.text} />
        </blockquote>
      );
    case "rule":
      return <hr />;
    default:
      return (
        <p>
          <Inline text={block.text} />
        </p>
      );
  }
}

export function Markdown({ text }: { text: string }) {
  return (
    <div className="markdown">
      {markdownBlocks(text).map((block, index) => (
        <BlockView key={index} block={block} />
      ))}
    </div>
  );
}
