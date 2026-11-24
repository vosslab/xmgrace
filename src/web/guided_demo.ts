import { element } from "./dialog";
import type { Command, Graph } from "./protocol";

type Context = {
  apply: (command: Command) => Promise<void>;
  graph: () => Graph | undefined;
  ready: () => boolean;
};

export function guidedDemo(context: Context): {
  open: () => void;
  start: () => void;
  stop: () => void;
} {
  const panel = element("guided-demo");
  const title = element("guided-demo-title");
  const instruction = element("guided-demo-instruction");
  const pause = element<HTMLButtonElement>("guided-demo-pause");
  const resume = element<HTMLButtonElement>("guided-demo-resume");
  const dismiss = element<HTMLButtonElement>("guided-demo-finish");
  let timer = 0;
  let signal: string | undefined;
  let complete = false;
  let failure: string | undefined;
  let paused = false;
  let running = false;

  function render(): void {
    const finished = complete || failure !== undefined;
    const compact = paused && !finished;
    const moveFocus =
      finished && [pause, resume].includes(document.activeElement as HTMLButtonElement);
    panel.classList.toggle("demo-paused", compact);
    title.textContent = failure
      ? "Demo stopped"
      : complete
        ? "Fourier spectrum"
        : signal === undefined
          ? "Creating a signal"
          : "Two-tone signal";
    instruction.textContent =
      failure ??
      (complete
        ? "Use the plot controls to explore the result."
        : signal === undefined
          ? "Generating 128 samples."
          : "Next, Grace calculates the Fourier spectrum.");
    title.hidden = compact;
    instruction.hidden = compact;
    pause.hidden = paused || finished;
    resume.hidden = !compact;
    dismiss.textContent = finished ? "Keep exploring" : "Stop";
    if (moveFocus) dismiss.focus();
  }

  function schedule(): void {
    if (panel.hidden || paused || running || complete || failure !== undefined) return;
    timer = window.setTimeout(() => void advance(), signal === undefined ? 1000 : 2500);
  }

  function suspend(): void {
    if (panel.hidden || paused || complete || failure !== undefined) return;
    window.clearTimeout(timer);
    paused = true;
    render();
  }

  async function advance(): Promise<void> {
    if (!context.ready()) {
      suspend();
      return;
    }
    running = true;
    try {
      if (signal === undefined) {
        const previousSets = context.graph()?.sets ?? {};
        await context.apply({
          type: "generate",
          graph: 0,
          start: "0",
          stop: "2*pi*127/128",
          points: 128,
          x: "$t",
          y: "sin(5*$t)+0.5*sin(13*$t)",
        });
        signal = Object.keys(context.graph()?.sets ?? {}).find((id) => !(id in previousSets));
        if (signal === undefined) throw new Error("The demo signal could not be created.");
      } else {
        if (context.graph()?.sets[signal]?.count !== 128)
          throw new Error("The demo signal is no longer available.");
        await context.apply({
          type: "fourier",
          graph: 0,
          set: Number(signal),
          options: {
            algorithm: "fft",
            input: "real",
            direction: "forward",
            output: "magnitude",
            abscissa: "index",
            window: "none",
          },
        });
        complete = true;
      }
    } catch (error) {
      failure = error instanceof Error ? error.message : "The demo could not continue.";
    } finally {
      running = false;
      if (!panel.hidden) render();
      schedule();
    }
  }

  function interrupt(event: PointerEvent): void {
    if (event.target instanceof Node && !panel.contains(event.target)) suspend();
  }

  function stop(): void {
    window.clearTimeout(timer);
    document.removeEventListener("click", interrupt, true);
    panel.hidden = true;
  }

  pause.addEventListener("click", () => {
    suspend();
    resume.focus();
  });
  resume.addEventListener("click", () => {
    paused = false;
    render();
    pause.focus();
    schedule();
  });
  dismiss.addEventListener("click", () => {
    stop();
    element("canvas").focus();
  });

  return {
    open: () => window.open("?demo=1", "_blank", "noopener"),
    start: (): void => {
      panel.hidden = false;
      document.addEventListener("click", interrupt, true);
      render();
      schedule();
    },
    stop,
  };
}
