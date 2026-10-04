import type { ReactNode } from "react";

const icons = {
  play: <path d="m8 5 11 7-11 7Z" fill="currentColor" strokeLinejoin="round" />,
  pause: <><path d="M8 5v14M16 5v14" strokeWidth="4" /></>,
  previous: <><path d="M6 5v14" /><path d="m18 5-10 7 10 7Z" fill="currentColor" strokeLinejoin="round" /></>,
  next: <><path d="M18 5v14" /><path d="m6 5 10 7-10 7Z" fill="currentColor" strokeLinejoin="round" /></>,
  volume: <><path d="m11 5-5 4H3v6h3l5 4Z" fill="currentColor" strokeLinejoin="round" /><path d="M15 8a6 6 0 0 1 0 8m3-11a10 10 0 0 1 0 14" /></>,
  muted: <><path d="m11 5-5 4H3v6h3l5 4Z" fill="currentColor" strokeLinejoin="round" /><path d="m16 9 5 6m0-6-5 6" /></>,
  settings: <><path d="m9 3-.6 2.4-2.1 1.2L4 6l-2 3.5L3.7 11v2L2 14.5 4 18l2.3-.6 2.1 1.2L9 21h4l.6-2.4 2.1-1.2L18 18l2-3.5-1.7-1.5v-2L20 9.5 18 6l-2.3.6-2.1-1.2L13 3Z" transform="translate(1)" strokeLinejoin="round" /><circle cx="12" cy="12" r="3" /></>,
  fullscreen: <path d="M9 4H4v5m11-5h5v5M4 15v5h5m11-5v5h-5" />,
  exitFullscreen: <path d="M4 9h5V4m6 0v5h5M9 20v-5H4m11 5v-5h5" />,
  panel: <><rect x="3" y="4" width="18" height="16" rx="2" /><path d="M15 4v16m3-11v6" /></>,
  chevronLeft: <path d="m14 7-5 5 5 5" />,
  chevronRight: <path d="m10 7 5 5-5 5" />,
  close: <path d="m6 6 12 12M6 18 18 6" />,
  danmakuSettings: <><path d="M4 6h16M4 12h16M4 18h16" /><path d="M8 4v4m8 2v4m-6 2v4" strokeWidth="3" /></>,
} satisfies Record<string, ReactNode>;

export function PlayerIcon({ name }: { name: keyof typeof icons }) {
  return <svg className="player-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true" focusable="false">{icons[name]}</svg>;
}
