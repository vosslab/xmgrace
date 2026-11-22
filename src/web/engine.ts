import type { Message, Request, Result } from "./protocol";

const worker = new Worker(new URL("./worker.js", import.meta.url), { type: "module" });
let sequence = 0;
let failure: Error | undefined;
const pending = new Map<
  number,
  { resolve: (value: Result) => void; reject: (error: Error) => void }
>();

worker.addEventListener("message", ({ data }: MessageEvent<Message<Result>>) => {
  pending.get(data.id)?.resolve(data.body);
  pending.delete(data.id);
});

worker.addEventListener("error", (event) => {
  failure = new Error(event.message || "Grace could not start its plotting engine.");
  for (const request of pending.values()) request.reject(failure);
  pending.clear();
});

export async function engine(body: Request): Promise<Exclude<Result, { type: "error" }>> {
  if (failure) throw failure;
  const id = ++sequence;
  const result = await new Promise<Result>((resolve, reject) => {
    pending.set(id, { resolve, reject });
    try {
      worker.postMessage(
        { id, body } satisfies Message<Request>,
        "bytes" in body ? [body.bytes] : [],
      );
    } catch (error) {
      pending.delete(id);
      reject(error instanceof Error ? error : new Error("Could not send a command to Grace."));
    }
  });
  if (result.type === "error") {
    const { line, message } = result.diagnostic;
    throw new Error(line === null ? message : `Line ${line}: ${message}`);
  }
  return result;
}
