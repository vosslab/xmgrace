import initialize, { Grace } from "../../generated/wasm/grace.js";
import type { Diagnostic, Message, Project, Request, Result } from "./protocol";

const ready = initialize().then(() => new Grace());

function diagnostic(error: unknown): Diagnostic {
  if (typeof error === "string") {
    try {
      return JSON.parse(error) as Diagnostic;
    } catch {
      return { message: error, line: null };
    }
  }
  return {
    message: error instanceof Error ? error.message : "Grace could not complete the operation.",
    line: null,
  };
}

self.addEventListener("message", (event: MessageEvent<Message<Request>>) => {
  void process(event.data);
});

async function process({ id, body }: Message<Request>): Promise<void> {
  const respond = (body: Result, transfers: Transferable[] = []): void => {
    self.postMessage({ id, body } satisfies Message<Result>, { transfer: transfers });
  };
  try {
    const grace = await ready;
    switch (body.type) {
      case "new":
        grace.new_project();
        break;
      case "open":
        grace.open_project(new Uint8Array(body.bytes));
        break;
      case "import":
        grace.import_data(body.graph, new Uint8Array(body.bytes));
        break;
      case "apply":
        grace.apply(JSON.stringify(body.command));
        break;
      case "save": {
        const bytes = new Uint8Array(grace.save_project());
        respond({ type: "save", bytes }, [bytes.buffer]);
        return;
      }
      case "state":
        break;
    }
    respond({
      type: "state",
      project: JSON.parse(grace.snapshot()) as Project,
      svg: grace.render_svg(),
    });
  } catch (error) {
    respond({ type: "error", diagnostic: diagnostic(error) });
  }
}
