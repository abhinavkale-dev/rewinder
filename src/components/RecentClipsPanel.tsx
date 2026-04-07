import type { ClipMetadataDto } from "../clipper/types";

type RecentClipsPanelProps = {
  recentClips: ClipMetadataDto[];
};

export function RecentClipsPanel({ recentClips }: RecentClipsPanelProps) {
  return (
    <section className="panel">
      <h2>Recent Clips</h2>
      {recentClips.length === 0 ? (
        <p>No clips saved yet.</p>
      ) : (
        <ul className="clip-list">
          {recentClips.map((clip) => (
            <li key={clip.id}>
              <div>
                <strong>{clip.id}</strong>
                <p>{clip.path}</p>
              </div>
              <div>
                <p>{clip.durationSecs.toFixed(2)}s</p>
                <p>{Math.round(clip.sizeBytes / 1024)} KB</p>
              </div>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
