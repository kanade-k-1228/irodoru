from pathlib import Path

from huggingface_hub import hf_hub_download, snapshot_download
from irodori_tts.inference_runtime import InferenceRuntime, RuntimeKey

from license_resources import CODEC_REPO, revisions

MODELS = {
    "small": "Aratako/Irodori-TTS-v4.1-Small",
    "mf": "Aratako/Irodori-TTS-v4.1-Small-MF",
    "anime": "phasefield-audio/Irodori-TTS-v4.1-Anime",
}


def load_runtime(repo: str) -> InferenceRuntime:
    reviewed = revisions()
    snapshot = Path(snapshot_download(repo_id=repo, revision=reviewed[repo],
                                     allow_patterns=["model.safetensors", "tokenizer/*"]))
    codec = hf_hub_download(repo_id=CODEC_REPO, revision=reviewed[CODEC_REPO], filename="weights.pth")
    return InferenceRuntime.from_key(RuntimeKey(checkpoint=str(snapshot / "model.safetensors"),
                                               codec_repo=codec, model_device="cpu", codec_device="cpu"))
