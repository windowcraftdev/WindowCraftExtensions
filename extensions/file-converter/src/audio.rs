// Made by Nordan
use crate::target::Target;
use crate::{Error, Result};
use std::fs::File;
use std::io::{BufWriter, Read, Seek, SeekFrom, Write};
use std::path::Path;
fn io_error(path: &Path) -> impl Fn(std::io::Error) -> Error + '_ {
    move |error| Error::io(path.display().to_string(), error)
}

use symphonia::core::audio::{Channels, SampleBuffer};
use symphonia::core::codecs::{
    CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_NULL, CodecType, Decoder,
    DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::{FormatOptions, FormatReader};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// An open audio track, ready to hand out interleaved samples.
struct Sound {
    codec: CodecType,
    format: Box<dyn FormatReader>,
    decoder: Box<dyn Decoder>,
    track: u32,
    rate: u32,
    channels: usize,
    layout: Option<Channels>,
    /// 24 for lossless sources deeper than 16 bits, otherwise 16.
    bits: u32,
    frames: Option<u64>,
    position: u64,
}

impl Sound {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path).map_err(io_error(path))?;
        let stream = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }
        let probed = symphonia::default::get_probe()
            .format(
                &hint,
                stream,
                &FormatOptions {
                    enable_gapless: true,
                    ..FormatOptions::default()
                },
                &MetadataOptions::default(),
            )
            .map_err(|_| Error::Reason(crate::text::Reason::Unsupported))?;
        let format = probed.format;
        let codecs = symphonia::default::get_codecs();
        let track = format
            .tracks()
            .iter()
            .find(|track| {
                track.codec_params.codec != CODEC_TYPE_NULL
                    && track.codec_params.sample_rate.is_some()
            })
            .ok_or(Error::Reason(crate::text::Reason::NoAudio))?;
        let params = track.codec_params.clone();
        let decoder = codecs
            .make(&params, &DecoderOptions::default())
            .map_err(|e| Error::Invalid(e.to_string()))?;
        let pcm = codecs
            .get_codec(params.codec)
            .is_some_and(|codec| codec.short_name.starts_with("pcm"));
        let lossless = pcm || matches!(params.codec, CODEC_TYPE_FLAC | CODEC_TYPE_ALAC);
        let bits = if lossless && params.bits_per_sample.unwrap_or(16) > 16 {
            24
        } else {
            16
        };
        Ok(Self {
            codec: params.codec,
            track: track.id,
            rate: params
                .sample_rate
                .ok_or(Error::Reason(crate::text::Reason::NoAudio))?,
            channels: params.channels.map_or(0, Channels::count),
            layout: params.channels,
            bits,
            frames: params.n_frames,
            format,
            decoder,
            position: 0,
        })
    }

    /// The next decoded block as interleaved samples in -1.0..=1.0, or `None`
    /// at the end.
    fn next(&mut self) -> Result<Option<Vec<f32>>> {
        loop {
            let packet = match self.format.next_packet() {
                Ok(packet) => packet,
                Err(SymphoniaError::IoError(error))
                    if error.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    return Ok(None);
                }
                Err(SymphoniaError::ResetRequired) => return Ok(None),
                Err(error) => return Err(Error::Invalid(error.to_string())),
            };
            if packet.track_id() != self.track {
                continue;
            }
            self.position = packet.ts();
            match self.decoder.decode(&packet) {
                Ok(decoded) => {
                    let spec = *decoded.spec();
                    if self.channels == 0 {
                        self.channels = spec.channels.count();
                        self.layout = Some(spec.channels);
                    }
                    let mut buffer = SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
                    buffer.copy_interleaved_ref(decoded);
                    return Ok(Some(buffer.samples().to_vec()));
                }
                // A damaged packet is skipped, as players do.
                Err(SymphoniaError::DecodeError(_)) => {}
                Err(error) => return Err(Error::Invalid(error.to_string())),
            }
        }
    }

    fn share(&self) -> f32 {
        self.frames
            .filter(|frames| *frames > 0)
            .map_or(0.0, |frames| {
                (self.position as f32 / frames as f32).clamp(0.0, 1.0)
            })
    }
}

