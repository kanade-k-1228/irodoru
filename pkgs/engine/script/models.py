from irodori_tts.inference_runtime import InferenceRuntime, RuntimeKey, download_hf_checkpoint

MODELS = {
    "small": "Aratako/Irodori-TTS-v4.1-Small",
    "mf": "Aratako/Irodori-TTS-v4.1-Small-MF",
    "anime": "phasefield-audio/Irodori-TTS-v4.1-Anime",
}


def load_runtime(repo: str) -> InferenceRuntime:
    ckpt = download_hf_checkpoint(repo)
    return InferenceRuntime.from_key(RuntimeKey(checkpoint=ckpt, model_device="cpu", codec_device="cpu"))
