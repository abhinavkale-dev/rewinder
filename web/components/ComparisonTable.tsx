const ROWS = [
  {
    label: "Disk usage after a day",
    recorder: "40&ndash;120 GB of footage",
    rewinder: "0 MB until you save",
  },
  {
    label: "Finding the moment",
    recorder: "Scrub hours of timeline",
    rewinder: "It is the whole clip",
  },
  {
    label: "Before it happened?",
    recorder: "Only if you pressed record",
    rewinder: "Always &mdash; that is the point",
  },
  {
    label: "Setup per session",
    recorder: "Pick source, start, stop",
    rewinder: "None &mdash; runs in the menu bar",
  },
  {
    label: "Sharing",
    recorder: "Trim, export, re-encode",
    rewinder: "MP4 ready to drag anywhere",
  },
];

export function ComparisonTable() {
  return (
    <section className="comparison" id="comparison">
      <h2>Screen recorders solve a different problem</h2>
      <p className="section-lede">
        Recording is for when you know something is about to happen. Rewinder
        is for when you did not.
      </p>
      <div className="compare-table" role="table">
        <div className="compare-head" role="row">
          <span role="columnheader" />
          <span role="columnheader">Traditional recorder</span>
          <span role="columnheader" className="compare-brand">
            Rewinder
          </span>
        </div>
        {ROWS.map((row) => (
          <div className="compare-row" role="row" key={row.label}>
            <span role="cell" className="compare-label">
              {row.label}
            </span>
            <span
              role="cell"
              className="compare-them"
              dangerouslySetInnerHTML={{ __html: row.recorder }}
            />
            <span
              role="cell"
              className="compare-us"
              dangerouslySetInnerHTML={{ __html: row.rewinder }}
            />
          </div>
        ))}
      </div>
    </section>
  );
}
