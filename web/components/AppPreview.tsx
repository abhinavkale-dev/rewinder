import { RewinderWindow } from "./rewinder-window/RewinderWindow";

export function AppPreview() {
  return (
    <div className="app-preview">
      <div className="app-preview-glow" />
      <RewinderWindow />
    </div>
  );
}