pub fn convert(
    source: &Path,
    output: &Path,
    target: Target,
    progress: &mut dyn FnMut(f32),
) -> Result<bool> {
    let mut head = [0u8; 64];
    let read = File::open(source)
        .map_err(io_error(source))?
        .read(&mut head)
        .map_err(io_error(source))?;
    let head = &head[..read];
    if (target == Target::Wav && head.starts_with(b"RIFF") && head.get(8..12) == Some(b"WAVE"))
        || (target == Target::Aiff
            && head.starts_with(b"FORM")
            && matches!(head.get(8..12), Some(b"AIFF" | b"AIFC")))
        || (target == Target::Opus
            && head.starts_with(b"OggS")
            && head.windows(8).any(|w| w == b"OpusHead"))
    {
        return Err(Error::Already(target));
    }
    if matches!(target, Target::M4a | Target::Opus) {
        // AAC and Opus are copied out of their container; this converter has no
        // encoder for them.
        if crate::remux::audio_only(source, output, target, progress)? {
            return Ok(true);
        }
        Sound::open(source)?;
        return Err(Error::Reason(crate::text::Reason::Container));
    }
    if !matches!(
        target,
        Target::Mp3 | Target::Flac | Target::Wav | Target::Aiff
    ) {
        return Ok(false);
    }
    let mut sound = Sound::open(source)?;
    if matches!(
        (target, sound.codec),
        (Target::Mp3, CODEC_TYPE_MP3) | (Target::Flac, CODEC_TYPE_FLAC)
    ) && (head.starts_with(b"fLaC")
        || head.starts_with(b"ID3")
        || (head.len() >= 2 && head[0] == 0xff && head[1] & 0xe0 == 0xe0))
    {
        return Err(Error::Already(target));
    }
    match target {
        Target::Mp3 => write_mp3(&mut sound, output, progress)?,
        Target::Flac => write_flac(&mut sound, output, progress)?,
        Target::Wav => write_pcm(&mut sound, output, false, progress)?,
        _ => write_pcm(&mut sound, output, true, progress)?,
    }
    Ok(true)
}

fn to_int(sample: f32, bits: u32) -> i32 {
    let scale = ((1i64 << (bits - 1)) - 1) as f32;
    (sample * scale).round().clamp(-scale - 1.0, scale) as i32
}

