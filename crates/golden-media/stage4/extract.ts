// Stage 4 extraction for the Russell–Copleston golden run (belief-lab#63).
//
// Calls proposition-api's `llama-cpp` draft provider directly on the persisted
// `stage4-document.json` and writes the raw candidates, diagnostics and receipts.
// The provider is the pinned proposition-api commit (PRs #12, #14, #15 and #16, merged to main); a different or
// dirty checkout is refused. Each llama-server completion is cached by request digest, so an
// interrupted run resumes without recomputing finished windows (temperature 0, seed 0).
//
//   eval "$(<proposition-api>/scripts/llama-server.sh --env)"
//   bun crates/golden-media/stage4/extract.ts <proposition-api checkout> \
//     target/golden/russell-copleston-1948
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";

export const PROPOSITION_API_COMMIT = "d3613a0d0f281ce82c0087ecce743080a6bc5312";

const [apiDir, outDir] = process.argv.slice(2);
if (!apiDir || !outDir) {
  console.error("usage: bun extract.ts <proposition-api checkout> <golden output dir>");
  process.exit(1);
}
const git = (...args: string[]) => execFileSync("git", ["-C", apiDir, ...args], { encoding: "utf8" }).trim();
const head = git("rev-parse", "HEAD");
if (head !== PROPOSITION_API_COMMIT) {
  throw new Error(`proposition-api is at ${head}, expected pinned ${PROPOSITION_API_COMMIT}`);
}
if (git("status", "--porcelain", "--untracked-files=no", "--", "src") !== "") {
  throw new Error("proposition-api src/ has local changes; refusing to attribute them to the pinned commit");
}

const { LlamaCppPropositionProvider, llamaConfigFromEnv, LLAMA_DRAFT_PROMPT_VERSION, draftWindows } = await import(
  path.resolve(apiDir, "src/providers/llamaCpp.ts")
);

const documentPath = path.join(outDir, "stage4-document.json");
const documentBytes = readFileSync(documentPath);
const stage4 = JSON.parse(documentBytes.toString("utf8"));
const input = {
  document: { id: stage4.id, kind: "transcript", language: stage4.language, text: stage4.text, segments: stage4.segments },
};

const cacheDir = path.join(outDir, "llama-cache");
mkdirSync(cacheDir, { recursive: true });
const config = llamaConfigFromEnv();
const windows = draftWindows(input.document, config.maxWindowBytes ?? 2400).length;
let done = 0;
const started = Date.now();
const cachedFetch = (async (url: string | URL | Request, init?: RequestInit) => {
  const target = String(url);
  if (!target.endsWith("/v1/chat/completions")) {
    return fetch(url, init);
  }
  const body = String(init?.body ?? "");
  const file = path.join(cacheDir, `${createHash("sha256").update(body).digest("hex")}.json`);
  let text: string;
  if (existsSync(file)) {
    text = readFileSync(file, "utf8");
  } else {
    // Bun aborts a fetch after 300 s by default; a CPU draft window can take longer. The
    // provider's own AbortSignal (PROPOSITION_LLAMA_TIMEOUT_MS) still bounds the request.
    const response = await fetch(url, { ...init, timeout: false } as RequestInit);
    text = await response.text();
    if (!response.ok) {
      return new Response(text, { status: response.status });
    }
    writeFileSync(file, text);
  }
  done += 1;
  console.error(`window ${done}/${windows} after ${Math.round((Date.now() - started) / 1000)} s`);
  return new Response(text, { status: 200, headers: { "content-type": "application/json" } });
}) as typeof fetch;

const provider = new LlamaCppPropositionProvider({ ...config, fetch: cachedFetch });
const result = await provider.extract(input);
const output = {
  schema: "belief_lab_golden_stage4_candidates",
  schemaVersion: 1,
  producer: {
    repository: "https://github.com/moritzbrantner/proposition-api",
    commit: PROPOSITION_API_COMMIT,
    pullRequests: [12, 14, 15, 16], // merged to main
    provider: provider.id,
    promptVersion: LLAMA_DRAFT_PROMPT_VERSION,
    llamaModelFile: config.modelFile,
    llamaModelSha256: config.modelSha256,
    maxWindowBytes: config.maxWindowBytes ?? 2400,
  },
  documentSha256: createHash("sha256").update(documentBytes).digest("hex"),
  windows,
  seconds: (Date.now() - started) / 1000,
  candidates: result.candidates,
  diagnostics: result.diagnostics ?? [],
};
writeFileSync(path.join(outDir, "stage4-candidates.json"), `${JSON.stringify(output, null, 2)}\n`);
console.error(`${result.candidates.length} candidates, ${output.diagnostics.length} diagnostics`);
