# Audio import fixtures

Synthetic 440 Hz sine, 1.1 seconds, stereo, generated locally with FFmpeg from the lavfi sine source. No recorded voice or third-party content. WAV is 44.1 kHz; MP3, FLAC, Ogg Vorbis, AAC/ADTS and AAC/M4A are encoded from it. These exercise the actual decoder and resampler, including compressed formats and channel mixing.

`legacy-project.zip` is a fixed version-1 archive using the former web app's YAML field names and values, including string seeds, camel-case editor properties, and snake-case character properties. The compatibility test loads and resaves it rather than constructing it through the current serializer.
