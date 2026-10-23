// A content score as a coloured number: good from 70, fair from 45.

import { scoreBand } from "./brief";

export function ScoreChip({ score }: { score: number | null }) {
  if (score === null) return <span className="faint">—</span>;
  return <span className={`score-chip ${scoreBand(score)}`}>{Math.round(score)}</span>;
}
