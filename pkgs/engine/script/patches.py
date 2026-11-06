from __future__ import annotations

from contextlib import contextmanager
from functools import cache

import torch

import irodori_tts.model as irodori_model
from irodori_tts import attention as irodori_attention


def rope_table(head_dim: int, seq_len_like: torch.Tensor, theta: float = 10000.0) -> torch.Tensor:
    freqs = 1.0 / (theta ** (torch.arange(0, head_dim, 2, dtype=torch.float32) / head_dim))
    pos = torch.arange(seq_len_like.shape[1], dtype=torch.float32)
    ang = pos[:, None] * freqs[None, :]
    return torch.stack([torch.cos(ang), torch.sin(ang)], dim=-1)


def apply_rotary_emb_real(x: torch.Tensor, table: torch.Tensor) -> torch.Tensor:
    shape = x.shape
    xf = x.float().reshape(shape[0], shape[1], shape[2], -1, 2)
    a = xf[..., 0]
    b = xf[..., 1]
    cos = table[None, :, None, :, 0]
    sin = table[None, :, None, :, 1]
    out = torch.stack([a * cos - b * sin, a * sin + b * cos], dim=-1)
    return out.reshape(shape).type_as(x)


def prefix_key_mask_attention_functional(q, k, v, key_mask):
    if key_mask is None:
        return irodori_attention._sdpa(q, k, v, None)
    first = key_mask[:, :1] | ~key_mask.any(dim=1, keepdim=True)
    safe_mask = torch.cat([first, key_mask[:, 1:]], dim=1)
    return irodori_attention._sdpa(q, k, v, safe_mask[:, None, None, :])


def snake_plain(x, alpha):
    shape = x.shape
    x = x.reshape(shape[0], shape[1], -1)
    x = x + (alpha + 1e-9).reciprocal() * torch.sin(alpha * x).pow(2)
    return x.reshape(shape)


SNAKE_DOMAIN = "ai.onnx.contrib"


class _SnakeFn(torch.autograd.Function):
    @staticmethod
    def forward(ctx, x, alpha):
        return snake_plain(x, alpha)

    @staticmethod
    def symbolic(g, x, alpha):
        return g.op(f"{SNAKE_DOMAIN}::Snake", x, alpha).setType(x.type())


def snake_fused(x, alpha):
    return _SnakeFn.apply(x, alpha)


@cache
def register_snake_pyop() -> str:
    import numpy as np
    from onnxruntime_extensions import PyCustomOpDef, get_library_path, onnx_op

    @onnx_op(op_type="Snake", inputs=[PyCustomOpDef.dt_float, PyCustomOpDef.dt_float],
             outputs=[PyCustomOpDef.dt_float])
    def _snake(x, alpha):
        x = x.astype(np.float64)
        a = alpha.astype(np.float64)
        return (x + np.reciprocal(a + 1e-9) * np.sin(a * x) ** 2).astype(np.float32)

    return get_library_path()


_ORIGINAL = {
    "apply_rotary_emb": irodori_model.apply_rotary_emb,
    "prefix_key_mask_attention": irodori_model.prefix_key_mask_attention,
}
_PATCHED = {
    "apply_rotary_emb": apply_rotary_emb_real,
    "prefix_key_mask_attention": prefix_key_mask_attention_functional,
}


def _use(funcs: dict) -> None:
    for k, v in funcs.items():
        setattr(irodori_model, k, v)


@contextmanager
def original_math():
    _use(_ORIGINAL)
    try:
        yield
    finally:
        _use(_PATCHED)


def apply_global_patches() -> None:
    _use(_PATCHED)

    import dacvae.nn.layers as dac_layers

    dac_layers.snake = snake_fused


def remove_weight_norms(module: torch.nn.Module) -> int:
    count = 0
    for m in module.modules():
        if hasattr(m, "weight_g") and hasattr(m, "weight_v"):
            torch.nn.utils.remove_weight_norm(m)
            count += 1
    return count
