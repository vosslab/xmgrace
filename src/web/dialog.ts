export function element<T extends HTMLElement>(id: string): T {
  const found = document.getElementById(id);
  if (!found) throw new Error(`Missing interface element ${id}`);
  return found as T;
}

type Submit = (data: FormData) => void | Promise<void>;
const dialog = element<HTMLDialogElement>("properties");
const form = element<HTMLFormElement>("dialog-form");
const content = element("dialog-content");
const error = element("dialog-error");
let submit: Submit | undefined;

form.addEventListener("submit", async (event) => {
  event.preventDefault();
  error.hidden = true;
  try {
    await submit?.(new FormData(form));
    if (!(event.submitter instanceof HTMLButtonElement) || event.submitter.value !== "apply")
      dialog.close();
  } catch (failure) {
    error.textContent =
      failure instanceof Error ? failure.message : "Grace could not apply these settings.";
    error.hidden = false;
  }
});

export function showDialog(title: string, controls: HTMLElement[], onSubmit?: Submit): void {
  element("dialog-title").textContent = title;
  content.replaceChildren(...controls);
  error.hidden = true;
  submit = onSubmit;
  for (const button of form.querySelectorAll<HTMLButtonElement>('button[type="submit"]'))
    button.hidden = !submit;
  if (!dialog.open) dialog.showModal();
}

export function closeDialog(): void {
  dialog.close();
}

export function field(
  label: string,
  control: HTMLInputElement | HTMLSelectElement,
): HTMLLabelElement {
  const wrapper = document.createElement("label");
  wrapper.append(label, control);
  return wrapper;
}

export function input(name: string, value: string, type = "text"): HTMLInputElement {
  const control = document.createElement("input");
  control.name = name;
  control.type = type;
  control.value = value;
  if (type === "number") {
    control.step = "any";
    control.required = true;
  }
  return control;
}

export function select(
  name: string,
  pairs: readonly (readonly [string, string])[],
  value: string,
): HTMLSelectElement {
  const control = document.createElement("select");
  control.name = name;
  for (const [id, label] of pairs) control.add(new Option(label, id, false, id === value));
  return control;
}

export function paragraph(text: string): HTMLParagraphElement {
  const p = document.createElement("p");
  p.textContent = text;
  return p;
}
