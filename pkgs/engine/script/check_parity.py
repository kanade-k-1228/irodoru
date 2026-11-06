from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np
import torch

from irodori_tts.inference_runtime import SamplingRequest
from models import load_runtime, MODELS
from irodori_tts.meanflow import sample_euler_meanflow
from irodori_tts.rf import sample_euler_rf_cfg

@torch.inference_mode()
def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--model", required=True)
    ap.add_argument("--dump", required=True)
    ap.add_argument("--caption", default=None)
    ap.add_argument("--voice-wav", default=None)
    args = ap.parse_args()

    dump = Path(args.dump)
    meta = json.loads((dump / "meta.json").read_text())
    steps = int(meta["latent_steps"])
    noise = torch.from_numpy(np.fromfile(dump / "noise.f32", dtype="<f4").reshape(1, steps, -1).copy())
    latent_rs = np.fromfile(dump / "latent.f32", dtype="<f4").reshape(1, steps, -1)

    rt = load_runtime(MODELS[args.model])
    text = meta["text"]
    ids_py = rt.tokenizer.encode(text).tolist()
    print("ids match:", ids_py == meta["ids"], len(ids_py))

    ids, mask = rt.tokenizer.batch_encode([text], max_length=rt.default_text_max_len)
    cap = args.caption or ""
    cids, cmask = rt.caption_tokenizer.batch_encode([cap], max_length=rt.default_caption_max_len)
    if not cap:
        cmask.zero_()
    req = SamplingRequest(text=text, ref_wav=args.voice_wav, no_ref=args.voice_wav is None)
    ref, ref_mask = rt._load_reference_latent(req=req, batch_size=1, messages=[])

    common = dict(model=rt.model, text_input_ids=ids, text_mask=mask, ref_latent=ref, ref_mask=ref_mask,
                  caption_input_ids=cids, caption_mask=cmask, sequence_length=steps,
                  num_steps=int(meta["num_steps"]), initial_noise=noise)
    if rt.model_cfg.flow_parameterization == "meanflow":
        z = sample_euler_meanflow(**common)
    else:
        z = sample_euler_rf_cfg(**common, cfg_scale_speaker=5.0 if args.voice_wav else 0.0)
    z = z.numpy()
    err = np.abs(z - latent_rs)
    print(f"latent shape={z.shape} max_abs_err={err.max():.3e} mean_abs_err={err.mean():.3e} "
          f"|z|max={np.abs(z).max():.2f} corr={np.corrcoef(z.ravel(), latent_rs.ravel())[0, 1]:.6f}")

    audio_path = dump / "audio.f32"
    if audio_path.exists():
        audio_rs = np.fromfile(audio_path, dtype="<f4")
        audio_py = rt.codec.decode_latent(torch.from_numpy(latent_rs.copy())).numpy().ravel()
        n = min(len(audio_rs), len(audio_py))
        aerr = np.abs(audio_rs[:n] - audio_py[:n])
        print(f"audio len rs={len(audio_rs)} py={len(audio_py)} max_abs_err={aerr.max():.3e} "
              f"|a|max={np.abs(audio_py).max():.3f}")


if __name__ == "__main__":
    main()
