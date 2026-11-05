#!/usr/bin/env python3
"""Resume the pinned Russell–Copleston golden pipeline and replay its diagnostics.

Requires only Python 3's standard library for orchestration. Missing stages need the
approved gated pyannote snapshot, pinned NLP bundles, Bun and llama.cpp from their
respective producers. Existing source-grounded outputs are reused without models.
"""

from __future__ import annotations

import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import time
from urllib.error import URLError
from urllib.request import urlopen


ROOT = Path(__file__).resolve().parents[1]
CASE = "russell-copleston-1948"
GOLDEN = Path("fixtures/golden") / CASE / "expected-beliefs.json"
OUTPUT = Path("target/golden") / CASE
API_COMMIT = "1b5ba66fa07120e05cd44980a18db01012294bf2"
API_URL = "https://github.com/moritzbrantner/proposition-api.git"
LLAMA_MODEL_SHA256 = "d98cdcbd03e17ce47681435b5150e34c1417f50b5c0019dd560e4882c5745785"


class PipelineError(Exception):
    """An actionable prerequisite or evidence-consistency failure."""


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def json_or_none(path: Path) -> dict | None:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
        return value if isinstance(value, dict) else None
    except (OSError, ValueError):
        return None


def mapping(value: object) -> dict:
    return value if isinstance(value, dict) else {}


def source_fingerprint(root: Path) -> tuple[str, str]:
    case = root / GOLDEN.parent
    manifest = json_or_none(case / "audio/source.json")
    golden = json_or_none(root / GOLDEN)
    if not manifest or not golden or golden.get("status") != "approved":
        raise PipelineError("missing or unapproved golden fixture / audio source manifest")
    filename = manifest.get("file")
    if not isinstance(filename, str) or Path(filename).name != filename:
        raise PipelineError("invalid golden source audio filename")
    audio = case / "audio" / filename
    if not audio.is_file() or sha256(audio) != manifest.get("sha256"):
        raise PipelineError(f"golden audio absent or digest mismatch: {audio}")
    try:
        bundle_sha = golden["speakerTurnEvidence"]["diarization"]["nativeRuntime"][
            "artifactSetSha256"
        ]
    except (KeyError, TypeError):
        raise PipelineError("golden fixture lacks the approved pyannote artifact digest") from None
    if not isinstance(bundle_sha, str) or len(bundle_sha) != 64:
        raise PipelineError("invalid approved pyannote bundle digest")
    return manifest["sha256"], bundle_sha


def stage13_cached(out: Path, audio_sha: str, bundle_sha: str) -> bool:
    transcript = json_or_none(out / "transcript.json")
    turns = json_or_none(out / "speaker-turns.json")
    document = json_or_none(out / "stage4-document.json")
    report = json_or_none(out / "report.json")
    stages = mapping(report.get("stages")) if report else {}
    transcribe = mapping(stages.get("transcribe"))
    diarization = mapping(stages.get("speakerTurns"))
    return bool(
        transcript
        and turns
        and document
        and report
        and transcript.get("audioSha256") == audio_sha
        and turns.get("artifactSetSha256") == bundle_sha
        and mapping(stages.get("acquire")).get("audioSha256") == audio_sha
        and diarization.get("artifactSetSha256") == bundle_sha
        and transcript.get("modelId") == transcribe.get("modelId")
        and transcript.get("provider") == transcribe.get("provider")
        and turns.get("modelId") == diarization.get("modelId")
        and transcript.get("modelId")
        and turns.get("modelId")
        and isinstance(transcript.get("segments"), list)
        and transcript["segments"]
        and isinstance(turns.get("turns"), list)
        and turns["turns"]
        and document.get("id") == CASE
        and isinstance(document.get("text"), str)
        and document["text"]
        and isinstance(document.get("segments"), list)
        and document["segments"]
        and isinstance(document.get("timing"), list)
        and document["timing"]
    )


def candidates_cached(out: Path) -> bool:
    document = out / "stage4-document.json"
    if not document.is_file():
        return False
    extraction = json_or_none(out / "stage4-candidates.json")
    return bool(
        extraction
        and extraction.get("documentSha256") == sha256(document)
        and mapping(extraction.get("producer")).get("commit") == API_COMMIT
        and mapping(extraction.get("producer")).get("llamaModelSha256") == LLAMA_MODEL_SHA256
        and isinstance(extraction.get("candidates"), list)
        and extraction["candidates"]
    )


