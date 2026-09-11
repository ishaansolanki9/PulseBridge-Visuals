// The controller illustration must not compete with live output or scale its
// workload with a high-DPI / maximized laptop display.
export function previewSize(width: number, height: number): [number, number] {
  const scale = Math.min(1, 640 / Math.max(width, 1), 360 / Math.max(height, 1));
  return [Math.max(1, Math.round(width * scale)), Math.max(1, Math.round(height * scale))];
}

export function startPreviewLoop(canvas: HTMLCanvasElement, render: (now: number, delta: number) => void): () => void {
  let frame = 0;
  let last = 0;
  let visible = true;
  let stopped = false;
  const tick = (now: number) => {
    frame = 0;
    if (stopped || !visible || document.hidden) return;
    if (now - last >= 1000 / 15) {
      const delta = last ? Math.min((now - last) / 1000, 0.1) : 0;
      last = now;
      render(now, delta);
    }
    if (!stopped) frame = requestAnimationFrame(tick);
  };
  const update = () => {
    cancelAnimationFrame(frame);
    frame = 0;
    last = 0;
    if (!stopped && visible && !document.hidden) frame = requestAnimationFrame(tick);
  };
  const observer = new IntersectionObserver(([entry]) => {
    visible = entry.isIntersecting;
    update();
  });
  observer.observe(canvas);
  document.addEventListener("visibilitychange", update);
  update();
  return () => {
    stopped = true;
    cancelAnimationFrame(frame);
    observer.disconnect();
    document.removeEventListener("visibilitychange", update);
  };
}