/// WAV (little-endian) or AIFF (big-endian) PCM. The sizes are filled in once
/// the length is known.
fn write_pcm(
    sound: &mut Sound,
    output: &Path,
    aiff: bool,
    progress: &mut dyn FnMut(f32),
) -> Result<()> {
    let fail = io_error(output);
    let mut writer = BufWriter::with_capacity(1 << 20, File::create(output).map_err(&fail)?);
    let header = if aiff { 54 } else { 44 };
    writer.write_all(&vec![0u8; header]).map_err(&fail)?;
    let bytes_per_sample = (sound.bits / 8) as usize;
    let mut data_bytes = 0u64;
    let mut count = 0u32;
    while let Some(block) = sound.next()? {
        count += 1;
        if count.is_multiple_of(64) {
            progress(sound.share());
        }
        let mut bytes = Vec::with_capacity(block.len() * bytes_per_sample);
        for sample in block {
            let value = to_int(sample, sound.bits).to_le_bytes();
            if aiff {
                bytes.extend(value[..bytes_per_sample].iter().rev());
            } else {
                bytes.extend_from_slice(&value[..bytes_per_sample]);
            }
        }
        data_bytes += bytes.len() as u64;
        writer.write_all(&bytes).map_err(&fail)?;
    }
    if sound.channels == 0 || data_bytes == 0 {
        return Err(Error::Reason(crate::text::Reason::NoAudio));
    }
    if data_bytes % 2 == 1 {
        writer.write_all(&[0]).map_err(&fail)?;
    }
    let mut file = writer.into_inner().map_err(|e| fail(e.into_error()))?;
    let channels = sound.channels as u16;
    let frames = data_bytes / (bytes_per_sample as u64 * u64::from(channels));
    let mut head = Vec::with_capacity(header);
    if aiff {
        let form = 4 + 26 + 16 + data_bytes + data_bytes % 2;
        head.extend_from_slice(b"FORM");
        head.extend_from_slice(&(form as u32).to_be_bytes());
        head.extend_from_slice(b"AIFFCOMM");
        head.extend_from_slice(&18u32.to_be_bytes());
        head.extend_from_slice(&channels.to_be_bytes());
        head.extend_from_slice(&(frames as u32).to_be_bytes());
        head.extend_from_slice(&(sound.bits as u16).to_be_bytes());
        head.extend_from_slice(&extended(f64::from(sound.rate)));
        head.extend_from_slice(b"SSND");
        head.extend_from_slice(&((data_bytes + 8) as u32).to_be_bytes());
        head.extend_from_slice(&[0; 8]);
    } else {
        let block_align = channels * bytes_per_sample as u16;
        head.extend_from_slice(b"RIFF");
        head.extend_from_slice(&((36 + data_bytes + data_bytes % 2) as u32).to_le_bytes());
        head.extend_from_slice(b"WAVEfmt ");
        head.extend_from_slice(&16u32.to_le_bytes());
        head.extend_from_slice(&1u16.to_le_bytes());
        head.extend_from_slice(&channels.to_le_bytes());
        head.extend_from_slice(&sound.rate.to_le_bytes());
        head.extend_from_slice(&(sound.rate * u32::from(block_align)).to_le_bytes());
        head.extend_from_slice(&block_align.to_le_bytes());
        head.extend_from_slice(&(sound.bits as u16).to_le_bytes());
        head.extend_from_slice(b"data");
        head.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    }
    if data_bytes > u64::from(u32::MAX) - 64 {
        return Err(Error::Reason(crate::text::Reason::TooLarge));
    }
    file.seek(SeekFrom::Start(0)).map_err(&fail)?;
    file.write_all(&head).map_err(&fail)?;
    file.sync_all().map_err(&fail)
}

/// The 80-bit extended float AIFF uses for the sample rate.
fn extended(value: f64) -> [u8; 10] {
    let mut out = [0u8; 10];
    if value <= 0.0 {
        return out;
    }
    let exponent = value.log2().floor() as i32;
    let mantissa = (value / 2f64.powi(exponent) * (1u64 << 63) as f64) as u64;
    out[..2].copy_from_slice(&((exponent + 16383) as u16).to_be_bytes());
    out[2..].copy_from_slice(&mantissa.to_be_bytes());
    out
}

/// Feeds decoded sound to flacenc block by block.
struct FlacFeed<'a> {
    sound: &'a mut Sound,
    pending: Vec<i32>,
    progress: &'a mut dyn FnMut(f32),
    error: Option<Error>,
    blocks: u32,
}

impl flacenc::source::Source for FlacFeed<'_> {
    fn channels(&self) -> usize {
        self.sound.channels
    }

    fn bits_per_sample(&self) -> usize {
        self.sound.bits as usize
    }

    fn sample_rate(&self) -> usize {
        self.sound.rate as usize
    }

    fn read_samples<F: flacenc::source::Fill>(
        &mut self,
        block_size: usize,
        dest: &mut F,
    ) -> std::result::Result<usize, flacenc::error::SourceError> {
        let wanted = block_size * self.sound.channels;
        while self.pending.len() < wanted && self.error.is_none() {
            self.blocks += 1;
            if self.blocks.is_multiple_of(64) {
                (self.progress)(self.sound.share());
            }
            match self.sound.next() {
                Ok(Some(block)) => self.pending.extend(
                    block
                        .into_iter()
                        .map(|sample| to_int(sample, self.sound.bits)),
                ),
                Ok(None) => break,
                Err(error) => self.error = Some(error),
            }
        }
        let take = self.pending.len().min(wanted);
        let take = take - take % self.sound.channels.max(1);
        let chunk: Vec<i32> = self.pending.drain(..take).collect();
        dest.fill_interleaved(&chunk)?;
        Ok(take / self.sound.channels.max(1))
    }
}

