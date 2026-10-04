import { useEffect, useRef, useState } from "react";
import { playerService } from "../services/playerService";
import { getErrorMessage } from "../lib/desktop";
import type { PlaybackState, PlayerControl, QueueOptions } from "../types/player";
export function usePlayback() {
  const [state, setState] = useState<PlaybackState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [pollError, setPollError] = useState("");
  const [fullscreen, setFullscreen] = useState(false);
  const inFlight = useRef(false);
  const revision = useRef(0);
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      const ticket = revision.current;
      if (!inFlight.current) {
        try {
          const next = await playerService.state();
          if (active && ticket === revision.current) { setState(next); setFullscreen(next.fullscreen); setPollError(""); }
        } catch (cause) { if (active && ticket === revision.current) setPollError(getErrorMessage(cause)); }
      }
      if (active) timer = setTimeout(() => void poll(), 500);
    }
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, []);
  async function perform(action: () => Promise<unknown>, interrupt = false) {
    if (inFlight.current && !interrupt) return;
    const ticket = ++revision.current;
    inFlight.current = true; setBusy(true); setError("");
    try {
      await action();
      const next = await playerService.state();
      if (ticket === revision.current) { setState(next); setFullscreen(next.fullscreen); setPollError(""); }
    } catch (cause) { if (ticket === revision.current) setError(getErrorMessage(cause)); }
    finally { if (ticket === revision.current) { inFlight.current = false; setBusy(false); } }
  }
  async function toggleFullscreen(value = !fullscreen) {
    try { await playerService.fullscreen(value); setFullscreen(value); }
    catch (cause) { setError(getErrorMessage(cause)); }
  }
  return {
    state, busy, fullscreen, toggleFullscreen, error: error || pollError,
    openLocal: () => perform(playerService.openLocal),
    playTorrent: (id: string, index: number) => perform(() => playerService.playTorrent(id, index)),
    control: (control: PlayerControl) => perform(() => playerService.control(control), control.type === "stop"),
    selectQueue: (index: number) => perform(() => playerService.selectQueue(index)),
    options: (options: QueueOptions) => perform(() => playerService.options(options)),
    openSubtitle: () => perform(playerService.subtitle),
  };
}
