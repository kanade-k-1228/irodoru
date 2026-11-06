import torch
import torch.nn as nn
import torch.nn.functional as F
from irodori_tts.attention import build_context_attention_plan
from irodori_tts.model import get_timestep_embedding, patch_sequence_with_mask

from patches import rope_table

class BackboneGraph(nn.Module):
    def __init__(self, backbone):
        super().__init__()
        self.backbone = backbone

    def forward(self, input_ids, mask):
        return self.backbone(input_ids, mask.bool())


class ProjectorGraph(nn.Module):
    def __init__(self, proj, norm):
        super().__init__()
        self.proj = proj
        self.norm = norm

    def forward(self, hidden, mask):
        p = self.proj
        out = p.projector(hidden)
        if p.projector_type == "residual_mlp":
            r = F.silu(p.residual_up(p.residual_norm(hidden)))
            out = out + p.residual_down(r)
        out = out * mask.unsqueeze(-1).to(out.dtype)
        return self.norm(out)


class SpeakerGraph(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.enc = model.speaker_encoder
        self.norm = model.speaker_norm
        self.patch = int(model.cfg.speaker_patch_size)

    def forward(self, ref_latent, ref_mask):
        x, mask = patch_sequence_with_mask(ref_latent, ref_mask.bool(), self.patch)
        enc = self.enc
        h = enc.in_proj(x) / 6.0
        mask_f = mask.unsqueeze(-1).to(h.dtype)
        h = h * mask_f
        table = rope_table(enc.head_dim, h)
        for block in enc.blocks:
            h = block(h, mask=mask, freqs_cis=table)
            h = h * mask_f
        h = self.norm(h * mask_f)
        denom = mask_f.sum(dim=1, keepdim=True).clamp_min(1.0)
        mean_tok = (h * mask_f).sum(dim=1, keepdim=True) / denom
        return torch.cat([mean_tok, h], dim=1)


class DurationGraph(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.model = model

    def forward(
        self, text_state, text_mask, speaker_state, has_speaker, caption_state, caption_mask, has_caption
    ):
        b = text_state.shape[0]
        aux = torch.zeros(b, self.model.cfg.duration_aux_dim, dtype=text_state.dtype)
        return self.model.duration_predictor(
            text_state,
            text_mask=text_mask.bool(),
            aux_features=aux,
            speaker_state=speaker_state,
            speaker_mask=None,
            has_speaker=has_speaker.bool(),
            caption_state=caption_state,
            caption_mask=caption_mask.bool(),
            has_caption=has_caption.bool(),
        )


class ContextGraph(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.blocks = model.blocks

    def forward(self, text_state, speaker_state, caption_state):
        per_layer = [
            b.attention.project_context_kv(text_state, speaker_state, caption_state)
            for b in self.blocks
        ]
        return tuple(torch.stack([kv[j] for kv in per_layer], dim=0) for j in range(6))


class DiTStepGraph(nn.Module):
    def __init__(self, model):
        super().__init__()
        self.m = model

    def _forward(self, x_t, t, delta_t, text_mask, speaker_mask, caption_mask, kt, vt, ks, vs, kc, vc):
        m = self.m
        cond = m.cond_module(get_timestep_embedding(t, m.cfg.timestep_embed_dim))
        if delta_t is not None:
            cond = cond + m.delta_cond_module(get_timestep_embedding(delta_t, m.cfg.timestep_embed_dim))
        cond = cond[:, None, :]
        x = m.in_proj(x_t)
        table = rope_table(m.head_dim, x)
        self_mask = torch.ones_like(x[:, :, 0], dtype=torch.bool)
        plan = build_context_attention_plan(
            [self_mask, text_mask.bool(), speaker_mask.bool(), caption_mask.bool()],
            query_len=0,
            attention_dtype=x.dtype,
        )
        for i, block in enumerate(m.blocks):
            x = block(
                x=x,
                cond_embed=cond,
                text_state=None,
                text_mask=None,
                speaker_state=None,
                speaker_mask=None,
                caption_state=None,
                caption_mask=None,
                freqs_cis=table,
                self_mask=None,
                context_kv=(kt[i], vt[i], ks[i], vs[i], kc[i], vc[i]),
                context_plan=plan,
            )
        return m.out_proj(m.out_norm(x))


class RectifiedFlowStepGraph(DiTStepGraph):
    def forward(self, x_t, t, text_mask, speaker_mask, caption_mask, kt, vt, ks, vs, kc, vc):
        return self._forward(x_t, t, None, text_mask, speaker_mask, caption_mask, kt, vt, ks, vs, kc, vc)


class MeanFlowStepGraph(DiTStepGraph):
    def forward(self, x_t, t, delta_t, text_mask, speaker_mask, caption_mask, kt, vt, ks, vs, kc, vc):
        return self._forward(x_t, t, delta_t, text_mask, speaker_mask, caption_mask, kt, vt, ks, vs, kc, vc)


class CodecEncoderGraph(nn.Module):
    def __init__(self, dac):
        super().__init__()
        self.dac = dac

    def forward(self, wav):
        z = self.dac.encoder(wav)
        mean, _ = self.dac.quantizer.in_proj(z).chunk(2, dim=1)
        return mean.transpose(1, 2)


class CodecDecoderGraph(nn.Module):
    def __init__(self, dac):
        super().__init__()
        self.dac = dac

    def forward(self, latent):
        return self.dac.decode(latent.transpose(1, 2))
