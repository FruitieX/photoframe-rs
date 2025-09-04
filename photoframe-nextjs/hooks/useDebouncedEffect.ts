import { useEffect, useRef } from "react";

/**
 * Debounced effect with optional leading (rising edge) execution.
 * Executes the callback after `delayMs` of silence. If `leading` is true,
 * it will also run once immediately when dependencies change while a timer isn't active.
 */
type DebounceOptions = {
  leading?: boolean;
  trailing?: boolean;
  maxWait?: number; // ensure invocation at least every maxWait during continuous changes
};

export function useDebouncedEffect(
  cb: () => void,
  deps: unknown[],
  delayMs: number,
  leadingOrOptions: boolean | DebounceOptions = false,
) {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const pendingLeading = useRef(false);
  const didMount = useRef(false);
  const lastInvokeTs = useRef<number | null>(null);

  // Schedule on dependency changes; don't clear the timer in the cleanup of this effect,
  // otherwise the next render will think there's no active timer and will fire the leading call again.
  useEffect(() => {
    const options: DebounceOptions =
      typeof leadingOrOptions === "boolean"
        ? { leading: leadingOrOptions, trailing: true }
        : leadingOrOptions || {};
    const leading = !!options.leading;
    const trailing = options.trailing !== false; // default true
    const maxWait = options.maxWait;

    // Skip firing on initial mount; only respond to actual changes.
    if (!didMount.current) {
      didMount.current = true;
      if (timer.current) {
        clearTimeout(timer.current);
        timer.current = null;
      }
      pendingLeading.current = false;
      return;
    }

    const hadTimer = !!timer.current;
    const now = Date.now();

    if (leading) {
      if (!hadTimer) {
        // Rising edge: run immediately.
        cb();
        lastInvokeTs.current = now;
        pendingLeading.current = false;
      } else {
        // Another change during debounce window.
        pendingLeading.current = true;
      }
    }

    // maxWait: ensure periodic invocation while changes keep coming in
    if (maxWait && lastInvokeTs.current !== null) {
      if (now - lastInvokeTs.current >= maxWait) {
        cb();
        lastInvokeTs.current = now;
        pendingLeading.current = false;
      }
    }

    if (timer.current) {
      clearTimeout(timer.current);
    }
    timer.current = setTimeout(() => {
      if (trailing) {
        if (!leading) {
          cb();
          lastInvokeTs.current = Date.now();
        } else if (pendingLeading.current) {
          cb();
          lastInvokeTs.current = Date.now();
          pendingLeading.current = false;
        }
      }
      timer.current = null;
    }, delayMs);

    // No cleanup here; we handle clearing above and on unmount below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);

  // Clear any pending timer on unmount.
  useEffect(() => {
    return () => {
      if (timer.current) {
        clearTimeout(timer.current);
        timer.current = null;
      }
    };
  }, []);
}