fn write_flac(sound: &mut Sound, output: &Path, progress: &mut dyn FnMut(f32)) -> Result<()> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    if sound.channels == 0 || sound.channels > 8 {
        return Err(Error::Reason(crate::text::Reason::Container));
    }
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, error)| Error::Invalid(format!("FLAC settings: {error:?}")))?;
    let block_size = config.block_size;
    let mut feed = FlacFeed {
        sound,
        pending: Vec::new(),
        progress,
        error: None,
        blocks: 0,
    };
    let stream = flacenc::encode_with_fixed_block_size(&config, &mut feed, block_size);
    if let Some(error) = feed.error.take() {
        return Err(error);
    }
    let mut stream = stream.map_err(|error| Error::Invalid(format!("FLAC: {error:?}")))?;
    // flacenc counts the short last block as the minimum. The reference
    // encoder does not, and readers take unequal sizes for variable blocking.
    stream
        .stream_info_mut()
        .set_block_sizes(block_size, block_size)
        .map_err(|error| Error::Invalid(format!("FLAC: {error:?}")))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream
        .write(&mut sink)
        .map_err(|error| Error::Invalid(format!("FLAC: {error:?}")))?;
    std::fs::write(output, sink.as_slice()).map_err(io_error(output))
}

/// Folds surround down to stereo with the usual ITU weights.
fn downmix(block: &[f32], channels: usize, layout: Option<Channels>) -> Vec<f32> {
    let order: Vec<Channels> = layout
        .map(|layout| layout.iter().collect())
        .unwrap_or_default();
    let weight = |index: usize| -> (f32, f32) {
        match order.get(index) {
            Some(&channel) if channel == Channels::FRONT_LEFT => (1.0, 0.0),
            Some(&channel) if channel == Channels::FRONT_RIGHT => (0.0, 1.0),
            Some(&channel) if channel == Channels::FRONT_CENTRE => (0.707, 0.707),
            Some(&channel) if channel == Channels::LFE1 => (0.0, 0.0),
            Some(&channel)
                if channel == Channels::REAR_LEFT
                    || channel == Channels::SIDE_LEFT
                    || channel == Channels::FRONT_LEFT_CENTRE =>
            {
                (0.707, 0.0)
            }
            Some(&channel)
                if channel == Channels::REAR_RIGHT
                    || channel == Channels::SIDE_RIGHT
                    || channel == Channels::FRONT_RIGHT_CENTRE =>
            {
                (0.0, 0.707)
            }
            _ if index == 0 => (1.0, 0.0),
            _ if index == 1 => (0.0, 1.0),
            _ => (0.5, 0.5),
        }
    };
    let weights: Vec<(f32, f32)> = (0..channels).map(weight).collect();
    let total_left: f32 = weights.iter().map(|w| w.0).sum::<f32>().max(1.0);
    let total_right: f32 = weights.iter().map(|w| w.1).sum::<f32>().max(1.0);
    let mut out = Vec::with_capacity(block.len() / channels * 2);
    for frame in block.chunks_exact(channels) {
        let (mut left, mut right) = (0.0, 0.0);
        for (sample, (to_left, to_right)) in frame.iter().zip(&weights) {
            left += sample * to_left;
            right += sample * to_right;
        }
        out.push(left / total_left);
        out.push(right / total_right);
    }
    out
}

