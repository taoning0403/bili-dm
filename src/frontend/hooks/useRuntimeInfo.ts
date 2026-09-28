import { useEffect, useState } from "react";
import { getErrorMessage } from "../lib/desktop";
import { getRuntimeInfo } from "../services/runtimeService";
import type { RuntimeState } from "../types/runtime";

export function useRuntimeInfo() {
  const [attempt, setAttempt] = useState(0);
  const [state, setState] = useState<RuntimeState>({ status: "checking" });

  useEffect(() => {
    let active = true;

    void getRuntimeInfo().then(
      (info) => {
        if (active) setState({ status: "ready", info });
      },
      (error: unknown) => {
        if (active) setState({ status: "error", message: getErrorMessage(error) });
      },
    );

    return () => {
      active = false;
    };
  }, [attempt]);

  function refresh() {
    setState({ status: "checking" });
    setAttempt((value) => value + 1);
  }

  return { state, refresh };
}
