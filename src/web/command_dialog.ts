import { paragraph, showDialog } from "./dialog";

let script = "";

export function commandDialog(execute: (input: string) => Promise<void>): void {
  const editor = document.createElement("textarea");
  editor.name = "script";
  editor.rows = 7;
  editor.value = script;
  editor.setAttribute("aria-label", "Grace commands");
  editor.spellcheck = false;
  const picker = document.createElement("input");
  picker.type = "file";
  picker.accept = ".bat,.txt,.cmd";
  picker.setAttribute("aria-label", "Open command file");
  const failure = paragraph("");
  failure.setAttribute("role", "alert");
  failure.hidden = true;
  picker.addEventListener("change", () => {
    const file = picker.files?.[0];
    picker.value = "";
    if (!file) return;
    failure.hidden = true;
    void file
      .text()
      .then((text) => {
        editor.value = text;
      })
      .catch(() => {
        failure.textContent = "Could not read the command file.";
        failure.hidden = false;
      });
  });
  showDialog(
    "Commands",
    [paragraph("Enter Grace commands or choose a local command file."), editor, picker, failure],
    async (form) => {
      const input = form.get("script");
      if (typeof input !== "string" || !input.trim()) throw new Error("Enter a Grace command.");
      await execute(input);
      script = input;
    },
  );
}