/// MP3 through LAME at its top variable-bitrate setting (V0).
fn write_mp3(sound: &mut Sound, output: &Path, progress: &mut dyn FnMut(f32)) -> Result<()> {
    use mp3lame_encoder::{Builder, FlushGap, InterleavedPcm, MonoPcm, Quality, VbrMode};
    let lame = |error: &dyn std::fmt::Display| Error::Invalid(format!("LAME: {error}"));
    let fail = io_error(output);
    // The first block tells how many channels there really are.
    let first = sound
        .next()?
        .ok_or(Error::Reason(crate::text::Reason::NoAudio))?;
    let channels = sound.channels;
    if channels == 0 {
        return Err(Error::Reason(crate::text::Reason::NoAudio));
    }
    let out_channels: u8 = if channels == 1 { 1 } else { 2 };
    let mut builder = Builder::new().ok_or(Error::Reason(crate::text::Reason::Encoder))?;
    builder
        .set_num_channels(out_channels)
        .map_err(|e| lame(&e))?;
    builder.set_sample_rate(sound.rate).map_err(|e| lame(&e))?;
    builder.set_vbr_mode(VbrMode::Mtrh).map_err(|e| lame(&e))?;
    builder
        .set_vbr_quality(Quality::Best)
        .map_err(|e| lame(&e))?;
    builder
        .set_quality(Quality::NearBest)
        .map_err(|e| lame(&e))?;
    builder.set_to_write_vbr_tag(true).map_err(|e| lame(&e))?;
    let mut encoder = builder.build().map_err(|e| lame(&e))?;

    let mut file = File::create(output).map_err(&fail)?;
    let mut writer = BufWriter::with_capacity(1 << 20, &mut file);
    let mut buffer: Vec<u8> = Vec::new();
    let mut block = Some(first);
    let mut count = 0u32;
    while let Some(samples) = block {
        count += 1;
        if count.is_multiple_of(64) {
            progress(sound.share());
        }
        let samples = if channels > 2 {
            downmix(&samples, channels, sound.layout)
        } else {
            samples
        };
        let frames = samples.len() / usize::from(out_channels);
        buffer.clear();
        buffer.reserve(mp3lame_encoder::max_required_buffer_size(frames));
        if out_channels == 1 {
            encoder
                .encode_to_vec(MonoPcm(samples.as_slice()), &mut buffer)
                .map_err(|e| lame(&e))?;
        } else {
            encoder
                .encode_to_vec(InterleavedPcm(samples.as_slice()), &mut buffer)
                .map_err(|e| lame(&e))?;
        }
        writer.write_all(&buffer).map_err(&fail)?;
        block = sound.next()?;
    }
    buffer.clear();
    buffer.reserve(7200);
    encoder
        .flush_to_vec::<FlushGap>(&mut buffer)
        .map_err(|e| lame(&e))?;
    writer.write_all(&buffer).map_err(&fail)?;
    writer.flush().map_err(&fail)?;
    drop(writer);
    // The Xing/LAME frame at the start gets the real length now, so players
    // can seek and show the duration of a variable-bitrate file.
    let mut tag = Vec::with_capacity(encoder.lame_tag_size());
    if encoder.lame_tag_encode_to_vec(&mut tag).is_some() {
        file.seek(SeekFrom::Start(0)).map_err(&fail)?;
        file.write_all(&tag).map_err(&fail)?;
    }
    file.sync_all().map_err(&fail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone_wav(path: &Path, rate: u32, channels: u16, seconds: f32) {
        let frames = (rate as f32 * seconds) as usize;
        let mut data = Vec::new();
        for index in 0..frames {
            let value = ((index as f32 * 440.0 * std::f32::consts::TAU / rate as f32).sin()
                * 12000.0) as i16;
            for _ in 0..channels {
                data.extend_from_slice(&value.to_le_bytes());
            }
        }
        let mut file = Vec::new();
        file.extend_from_slice(b"RIFF");
        file.extend_from_slice(&((36 + data.len()) as u32).to_le_bytes());
        file.extend_from_slice(b"WAVEfmt ");
        file.extend_from_slice(&16u32.to_le_bytes());
        file.extend_from_slice(&1u16.to_le_bytes());
        file.extend_from_slice(&channels.to_le_bytes());
        file.extend_from_slice(&rate.to_le_bytes());
        file.extend_from_slice(&(rate * u32::from(channels) * 2).to_le_bytes());
        file.extend_from_slice(&(channels * 2).to_le_bytes());
        file.extend_from_slice(&16u16.to_le_bytes());
        file.extend_from_slice(b"data");
        file.extend_from_slice(&(data.len() as u32).to_le_bytes());
        file.extend_from_slice(&data);
        std::fs::write(path, file).unwrap();
    }

    fn convert_to(source: &Path, target: Target) -> std::path::PathBuf {
        let output = source.with_extension(format!("out.{}", target.extension()));
        if target == Target::Wav {
            write_pcm(
                &mut Sound::open(source).unwrap(),
                &output,
                false,
                &mut |_| {},
            )
            .unwrap();
            return output;
        }
        assert!(
            convert(source, &output, target, &mut |_| {}).unwrap(),
            "{target:?} must be done here"
        );
        output
    }

    #[test]
    fn sound_round_trips_through_every_native_format() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("tone.wav");
        tone_wav(&wav, 44_100, 2, 1.0);
        for target in [Target::Flac, Target::Aiff, Target::Wav, Target::Mp3] {
            let output = convert_to(&wav, target);
            let mut sound = Sound::open(&output)
                .unwrap_or_else(|e| panic!("{target:?} output must decode: {e}"));
            let mut samples = 0usize;
            while let Some(block) = sound.next().unwrap() {
                samples += block.len();
            }
            assert_eq!(sound.channels, 2, "{target:?}");
            assert_eq!(sound.rate, 44_100, "{target:?}");
            // MP3 works in frames of 1152; a reader trims the encoder delay
            // from the LAME header, so only the last frame may differ.
            let expected = 44_100 * 2;
            if target == Target::Mp3 {
                assert!(samples.abs_diff(expected) <= 2 * 1152, "{samples}");
            } else {
                assert_eq!(samples, expected, "{target:?}");
            }
        }
    }

    #[test]
    fn mp3_starts_with_a_vbr_header() {
        let dir = tempfile::tempdir().unwrap();
        let wav = dir.path().join("tone.wav");
        tone_wav(&wav, 48_000, 1, 2.0);
        let mp3 = std::fs::read(convert_to(&wav, Target::Mp3)).unwrap();
        assert_eq!(mp3[0], 0xFF, "no ID3 tag in front");
        assert!(
            mp3.windows(4)
                .take(200)
                .any(|window| window == b"Xing" || window == b"Info")
        );
    }

    #[test]
    fn surround_folds_to_stereo_without_clipping() {
        let layout = Channels::FRONT_LEFT
            | Channels::FRONT_RIGHT
            | Channels::FRONT_CENTRE
            | Channels::LFE1
            | Channels::REAR_LEFT
            | Channels::REAR_RIGHT;
        let frame = [1.0f32; 6];
        let stereo = downmix(&frame, 6, Some(layout));
        assert_eq!(stereo.len(), 2);
        assert!(stereo.iter().all(|sample| *sample <= 1.0 && *sample > 0.9));
    }

    #[test]
    fn aiff_rate_is_written_as_extended_float() {
        assert_eq!(
            extended(44_100.0),
            [0x40, 0x0E, 0xAC, 0x44, 0, 0, 0, 0, 0, 0]
        );
        assert_eq!(
            extended(48_000.0),
            [0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0]
        );
    }

    #[test]
    fn rejects_existing_audio_format_by_content() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("tone.bin");
        tone_wav(&source, 44_100, 1, 0.1);
        let out = dir.path().join("out.wav");
        assert!(matches!(
            convert(&source, &out, Target::Wav, &mut |_| {}),
            Err(Error::Already(Target::Wav))
        ));
        for target in [Target::Mp3, Target::Flac, Target::Aiff] {
            let converted = convert_to(&source, target);
            assert!(matches!(
                convert(&converted, &out, target, &mut |_| {}),
                Err(Error::Already(_))
            ));
        }
    }
}
