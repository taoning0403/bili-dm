import { useEffect, useRef, useState } from "react";
import { playerService } from "../services/playerService";
import { getErrorMessage } from "../lib/desktop";
import type { PlaybackState, PlayerControl } from "../types/player";

export function usePlayback() {
  const [state, setState] = useState<PlaybackState | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");
  const [pollError, setPollError] = useState("");
  const inFlight = useRef(false);
  const revision = useRef(0);

  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      if (inFlight.current) {
        if (active) timer = setTimeout(() => void poll(), 1_000);
        return;
      }
      const startedAt = revision.current;
      try {
        const next = await playerService.state();
        if (active && startedAt === revision.current) { setState(next); setPollError(""); }
      } catch (cause) {
        if (active && startedAt === revision.current) setPollError(getErrorMessage(cause));
      }
      if (active) timer = setTimeout(() => void poll(), 1_000);
    }
    void poll();
    return () => { active = false; clearTimeout(timer); };
  }, []);

  async function perform(action: () => Promise<unknown>) {
    if (inFlight.current) return;
    inFlight.current = true;
    revision.current += 1;
    setBusy(true);
    setError("");
    try {
      await action();
      revision.current += 1;
      setState(await playerService.state());
    } catch (cause) { setError(getErrorMessage(cause)); }
    finally { inFlight.current = false; setBusy(false); }
  }

  return {
    state, busy, error: error || pollError,
    openLocal: () => perform(playerService.openLocal),
    playTorrent: (id: string, index: number) => perform(() => playerService.playTorrent(id, index)),
    control: (control: PlayerControl) => perform(() => playerService.control(control)),
  };
}
