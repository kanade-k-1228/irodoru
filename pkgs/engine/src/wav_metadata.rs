//! RIFF INFO comments, without decoding or modifying the PCM payload.
use anyhow::{Result, ensure};
use std::io::{Read, Seek, SeekFrom};

const MAX_COMMENT: usize = 4 * 1024 * 1024;

/// Read only chunk headers and the comment; skip audio data even in large files.
pub fn read_comment(reader: &mut (impl Read + Seek)) -> Result<Option<Vec<u8>>> {
    let length = reader.seek(SeekFrom::End(0))?;
    reader.seek(SeekFrom::Start(0))?;
    let mut header = [0; 12];
    reader.read_exact(&mut header)?;
    ensure!(
        &header[..4] == b"RIFF" && &header[8..] == b"WAVE",
        "invalid RIFF/WAVE header"
    );
    let end = u32::from_le_bytes(header[4..8].try_into()?) as u64 + 8;
    ensure!(end >= 12 && end <= length, "truncated WAV");
    let mut position = 12;
    let mut comment = None;
    while position < end {
        let (id, size, next) = chunk_header(reader, position, end)?;
        if &id == b"LIST" && size >= 4 {
            let mut kind = [0; 4];
            reader.read_exact(&mut kind)?;
            if &kind == b"INFO" {
                let info_end = position + 8 + size;
                let mut info_position = position + 12;
                while info_position < info_end {
                    let (id, size, next) = chunk_header(reader, info_position, info_end)?;
                    if &id == b"ICMT" {
                        ensure!(size <= MAX_COMMENT as u64, "WAV comment too large");
                        ensure!(comment.is_none(), "duplicate WAV comment");
                        let mut bytes = vec![0; size as usize];
                        reader.read_exact(&mut bytes)?;
                        while bytes.last() == Some(&0) {
                            bytes.pop();
                        }
                        comment = Some(bytes);
                    }
                    info_position = next;
                }
            }
        }
        position = next;
    }
    Ok(comment)
}

fn chunk_header(
    reader: &mut (impl Read + Seek),
    position: u64,
    end: u64,
) -> Result<([u8; 4], u64, u64)> {
    ensure!(end - position >= 8, "truncated WAV chunk header");
    reader.seek(SeekFrom::Start(position))?;
    let mut header = [0; 8];
    reader.read_exact(&mut header)?;
    let size = u32::from_le_bytes(header[4..].try_into()?) as u64;
    let next = position + 8 + size + size % 2;
    ensure!(next <= end, "truncated WAV chunk");
    Ok((header[..4].try_into()?, size, next))
}

/// Replace ICMT while preserving all other RIFF chunks, including INFO tags.
pub fn with_comment(wav: &[u8], comment: &str) -> Result<Vec<u8>> {
    ensure!(comment.len() < MAX_COMMENT, "WAV comment too large");
    ensure!(
        wav.len() >= 12 && &wav[..4] == b"RIFF" && &wav[8..12] == b"WAVE",
        "invalid RIFF/WAVE header"
    );
    let end = u32::from_le_bytes(wav[4..8].try_into()?) as usize + 8;
    ensure!(end == wav.len(), "invalid WAV length");
    let mut output = wav[..12].to_vec();
    for chunk in chunks(&wav[12..])? {
        let payload = chunk_payload(chunk);
        if &chunk[..4] == b"LIST" && payload.starts_with(b"INFO") {
            let mut info = b"INFO".to_vec();
            for tag in chunks(&payload[4..])? {
                if &tag[..4] != b"ICMT" {
                    info.extend_from_slice(tag);
                }
            }
            if info.len() > 4 {
                append_chunk(&mut output, b"LIST", &info)?;
            }
        } else {
            output.extend_from_slice(chunk);
        }
    }
    let mut info = b"INFO".to_vec();
    let mut bytes = comment.as_bytes().to_vec();
    bytes.push(0);
    append_chunk(&mut info, b"ICMT", &bytes)?;
    append_chunk(&mut output, b"LIST", &info)?;
    let size = u32::try_from(output.len() - 8)?;
    output[4..8].copy_from_slice(&size.to_le_bytes());
    Ok(output)
}

fn chunk_payload(chunk: &[u8]) -> &[u8] {
    let size = u32::from_le_bytes(chunk[4..8].try_into().unwrap()) as usize;
    &chunk[8..8 + size]
}

fn chunks(mut bytes: &[u8]) -> Result<Vec<&[u8]>> {
    let mut chunks = Vec::new();
    while !bytes.is_empty() {
        ensure!(bytes.len() >= 8, "truncated WAV chunk header");
        let size = u32::from_le_bytes(bytes[4..8].try_into()?) as usize;
        let end = 8usize
            .checked_add(size)
            .and_then(|end| end.checked_add(size % 2))
            .ok_or_else(|| anyhow::anyhow!("WAV chunk too large"))?;
        ensure!(end <= bytes.len(), "truncated WAV chunk");
        chunks.push(&bytes[..end]);
        bytes = &bytes[end..];
    }
    Ok(chunks)
}

fn append_chunk(output: &mut Vec<u8>, id: &[u8; 4], bytes: &[u8]) -> Result<()> {
    output.extend_from_slice(id);
    output.extend_from_slice(&u32::try_from(bytes.len())?.to_le_bytes());
    output.extend_from_slice(bytes);
    if !bytes.len().is_multiple_of(2) {
        output.push(0);
    }
    Ok(())
}
