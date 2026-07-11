const ITEMS = [
  { label: "Capturing", detail: "On", divider: false },
  { label: "Save last 2 minutes", detail: "\u2318\u21e7S", divider: true },
  { label: "Open clips folder", detail: "", divider: false },
  { label: "Settings\u2026", detail: "\u2318,", divider: true },
  { label: "Quit Rewinder", detail: "\u2318Q", divider: false },
];

export function TrayMenu() {
  return (
    <div className="tray-menu" aria-hidden>
      {ITEMS.map((item) => (
        <div key={item.label}>
          <div className="tray-item">
            <span>{item.label}</span>
            <span className="tray-detail">{item.detail}</span>
          </div>
          {item.divider && <div className="tray-divider" />}
        </div>
      ))}
    </div>
  );
}
