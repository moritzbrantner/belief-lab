"""Offline acceptance for the single-command golden pipeline's caching and fail-closed behavior."""

import hashlib
import json
from pathlib import Path
import subprocess
import sys
from tempfile import TemporaryDirectory
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import russell_copleston_stage4 as stage  # noqa: E402


def write_json(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, sort_keys=True), encoding="utf-8")


class GoldenPipelineTests(unittest.TestCase):
    def setUp(self):
        self.temp = TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        tools = patch.object(stage, "required_tool", return_value=None)
        tools.start()
        self.addCleanup(tools.stop)
        self.root = Path(self.temp.name)
        self.out = self.root / stage.OUTPUT
        self.out.mkdir(parents=True)
        self.audio = self.root / stage.GOLDEN.parent / "audio/sample.webm"
        self.audio.parent.mkdir(parents=True)
        self.audio.write_bytes(b"fixture audio")
        self.audio_sha = hashlib.sha256(self.audio.read_bytes()).hexdigest()
        self.bundle_sha = "f" * 64
        write_json(
            self.audio.parent / "source.json",
            {"file": "sample.webm", "sha256": self.audio_sha},
        )
        write_json(
            self.root / stage.GOLDEN,
            {
                "status": "approved",
                "speakerTurnEvidence": {
                    "diarization": {"nativeRuntime": {"artifactSetSha256": self.bundle_sha}}
                },
            },
        )

    def stage13(self):
        write_json(
            self.out / "transcript.json",
            {"audioSha256": self.audio_sha, "modelId": "whisper-medium", "provider": "candle", "segments": [1]},
        )
        write_json(
            self.out / "speaker-turns.json",
            {"artifactSetSha256": self.bundle_sha, "modelId": "pyannote-community-1", "turns": [1]},
        )
        write_json(
            self.out / "stage4-document.json",
            {"id": stage.CASE, "text": "God exists", "segments": [1], "timing": [1]},
        )
        write_json(
            self.out / "report.json",
            {
                "stages": {
                    "acquire": {"audioSha256": self.audio_sha},
                    "transcribe": {"modelId": "whisper-medium", "provider": "candle"},
                    "speakerTurns": {"modelId": "pyannote-community-1", "artifactSetSha256": self.bundle_sha},
                }
            },
        )

    def test_gpu_command_enables_cuda_without_changing_test_selection(self):
        cpu = stage.stage13_command(gpu=False)
        gpu = stage.stage13_command(gpu=True)
        self.assertEqual(gpu[:5], cpu[:5])
        self.assertEqual(gpu[5:7], ["--features", "cuda"])
        self.assertEqual(gpu[7:], cpu[5:])
        self.assertNotIn("--features", cpu)

    def candidates(self):
        write_json(
            self.out / "stage4-candidates.json",
            {
                "documentSha256": stage.sha256(self.out / "stage4-document.json"),
                "producer": {"commit": stage.API_COMMIT, "llamaModelSha256": stage.LLAMA_MODEL_SHA256},
                "candidates": [{"canonicalText": "God exists"}],
            },
        )

    def test_source_audio_is_verified_before_reuse(self):
        self.assertEqual(stage.source_fingerprint(self.root), (self.audio_sha, self.bundle_sha))
        self.audio.write_bytes(b"changed")
        with self.assertRaisesRegex(stage.PipelineError, "digest mismatch"):
            stage.source_fingerprint(self.root)

    def test_stage13_cache_requires_matching_source_and_approved_bundle(self):
        self.stage13()
        self.assertTrue(stage.stage13_cached(self.out, self.audio_sha, self.bundle_sha))
        self.assertFalse(stage.stage13_cached(self.out, "0" * 64, self.bundle_sha))
        self.assertFalse(stage.stage13_cached(self.out, self.audio_sha, "0" * 64))
        (self.out / "stage4-document.json").write_text("{broken", encoding="utf-8")
        self.assertFalse(stage.stage13_cached(self.out, self.audio_sha, self.bundle_sha))
        self.stage13()
        write_json(self.out / "report.json", {"stages": ["malformed"]})
        self.assertFalse(stage.stage13_cached(self.out, self.audio_sha, self.bundle_sha))

    def test_extraction_cache_rejects_unpinned_or_stale_inputs(self):
        self.stage13()
        self.candidates()
        self.assertTrue(stage.candidates_cached(self.out))
        extraction = stage.json_or_none(self.out / "stage4-candidates.json")
        extraction["producer"]["commit"] = "unapproved"
        write_json(self.out / "stage4-candidates.json", extraction)
        self.assertFalse(stage.candidates_cached(self.out))
        extraction["producer"]["commit"] = stage.API_COMMIT
        extraction["producer"]["llamaModelSha256"] = "a" * 64
        write_json(self.out / "stage4-candidates.json", extraction)
        self.assertFalse(stage.candidates_cached(self.out))
        extraction["producer"]["llamaModelSha256"] = stage.LLAMA_MODEL_SHA256
        write_json(self.out / "stage4-candidates.json", extraction)
        write_json(self.out / "stage4-document.json", {"id": "changed"})
        self.assertFalse(stage.candidates_cached(self.out))
        extraction["producer"] = None
        write_json(self.out / "stage4-candidates.json", extraction)
        self.assertFalse(stage.candidates_cached(self.out))

    @patch.object(stage, "replay", return_value=0)
    @patch.object(stage, "command", side_effect=AssertionError("model stages must be skipped"))
    def test_complete_cached_run_only_replays(self, _command, replay):
        self.stage13()
        self.candidates()
        stage.run_pipeline(self.root, self.out, replay_only=False, api_dir=None, gpu=True)
        stage.run_pipeline(self.root, self.out, replay_only=False, api_dir=None)
        self.assertEqual(replay.call_count, 2)
        replay.assert_called_with(self.root, self.out)

    @patch.object(stage, "replay", return_value=0)
    @patch.object(stage, "command", side_effect=AssertionError("no model calls permitted"))
    def test_replay_only_works_without_original_model_stages(self, _command, replay):
        stage.run_pipeline(self.root, self.out, replay_only=True, api_dir=None)
        replay.assert_called_once_with(self.root, self.out)

    @patch.object(stage, "command", side_effect=AssertionError("must fail before model stage"))
    def test_fresh_run_fails_with_explicit_gated_bundle_prerequisite(self, _command):
        with patch.dict(stage.os.environ, {"GOLDEN_PYANNOTE_BUNDLE": ""}):
            with self.assertRaisesRegex(stage.PipelineError, "approved gated community-1"):
                stage.run_pipeline(self.root, self.out, replay_only=False, api_dir=None)

    @patch.object(stage, "replay", side_effect=[1, 0])
    @patch.object(stage, "command", return_value=101)
    def test_golden_acceptance_failure_does_not_discard_replayable_outputs(self, command, replay):
        self.stage13()
        self.candidates()
        stage.run_pipeline(self.root, self.out, replay_only=False, api_dir=None)
        self.assertEqual(replay.call_count, 2)
        self.assertEqual(command.call_count, 1)
        self.assertIn("russell_copleston_stage4", command.call_args.args[0])

    def test_managed_producer_checkout_is_pinned_and_reused(self):
        remote = self.root / "repository-source"
        remote.mkdir()
        subprocess.run(["git", "init", "-q", "-b", "main", str(remote)], check=True)
        (remote / "src").mkdir()
        (remote / "src/example.rs").write_text("// pinned", encoding="utf-8")
        subprocess.run(["git", "-C", str(remote), "add", "src/example.rs"], check=True)
        subprocess.run(
            ["git", "-C", str(remote), "-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid",
             "commit", "-qm", "source fixture"],
            check=True,
        )
        sha = subprocess.run(
            ["git", "-C", str(remote), "rev-parse", "HEAD"],
            capture_output=True,
            text=True,
            check=True,
        ).stdout.strip()
        with patch.object(stage, "API_URL", str(remote)), patch.object(stage, "API_COMMIT", sha):
            checkout = stage.ensure_api_checkout(self.root, self.out, None)
            self.assertTrue((checkout / "src/example.rs").is_file())
            with patch.object(stage, "command", side_effect=AssertionError("no clone or fetch")):
                self.assertEqual(stage.ensure_api_checkout(self.root, self.out, None), checkout)


if __name__ == "__main__":
    unittest.main()
