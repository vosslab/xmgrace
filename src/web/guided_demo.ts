import { element } from "./dialog";

type Step = {
  name: string;
  text: string;
  hint?: "formula" | "fourier";
};

const hints = {
  formula:
    "Enter 0, 2*pi*127/128, 128, $t, and sin(5*$t)+0.5*sin(13*$t) for Start, Stop, Points, X, and Y. Then choose Accept.",
  fourier:
    "Select the 128-point signal as the input set. Keep Fast Fourier transform, Magnitude, and Index, then choose Accept. Peaks appear at bins 5 and 13.",
};

const steps: readonly Step[] = [
  {
    name: "Create a signal",
    text: `Open the formula editor from Edit > Data sets > Create new. ${hints.formula}`,
    hint: "formula",
  },
  {
    name: "Find its frequencies",
    text: `Open Fourier transforms under Data > Transformations. ${hints.fourier}`,
    hint: "fourier",
  },
  {
    name: "Save the project",
    text: "Use File > Save as to download your editable AGR project. Finish leaves this practice plot open for further exploration.",
  },
];

export function guidedDemoHint(step: "formula" | "fourier"): string | undefined {
  return document.body.dataset.guidedDemo === step ? hints[step] : undefined;
}

export type GuidedDemo = {
  open: () => void;
  start: () => void;
};

export function guidedDemo(): GuidedDemo {
  const panel = element<HTMLElement>("guided-demo");
  const title = element("guided-demo-title");
  const instruction = element("guided-demo-instruction");
  const progress = element("guided-demo-progress");
  const back = element<HTMLButtonElement>("guided-demo-back");
  const next = element<HTMLButtonElement>("guided-demo-next");
  const pause = element<HTMLButtonElement>("guided-demo-pause");
  const resume = element<HTMLButtonElement>("guided-demo-resume");
  const finish = element<HTMLButtonElement>("guided-demo-finish");
  let step = 0;
  let paused = false;

  function render(): void {
    const current = steps[step]!;
    document.body.dataset.guidedDemo = paused ? "paused" : (current.hint ?? "active");
    panel.hidden = false;
    title.textContent = current.name;
    instruction.textContent = current.text;
    progress.textContent = `Step ${step + 1} of ${steps.length}`;
    title.hidden = paused;
    instruction.hidden = paused;
    progress.hidden = paused;
    back.hidden = paused || step === 0;
    next.hidden = paused || step === steps.length - 1;
    pause.hidden = paused;
    resume.hidden = !paused;
    finish.hidden = paused || step !== steps.length - 1;
  }

  function start(): void {
    step = 0;
    paused = false;
    render();
  }

  back.addEventListener("click", () => {
    step = Math.max(0, step - 1);
    render();
    (step === 0 ? next : back).focus();
  });
  next.addEventListener("click", () => {
    step = Math.min(steps.length - 1, step + 1);
    render();
    (step === steps.length - 1 ? finish : next).focus();
  });
  pause.addEventListener("click", () => {
    paused = true;
    render();
    resume.focus();
  });
  resume.addEventListener("click", () => {
    paused = false;
    render();
    (step === steps.length - 1 ? finish : pause).focus();
  });
  finish.addEventListener("click", () => {
    panel.hidden = true;
    delete document.body.dataset.guidedDemo;
    element<HTMLElement>("canvas").focus();
  });

  return {
    open: () => window.open("?demo=1", "_blank", "noopener"),
    start,
  };
}
