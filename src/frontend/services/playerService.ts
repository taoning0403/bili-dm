import { invokeDesktop } from "../lib/desktop";
import type { ActiveMedia, PlaybackState, PlayerControl } from "../types/player";

export const playerService = {
  openLocal: () => invokeDesktop<ActiveMedia | null>("open_local_video", undefined, 0),
  playTorrent: (torrentId: string, fileIndex: number) =>
    invokeDesktop<ActiveMedia>("play_torrent", { torrentId, fileIndex }, 80_000),
  state: () => invokeDesktop<PlaybackState>("get_playback_state", undefined, 40_000),
  control: (control: PlayerControl) => invokeDesktop<void>("control_player", { control }, 15_000),
};
