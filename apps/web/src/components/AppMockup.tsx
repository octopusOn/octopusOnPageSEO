import type { Dictionary } from "@/i18n";

/** A stylised screenshot of the desktop app, drawn with HTML so it stays sharp and light. */
export function AppMockup({ dict }: { dict: Dictionary }) {
  const m = dict.mockup;
  const days = [62, 58, 71, 66, 74, 69, 81, 77, 72, 86, 79, 90, 84, 94];
  const ai = [12, 14, 13, 18, 16, 21, 19, 24, 22, 27, 25, 31, 29, 34];
  const bots: [string, number, string][] = [
    ["Googlebot Smartphone", 99_590, "ok"],
    ["Bingbot", 39_796, "ok"],
    ["GPTBot", 24_320, "ok"],
    ["Googlebot (fake)", 12_148, "bad"],
  ];
  return (
    <div className="mockup" aria-hidden="true">
      <div className="mockup-bar">
        <span />
        <span />
        <span />
        <em>{m.title}</em>
      </div>
      <div className="mockup-body">
        <aside className="mockup-side">
          <i className="active" />
          <i />
          <i />
          <i />
          <i />
        </aside>
        <div className="mockup-main">
          <div className="mockup-kpis">
            <div>
              <small>{m.hits}</small>
              <b>270,778</b>
            </div>
            <div>
              <small>{m.urls}</small>
              <b>1,025</b>
            </div>
            <div>
              <small>{m.errors}</small>
              <b className="warn">4.6%</b>
            </div>
            <div>
              <small>{m.spoofed}</small>
              <b className="bad">29,106</b>
            </div>
          </div>
          <div className="mockup-panel">
            <small>{m.chart}</small>
            <div className="mockup-chart">
              {days.map((value, index) => (
                <span key={index} className="bar">
                  <span className="bar-search" style={{ height: `${value}%` }} />
                  <span className="bar-ai" style={{ height: `${ai[index]}%` }} />
                </span>
              ))}
            </div>
          </div>
          <div className="mockup-panel mockup-table">
            {bots.map(([name, hits, state]) => (
              <div key={name} className="mockup-row">
                <span>{name}</span>
                <span className="num">{hits.toLocaleString("en-US")}</span>
                <span className={`tag ${state}`}>{state === "ok" ? m.verified : m.fake}</span>
              </div>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}
