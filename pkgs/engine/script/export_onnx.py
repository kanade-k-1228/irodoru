from __future__ import annotations

import argparse
import json
from pathlib import Path

import torch
from irodori_tts.model import patch_sequence_with_mask

from graphs import (
    BackboneGraph,
    CodecDecoderGraph,
    CodecEncoderGraph,
    ContextGraph,
    DurationGraph,
    MeanFlowStepGraph,
    ProjectorGraph,
    RectifiedFlowStepGraph,
    SpeakerGraph,
)
from models import MODELS, InferenceRuntime, load_runtime
from license_resources import CODEC_REPO, write_notices
from onnx_io import export, verify
from patches import apply_global_patches, original_math, remove_weight_norms
from portable_onnx import make_portable, original_graphs


def mask(b, valid, total):
    m = torch.zeros(b, total, dtype=torch.bool)
    for i, v in enumerate(valid):
        m[i, :v] = True
    return m


def export_codec(rt: InferenceRuntime, out: Path):
    dac = rt.codec.model
    n = remove_weight_norms(dac)
    print(f"[codec] removed weight_norm from {n} modules")
    hop = int(dac.hop_length)
    sr = int(rt.codec.sample_rate)
    latent_dim = int(rt.codec.latent_dim)
    d = out / "codec"

    enc = CodecEncoderGraph(dac).eval()
    wav = torch.randn(1, 1, hop * 40) * 0.1
    export(enc, (wav,), d / "dacvae_encoder.onnx", ["wav"], ["latent"],
           {"wav": {0: "b", 2: "n"}, "latent": {0: "b", 1: "t"}})
    verify(enc, [(wav,), (torch.randn(1, 1, hop * 113) * 0.1,)], d / "dacvae_encoder.onnx", ["wav"])

    dec = CodecDecoderGraph(dac).eval()
    lat = torch.randn(1, 30, latent_dim)
    export(dec, (lat,), d / "dacvae_decoder.onnx", ["latent"], ["audio"],
           {"latent": {0: "b", 1: "t"}, "audio": {0: "b", 2: "n"}})
    verify(dec, [(lat,), (torch.randn(1, 77, latent_dim),)], d / "dacvae_decoder.onnx", ["latent"])

    for filename in ("dacvae_encoder.onnx", "dacvae_decoder.onnx"):
        make_portable(d / filename)

    (d / "codec.json").write_text(
        json.dumps({"sample_rate": sr, "hop_length": hop, "latent_dim": latent_dim}, indent=2)
    )
    write_notices(d, CODEC_REPO)


def export_tokenizer(rt: InferenceRuntime, d: Path):
    tok = rt.tokenizer
    splitter = tok.rare_token_splitter
    if splitter is not None:
        tok_json = splitter.tokenizer.to_str()
        forced = {str(k): v for k, v in splitter.forced.items()}
    else:
        tok_json = tok.tokenizer.backend_tokenizer.to_str()
        forced = {}
    (d / "tokenizer.json").write_text(tok_json, encoding="utf-8")
    cap_tok = rt.caption_tokenizer or tok
    (d / "caption_tokenizer.json").write_text(cap_tok.tokenizer.backend_tokenizer.to_str(), encoding="utf-8")
    (d / "forced_splits.json").write_text(json.dumps(forced), encoding="utf-8")
    return {
        "bos_token_id": int(tok.bos_token_id),
        "pad_token_id": int(tok.pad_token_id),
        "add_bos": bool(tok.add_bos),
        "caption_add_bos": bool(rt.caption_tokenizer.add_bos) if rt.caption_tokenizer else True,
    }


