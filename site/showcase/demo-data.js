window.__BELIEF_DEMO__ = {
  "claim": {
    "label": "person:alice uses product:thinkpad",
    "score": 0.88,
    "semantics": "heuristic_strength"
  },
  "summary": [
    {
      "depth": 0,
      "label": "relation: voice-track-2 uses product:thinkpad",
      "kind": "relation",
      "score": "0.88 ner_confidence",
      "producer": "nlp-stack"
    },
    {
      "depth": 1,
      "label": "\"I've been using this ThinkPad with Arch for about two years.\"",
      "kind": "source_span",
      "producer": "youtube-corpus"
    },
    {
      "depth": 1,
      "label": "scene_text · ThinkPad X1",
      "kind": "ocr_track",
      "producer": "visual-analysis",
      "correlation": "ocr-track:ocr-track-1"
    },
    {
      "depth": 0,
      "label": "identity: voice-track-2 same_entity person:alice",
      "kind": "entity_link",
      "score": "0.91 entity_link_probability",
      "producer": "youtube-corpus"
    },
    {
      "depth": 1,
      "label": "face-track-4 visible_in scene-17",
      "kind": "face_track",
      "score": "0.90 detection_confidence",
      "producer": "visual-analysis"
    }
  ],
  "full": [
    {
      "depth": 0,
      "label": "relation: voice-track-2 uses product:thinkpad",
      "kind": "relation",
      "score": "0.88 ner_confidence",
      "producer": "nlp-stack"
    },
    {
      "depth": 1,
      "label": "\"I've been using this ThinkPad with Arch for about two years.\"",
      "kind": "source_span",
      "producer": "youtube-corpus"
    },
    {
      "depth": 1,
      "label": "named entity: span mentions product:thinkpad",
      "kind": "named_entity",
      "score": "0.91 ner_confidence",
      "producer": "nlp-stack",
      "correlation": "transcript-semantics-span-1"
    },
    {
      "depth": 1,
      "label": "scene_text · ThinkPad X1",
      "kind": "ocr_track",
      "producer": "visual-analysis",
      "correlation": "ocr-track:ocr-track-1"
    },
    {
      "depth": 2,
      "label": "OCR observation · ThinkPad X1",
      "kind": "ocr_observation",
      "score": "0.88 detection_confidence",
      "producer": "visual-analysis"
    },
    {
      "depth": 2,
      "label": "scene 17 · 750.00s–767.00s",
      "kind": "scene",
      "producer": "scenedetect-rs"
    },
    {
      "depth": 0,
      "label": "identity: voice-track-2 same_entity person:alice",
      "kind": "entity_link",
      "score": "0.91 entity_link_probability",
      "producer": "youtube-corpus",
      "correlation": "cross-modal-identity-alice"
    },
    {
      "depth": 1,
      "label": "voice-track-2 speaks_in scene-17",
      "kind": "voice_track",
      "score": "0.92 transcription_confidence",
      "producer": "audio-analysis",
      "correlation": "speaker-turn-752"
    },
    {
      "depth": 2,
      "label": "\"I've been using this ThinkPad with Arch for about two years.\"",
      "kind": "source_span",
      "producer": "youtube-corpus"
    },
    {
      "depth": 2,
      "label": "scene 17 · 750.00s–767.00s",
      "kind": "scene",
      "producer": "scenedetect-rs"
    },
    {
      "depth": 1,
      "label": "face-track-4 visible_in scene-17",
      "kind": "face_track",
      "score": "0.90 detection_confidence",
      "producer": "visual-analysis",
      "correlation": "person-presence-scene-17"
    }
  ]
};
