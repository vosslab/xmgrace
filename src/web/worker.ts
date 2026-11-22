import initialize, { Grace } from "../../generated/wasm/grace.js";
import type { Diagnostic, Message, Project, Request, Result } from "./protocol";

const ready = initialize().then(() => new Grace());
let revision = 0;

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
      case "importMapped":
        grace.import_mapped(body.graph, new Uint8Array(body.bytes), JSON.stringify(body.options));
        break;
      case "reloadData":
        grace.reload_data(body.graph, body.set, new Uint8Array(body.bytes));
        break;
      case "loadParameters":
        grace.load_parameters(body.graph, new Uint8Array(body.bytes));
        break;
      case "saveParameters": {
        const bytes = new Uint8Array(grace.save_parameters(body.graph));
        respond({ type: "parameters", bytes }, [bytes.buffer]);
        return;
      }
      case "netcdfVariables": {
        const variables = JSON.parse(grace.netcdf_variables(new Uint8Array(body.bytes))) as [
          string,
          number,
        ][];
        respond({ type: "netcdfVariables", variables });
        return;
      }
      case "importNetcdf":
        grace.import_netcdf(body.graph, new Uint8Array(body.bytes), body.x, body.y);
        break;
      case "apply":
        if (
          body.command.type === "replaceData" &&
          body.command.expectedRevision !== undefined &&
          body.command.expectedRevision !== revision
        )
          throw new Error(
            "The project changed while the spreadsheet was open. Reopen the set to edit it.",
          );
        grace.apply(JSON.stringify(body.command));
        break;
      case "script":
        grace.run_script(body.graph, body.input);
        break;
      case "save": {
        const bytes = new Uint8Array(grace.save_project());
        respond({ type: "save", bytes }, [bytes.buffer]);
        return;
      }
      case "exportAscii": {
        const bytes = new Uint8Array(grace.export_ascii(body.graph, body.set));
        respond({ type: "ascii", bytes }, [bytes.buffer]);
        return;
      }
      case "dataTable": {
        const rows = JSON.parse(
          grace.data_table(body.graph, body.set, body.offset, body.limit),
        ) as number[][];
        respond({ type: "dataTable", rows, revision });
        return;
      }
      case "regionReport": {
        const counts = JSON.parse(grace.region_report(body.graph, body.id)) as [number, number][];
        respond({ type: "regionReport", sets: counts.map(([set, count]) => ({ set, count })) });
        return;
      }
      case "state":
        break;
    }
    if (body.type !== "state") revision++;
    respond({
      type: "state",
      project: JSON.parse(grace.snapshot()) as Project,
      svg: grace.render_svg(),
    });
  } catch (error) {
    respond({ type: "error", diagnostic: diagnostic(error) });
  }
}
