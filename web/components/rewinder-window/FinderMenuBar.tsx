const MENUS = ["Rewinder", "File", "Edit", "View", "Window", "Help"];

export function FinderMenuBar({ clock = "9:41 AM" }: { clock?: string }) {
  return (
    <div className="menu-bar">
      <div className="menu-bar-items">
        {MENUS.map((menu, i) => (
          <span key={menu} className={i === 0 ? "menu-item menu-item-app" : "menu-item"}>
            {menu}
          </span>
        ))}
      </div>
      <div className="menu-bar-clock">{clock}</div>
    </div>
  );
}