def export_model(name: str, rt: InferenceRuntime, out: Path):
    m = rt.model.eval()
    cfg = m.cfg
    d = out / name
    d.mkdir(parents=True, exist_ok=True)
    meanflow = str(cfg.flow_parameterization).lower() == "meanflow"
    print(f"[{name}] flow={'meanflow' if meanflow else 'rf'}")

    tok_info = export_tokenizer(rt, d)

    bb = BackboneGraph(m.pretrained_text_backbone).eval()
    ids = torch.randint(5, 1000, (2, 24))
    ids[:, 0] = tok_info["bos_token_id"]
    mk = mask(2, [24, 13], 24)
    export(bb, (ids, mk), d / "backbone.onnx", ["input_ids", "mask"], ["hidden"],
           {"input_ids": {0: "b", 1: "s"}, "mask": {0: "b", 1: "s"}, "hidden": {0: "b", 1: "s"}})
    ids2 = torch.randint(5, 1000, (1, 200))
    verify(bb, [(ids, mk), (ids2, mask(1, [200], 200))], d / "backbone.onnx", ["input_ids", "mask"])

    hidden = bb(ids, mk).detach()

    for pname, proj, norm in (
        ("text_proj", m.text_encoder, m.text_norm),
        ("caption_proj", m.caption_encoder, m.caption_norm),
    ):
        pw = ProjectorGraph(proj, norm).eval()
        export(pw, (hidden, mk), d / f"{pname}.onnx", ["hidden", "mask"], ["state"],
               {"hidden": {0: "b", 1: "s"}, "mask": {0: "b", 1: "s"}, "state": {0: "b", 1: "s"}})
        verify(pw, [(hidden, mk)], d / f"{pname}.onnx", ["hidden", "mask"])

    p = int(cfg.speaker_patch_size)
    sw = SpeakerGraph(m).eval()
    ref = torch.randn(1, p * 50, cfg.latent_dim)
    rmask = torch.ones(1, p * 50, dtype=torch.bool)
    export(sw, (ref, rmask), d / "speaker_encoder.onnx", ["ref_latent", "ref_mask"], ["speaker_state"],
           {"ref_latent": {0: "b", 1: "t"}, "ref_mask": {0: "b", 1: "t"}, "speaker_state": {0: "b", 1: "s"}})
    verify(sw, [(ref, rmask), (torch.randn(1, p * 7, cfg.latent_dim), torch.ones(1, p * 7, dtype=torch.bool))],
           d / "speaker_encoder.onnx", ["ref_latent", "ref_mask"])

    def upstream_speaker(lat, lmask):
        x, mk2 = patch_sequence_with_mask(lat, lmask, p)
        with original_math():
            st = m.speaker_norm(m.speaker_encoder(x, mk2))
        return m._prepend_masked_mean_token(st, mk2)[0]

    verify(upstream_speaker, [(ref, rmask)], d / "speaker_encoder.onnx", ["ref_latent", "ref_mask"])

    text_state = ProjectorGraph(m.text_encoder, m.text_norm)(hidden, mk).detach()
    cap_ids = torch.randint(5, 1000, (2, 10))
    cap_mask = mask(2, [10, 0], 10)
    cap_state = ProjectorGraph(m.caption_encoder, m.caption_norm)(bb(cap_ids, cap_mask), cap_mask).detach()
    spk_state = sw(torch.randn(2, p * 12, cfg.latent_dim), torch.ones(2, p * 12, dtype=torch.bool)).detach()
    spk_mask = torch.ones(2, spk_state.shape[1], dtype=torch.bool)
    spk_mask[1] = False

    dw = DurationGraph(m).eval()
    has_spk = torch.tensor([True, False])
    has_cap = torch.tensor([True, False])
    dargs = (text_state, mk, spk_state, has_spk, cap_state, cap_mask, has_cap)
    dnames = ["text_state", "text_mask", "speaker_state", "has_speaker", "caption_state", "caption_mask", "has_caption"]
    export(dw, dargs, d / "duration.onnx", dnames, ["log_frames"],
           {"text_state": {0: "b", 1: "s"}, "text_mask": {0: "b", 1: "s"},
            "speaker_state": {0: "b", 1: "sp"}, "has_speaker": {0: "b"},
            "caption_state": {0: "b", 1: "c"}, "caption_mask": {0: "b", 1: "c"},
            "has_caption": {0: "b"}, "log_frames": {0: "b"}})
    verify(dw, [dargs], d / "duration.onnx", dnames)

    cw = ContextGraph(m).eval()
    cargs = (text_state, spk_state, cap_state)
    knames = ["k_text", "v_text", "k_speaker", "v_speaker", "k_caption", "v_caption"]
    kaxes = {"k_text": {1: "b", 2: "s"}, "v_text": {1: "b", 2: "s"},
             "k_speaker": {1: "b", 2: "sp"}, "v_speaker": {1: "b", 2: "sp"},
             "k_caption": {1: "b", 2: "c"}, "v_caption": {1: "b", 2: "c"}}
    export(cw, cargs, d / "dit_context.onnx", ["text_state", "speaker_state", "caption_state"], knames,
           {"text_state": {0: "b", 1: "s"}, "speaker_state": {0: "b", 1: "sp"},
            "caption_state": {0: "b", 1: "c"}, **kaxes})
    verify(cw, [cargs], d / "dit_context.onnx", ["text_state", "speaker_state", "caption_state"])
    ctx = tuple(t.detach() for t in cw(*cargs))

    x_t = torch.randn(2, 37, cfg.patched_latent_dim)
    t = torch.tensor([0.9, 0.3])
    if meanflow:
        stw = MeanFlowStepGraph(m).eval()
        sargs = (x_t, t, torch.tensor([0.25, 0.25]), mk, spk_mask, cap_mask, *ctx)
        snames = ["x_t", "t", "delta_t", "text_mask", "speaker_mask", "caption_mask", *knames]
    else:
        stw = RectifiedFlowStepGraph(m).eval()
        sargs = (x_t, t, mk, spk_mask, cap_mask, *ctx)
        snames = ["x_t", "t", "text_mask", "speaker_mask", "caption_mask", *knames]
    saxes = {"x_t": {0: "b", 1: "n"}, "t": {0: "b"}, "delta_t": {0: "b"},
             "text_mask": {0: "b", 1: "s"}, "speaker_mask": {0: "b", 1: "sp"},
             "caption_mask": {0: "b", 1: "c"}, **kaxes, "v": {0: "b", 1: "n"}}
    export(stw, sargs, d / "dit_step.onnx", snames, ["v"], {k: v for k, v in saxes.items() if k in snames + ["v"]})

    def upstream(*a):
        if meanflow:
            x, tt, dt, tm, sm, cm, *c = a
        else:
            x, tt, tm, sm, cm, *c = a
            dt = None
        cache = [tuple(ci[i] for ci in c) for i in range(len(m.blocks))]
        with original_math():
            return m.forward_with_encoded_conditions(
                x_t=x, t=tt, delta_t=dt, text_state=text_state, text_mask=tm,
                speaker_state=spk_state, speaker_mask=sm, caption_state=cap_state,
                caption_mask=cm, context_kv_cache=cache,
            )

    verify(upstream, [sargs], d / "dit_step.onnx", snames)

    manifest = {
        "name": name,
        "checkpoint": MODELS.get(name, name),
        "flow": "meanflow" if meanflow else "rf",
        "latent_dim": int(cfg.latent_dim),
        "latent_patch_size": int(cfg.latent_patch_size),
        "speaker_patch_size": p,
        "speaker_dim": int(cfg.speaker_dim),
        "text_dim": int(cfg.text_dim),
        "caption_dim": int(cfg.caption_dim_resolved),
        "num_layers": len(m.blocks),
        "max_text_len": int(rt.default_text_max_len),
        "max_caption_len": int(rt.default_caption_max_len),
        "max_ref_seconds": float(rt.default_max_ref_seconds),
        "default_num_steps": 4 if meanflow else 40,
        **tok_info,
    }
    (d / "manifest.json").write_text(json.dumps(manifest, indent=2, ensure_ascii=False))
    write_notices(d, MODELS[name])
    for path in original_graphs(d):
        make_portable(path)
    print(f"[{name}] manifest: {manifest}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default=str(Path(__file__).resolve().parent.parent.parent.parent / "models"))
    ap.add_argument("--models", default="small,mf,anime")
    ap.add_argument("--skip-codec", action="store_true")
    ap.add_argument("--codec-only", action="store_true", help="re-export only the DACVAE codec")
    args = ap.parse_args()
    apply_global_patches()
    out = Path(args.out).resolve()
    torch.set_grad_enabled(False)

    names = [n.strip() for n in args.models.split(",") if n.strip()]
    if args.codec_only:
        export_codec(load_runtime(MODELS[names[0]]), out)
        print("done")
        return
    codec_done = args.skip_codec
    for name in names:
        repo = MODELS.get(name)
        if repo is None:
            raise SystemExit(f"unknown model {name!r}; choose from {sorted(MODELS)}")
        print(f"=== {name}: {repo}")
        rt = load_runtime(repo)
        export_model(name, rt, out)
        if not codec_done:
            export_codec(rt, out)
            codec_done = True
        del rt
    print("done")


if __name__ == "__main__":
    main()
