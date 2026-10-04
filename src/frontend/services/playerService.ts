import { invokeDesktop } from "../lib/desktop";
import type { ActiveMedia, PlaybackState, PlayerControl, QueueOptions, VideoViewport } from "../types/player";
export const playerService = {
  openLocal: () => invokeDesktop<ActiveMedia | null>("open_local_video", undefined, 0),
  playTorrent: (torrentId: string, fileIndex: number) => invokeDesktop<ActiveMedia>("play_torrent", { torrentId, fileIndex }, 0),
  state: () => invokeDesktop<PlaybackState>("get_playback_state", undefined, 5_000),
  control: (control: PlayerControl) => invokeDesktop<void>("control_player", { control }, 0),
  selectQueue: (index: number) => invokeDesktop<ActiveMedia>("select_queue", { index }, 0),
  options: (options: QueueOptions) => invokeDesktop<void>("set_queue_options", { options }),
  subtitle: () => invokeDesktop<void>("open_subtitle", undefined, 0),
  viewport: (viewport: VideoViewport) => invokeDesktop<void>("set_video_viewport", { viewport }),
  fullscreen: (fullscreen: boolean) => invokeDesktop<void>("set_player_fullscreen", { fullscreen }),
};