def required_tool(name: str) -> None:
    if shutil.which(name) is None:
        raise PipelineError(f"{name} is not on PATH; install it before running this model stage")


def command(args: list[str], *, cwd: Path, env: dict[str, str] | None = None) -> int:
    print("+", " ".join(args), flush=True)
    return subprocess.run(args, cwd=cwd, env=env, check=False).returncode


def ensure_api_checkout(root: Path, out: Path, requested: Path | None) -> Path:
    required_tool("git")
    checkout = requested.resolve() if requested else out / ".producers/proposition-api"
    managed = requested is None
    if not (checkout / ".git").exists():
        if checkout.exists() and any(checkout.iterdir()):
            raise PipelineError(f"not an empty proposition-api checkout: {checkout}")
        if not managed:
            raise PipelineError(f"proposition-api checkout not found: {checkout}")
        checkout.mkdir(parents=True, exist_ok=True)
        for args in (["git", "init", "--quiet", "-b", "main", str(checkout)], ["git", "-C", str(checkout), "remote", "add", "origin", API_URL]):
            if command(args, cwd=root):
                raise PipelineError("could not initialize the pinned proposition-api checkout")
    head = subprocess.run(
        ["git", "-C", str(checkout), "rev-parse", "HEAD"],
        capture_output=True,
        text=True,
        check=False,
    ).stdout.strip()
    if head != API_COMMIT and managed:
        modified = subprocess.run(
            ["git", "-C", str(checkout), "status", "--porcelain"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout
        if modified.strip():
            raise PipelineError(f"managed proposition-api checkout has local edits: {checkout}")
        if command(["git", "-C", str(checkout), "fetch", "--depth=1", "origin", API_COMMIT], cwd=root):
            raise PipelineError(f"could not fetch pinned proposition-api {API_COMMIT}")
        if command(["git", "-C", str(checkout), "checkout", "--detach", API_COMMIT], cwd=root):
            raise PipelineError("could not check out the pinned proposition-api source")
        head = API_COMMIT
    if head != API_COMMIT:
        raise PipelineError(f"proposition-api is at {head or 'unknown'}, expected {API_COMMIT}")
    modified = subprocess.run(
        ["git", "-C", str(checkout), "status", "--porcelain", "--untracked-files=no", "--", "src"],
        capture_output=True,
        text=True,
        check=True,
    ).stdout
    if modified.strip():
        raise PipelineError("pinned proposition-api src/ has local edits")
    return checkout


def server_env(checkout: Path, environment: dict[str, str]) -> dict[str, str]:
    result = subprocess.run(
        [str(checkout / "scripts/llama-server.sh"), "--env"],
        cwd=checkout,
        env=environment,
        text=True,
        capture_output=True,
        check=False,
    )
    if result.returncode:
        raise PipelineError(result.stderr.strip() or "pinned Qwen3 model unavailable")
    prepared = environment.copy()
    for line in result.stdout.splitlines():
        key, separator, value = line.removeprefix("export ").partition("=")
        if not separator or not re.fullmatch(r"PROPOSITION_[A-Z0-9_]+", key):
            raise PipelineError("unexpected llama-server environment output")
        prepared[key] = value
    return prepared


@contextlib.contextmanager
def pinned_llama_server(checkout: Path, out: Path):
    required_tool("llama-server")
    environment = os.environ.copy()
    environment.setdefault("PROPOSITION_LLAMA_THREADS", environment.get("GOLDEN_THREADS", "4"))
    environment.setdefault("PROPOSITION_LLAMA_TIMEOUT_MS", "1800000")
    if "PROPOSITION_LLAMA_PORT" not in environment:
        with socket.socket() as address:
            address.bind(("127.0.0.1", 0))
            environment["PROPOSITION_LLAMA_PORT"] = str(address.getsockname()[1])
    prepared = server_env(checkout, environment)
    log_path = out / "llama-server.log"
    print(f"Starting pinned Qwen3 llama-server (log: {log_path})", flush=True)
    with log_path.open("a", encoding="utf-8") as log:
        process = subprocess.Popen(
            [str(checkout / "scripts/llama-server.sh")],
            cwd=checkout,
            env=prepared,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        try:
            url = prepared["PROPOSITION_LLAMA_URL"]
            deadline = time.monotonic() + 180
            while time.monotonic() < deadline:
                if process.poll() is not None:
                    raise PipelineError(f"llama-server exited; inspect {log_path}")
                try:
                    with urlopen(f"{url}/health", timeout=2) as response:
                        if response.status == 200:
                            break
                except (OSError, URLError):
                    pass
                time.sleep(1)
            else:
                raise PipelineError(f"llama-server did not become healthy; inspect {log_path}")
            yield prepared
        finally:
            process.terminate()
            try:
                process.wait(timeout=15)
            except subprocess.TimeoutExpired:
                process.kill()
                process.wait()


def replay(root: Path, out: Path) -> int:
    required_tool("cargo")
    return command(
        ["cargo", "run", "-p", "golden-scoring", "--bin", "stage4-diagnostics", "--", str(root / GOLDEN), str(out)],
        cwd=root,
    )


def stage13_command(*, gpu: bool) -> list[str]:
    args = ["cargo", "test", "--release", "--manifest-path", "crates/golden-media/Cargo.toml"]
    if gpu:
        args.extend(["--features", "cuda"])
    return args + ["--test", "russell_copleston", "--", "--ignored", "--nocapture"]


def run_pipeline(
    root: Path, out: Path, *, replay_only: bool, api_dir: Path | None, gpu: bool = False
) -> None:
    required_tool("cargo")
    out.mkdir(parents=True, exist_ok=True)
    # Only one runner can write a run's intermediates at a time.
    with (out / ".pipeline.lock").open("a+") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if replay_only:
            if replay(root, out):
                raise PipelineError("saved Stage 4 inputs are missing or stale; run without --replay-only")
            print("Offline diagnostics ready; no model inference was needed.")
            return

        audio_sha, bundle_sha = source_fingerprint(root)
        if stage13_cached(out, audio_sha, bundle_sha):
            print("Reuse: verified transcript, speaker turns and extraction document.")
        else:
            snapshot = os.environ.get("GOLDEN_PYANNOTE_BUNDLE")
            if not snapshot or not Path(snapshot).is_dir():
                raise PipelineError(
                    "GOLDEN_PYANNOTE_BUNDLE must point to the approved gated community-1 ONNX snapshot. "
                    "Prepare it with audio-analysis, accept the upstream model terms, then rerun."
                )
            device = "CUDA:0 (Whisper FP16)" if gpu else "CPU (Whisper FP32)"
            print(f"Compute: Whisper medium on {device} + pyannote community-1 (ONNX).")
            code = command(
                stage13_command(gpu=gpu),
                cwd=root,
            )
            if not stage13_cached(out, audio_sha, bundle_sha):
                raise PipelineError(f"transcription/diarization did not produce verified files (exit {code})")
            if code:
                print("Warning: transcription golden acceptance failed; evidence was still saved.")

        if candidates_cached(out):
            print("Reuse: pinned proposition-api candidates (matching document digest).")
        else:
            required_tool("bun")
            checkout = ensure_api_checkout(root, out, api_dir)
            with pinned_llama_server(checkout, out) as environment:
                code = command(
                    ["bun", str(root / "crates/golden-media/stage4/extract.ts"), str(checkout), str(out)],
                    cwd=root,
                    env=environment,
                )
            if code or not candidates_cached(out):
                raise PipelineError(f"pinned proposition extraction failed (exit {code})")

        if replay(root, out) == 0:
            print("Reuse: existing embeddings, NLI judgments and exact scoring baseline.")
        else:
            print("Compute: multilingual-e5-base + mDeBERTa NLI; model bundles must be locally materialized.")
            code = command(
                ["cargo", "test", "--release", "--manifest-path", "crates/golden-media/Cargo.toml", "--test", "russell_copleston_stage4", "--", "--ignored", "--nocapture"],
                cwd=root,
            )
            if replay(root, out):
                raise PipelineError(f"NLP scoring did not produce a replayable baseline (exit {code})")
            if code:
                print("Warning: Stage 4 golden acceptance failed; diagnostics were preserved for review.")
        print(f"Diagnostics: {out / 'attributions-diagnostics.md'}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--replay-only", action="store_true", help="never run/download models; use saved results")
    parser.add_argument(
        "--gpu",
        action="store_true",
        help="run Whisper medium on CUDA device 0 (requires a CUDA-capable Rust build; never silently falls back to CPU)",
    )
    parser.add_argument("--api-dir", type=Path, help="existing checkout of the exact pinned proposition-api commit")
    args = parser.parse_args()
    try:
        run_pipeline(ROOT, ROOT / OUTPUT, replay_only=args.replay_only, api_dir=args.api_dir, gpu=args.gpu)
    except (PipelineError, OSError, subprocess.SubprocessError) as error:
        print(f"Stage 4: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
