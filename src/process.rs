//! Video processing pipeline: ffmpeg decode -> fps/scale filter -> sysex ->
//! MIDI output (either SMF file or real-time port).

use std::sync::OnceLock;

use ffmpeg_next as ffmpeg;
use ffmpeg::media::Type;
use ffmpeg::util::frame::video::Video;
use midly::num::{u15, u24, u28};
use midly::{Arena, Format, Header, MetaMessage, Smf, Timing, TrackEvent, TrackEventKind};

use crate::image;
use crate::sysex;
use crate::{DeviceMode, Options, Result};

static FFMPEG_READY: OnceLock<bool> = OnceLock::new();

fn ensure_ffmpeg() -> Result<()> {
    if *FFMPEG_READY.get_or_init(|| ffmpeg::init().is_ok()) {
        Ok(())
    } else {
        Err("failed to initialize ffmpeg".into())
    }
}

struct MidiEvent {
    absolute_tick: u64,
    data: Vec<u8>,
}

const TICKS_PER_SECOND: f64 = 480.0 * 1_000_000.0 / 500_000.0; // 120 BPM

fn process_frame(
    filtered: &Video,
    frame_index: u64,
    options: &Options,
    prev_abs_tick: &mut u64,
) -> Result<Vec<MidiEvent>> {
    let frame_duration = 1.0 / options.framerate as f64;
    let pixels = image::extract_pixels(filtered);
    let binary = if let Some(t) = options.edge
        && matches!(options.mode, DeviceMode::Sc8850 | DeviceMode::Sd90)
    {
        image::sobel_edge_detect(&pixels, filtered.width(), filtered.height(), t)
    } else if options.dither && matches!(options.mode, DeviceMode::Sc8850 | DeviceMode::Sd90) {
        image::floyd_steinberg_dither(&pixels, filtered.width(), filtered.height())
    } else {
        image::otsu_threshold(&pixels)
    };
    let frame_seconds = filtered.pts().unwrap_or(0) as f64 / options.framerate as f64;

    match options.mode {
        DeviceMode::Sc55 => {
            let data = sysex::calculate_data(&binary);
            let sysex = sysex::pack_sysex_message(&data);
            let abs_tick = (frame_seconds * TICKS_PER_SECOND) as u64;
            *prev_abs_tick = abs_tick;
            Ok(vec![MidiEvent { absolute_tick: abs_tick, data: sysex }])
        }
        DeviceMode::Sc8850 | DeviceMode::Sd90 => {
            let sections = if options.mode == DeviceMode::Sd90 {
                sysex::calculate_data_sd90(&binary)
            } else {
                sysex::calculate_data_8850(&binary)
            };
            let mut sysexes = sysex::pack_sysex_message_8850(&sections);
            let section_order = image::interlace_section_order(frame_index, options.interlace);
            let num_sections = section_order.len();

            let mut events = Vec::with_capacity(num_sections);
            for (slot, &section_idx) in section_order.iter().enumerate() {
                let abs_time = frame_seconds
                    + slot as f64 * frame_duration / num_sections as f64;
                let abs_tick = (abs_time * TICKS_PER_SECOND) as u64;
                events.push(MidiEvent {
                    absolute_tick: abs_tick,
                    data: std::mem::take(&mut sysexes[section_idx]),
                });
            }
            *prev_abs_tick = events.last().map(|e| e.absolute_tick).unwrap_or(*prev_abs_tick);
            Ok(events)
        }
    }
}

fn run_pipeline<F>(input: &str, options: &Options, mut on_frame: F) -> Result<()>
where
    F: FnMut(Vec<MidiEvent>) -> Result<()>,
{
    ensure_ffmpeg()?;

    let mut ictx = ffmpeg::format::input(input)?;

    let input_stream = ictx
        .streams()
        .best(Type::Video)
        .ok_or("no video stream found")?;
    let video_idx = input_stream.index();
    let time_base = input_stream.time_base();

    let context_decoder =
        ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context_decoder.decoder().video()?;

    let (target_w, target_h) = match options.mode {
        DeviceMode::Sd90 => (128, 64),
        DeviceMode::Sc8850 => (160, 64),
        DeviceMode::Sc55 => (16, 16),
    };

    let mut graph = ffmpeg::filter::Graph::new();

    let buffer_args = format!(
        "video_size={}x{}:pix_fmt={}:time_base={}:pixel_aspect={}",
        decoder.width(),
        decoder.height(),
        decoder.format().descriptor().ok_or("unknown pixel format")?.name(),
        time_base,
        decoder.aspect_ratio(),
    );

    graph.add(
        &ffmpeg::filter::find("buffer").ok_or("buffer filter not found")?,
        "in",
        &buffer_args,
    )?;
    graph.add(
        &ffmpeg::filter::find("buffersink").ok_or("buffersink filter not found")?,
        "out",
        "",
    )?;

    let filter_spec = format!(
        "fps=fps={},format=gray,scale={}:{}:flags=lanczos",
        options.framerate, target_w, target_h,
    );

    graph.output("in", 0)?.input("out", 0)?.parse(&filter_spec)?;
    graph.validate()?;

    let mut prev_abs_tick: u64 = 0;
    let mut frame_index: u64 = 0;

    for (stream, packet) in ictx.packets() {
        if stream.index() != video_idx {
            continue;
        }
        decoder.send_packet(&packet)?;

        let mut decoded = Video::empty();
        while decoder.receive_frame(&mut decoded).is_ok() {
            graph.get("in").ok_or("filter graph missing 'in'")?.source().add(&decoded)?;

            let mut filtered = Video::empty();
            while graph.get("out").ok_or("filter graph missing 'out'")?.sink().frame(&mut filtered).is_ok() {
                let events = process_frame(
                    &filtered,
                    frame_index,
                    options,
                    &mut prev_abs_tick,
                )?;
                on_frame(events)?;
                frame_index += 1;
            }
        }
    }

    decoder.send_eof()?;
    let mut decoded = Video::empty();
    while decoder.receive_frame(&mut decoded).is_ok() {
        graph.get("in").ok_or("filter graph missing 'in'")?.source().add(&decoded)?;
    }
    graph.get("in").ok_or("filter graph missing 'in'")?.source().flush()?;

    let mut filtered = Video::empty();
    while graph.get("out").ok_or("filter graph missing 'out'")?.sink().frame(&mut filtered).is_ok() {
        let events = process_frame(
            &filtered,
            frame_index,
            options,
            &mut prev_abs_tick,
        )?;
        on_frame(events)?;
        frame_index += 1;
    }

    eprintln!("Processed {} frames", frame_index);

    Ok(())
}

/// Process video -> MIDI SMF file (returned as `Smf<'static>` for the caller to save).
pub fn video_to_smf(input: &str, options: &Options) -> Result<Smf<'static>> {
    let arena = Arena::new();
    let header = Header::new(Format::Parallel, Timing::Metrical(u15::new(480)));
    let mut smf = Smf::new(header);

    let track_meta: Vec<TrackEvent> = vec![
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::Tempo(u24::new(500_000))),
        },
        TrackEvent {
            delta: u28::new(0),
            kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
        },
    ];
    smf.tracks.push(track_meta);

    let mut track_data: Vec<TrackEvent> = Vec::new();
    let mut prev_tick: u64 = 0;

    run_pipeline(input, options, |events| {
        for event in events {
            let delta = (event.absolute_tick - prev_tick).min(u32::MAX as u64) as u32;
            prev_tick = event.absolute_tick;
            let mut data = event.data;
            data.push(0xF7);
            track_data.push(TrackEvent {
                delta: u28::new(delta),
                kind: TrackEventKind::SysEx(arena.add(&data)),
            });
        }
        Ok(())
    })?;

    track_data.push(TrackEvent {
        delta: u28::new(0),
        kind: TrackEventKind::Meta(MetaMessage::EndOfTrack),
    });
    smf.tracks.push(track_data);

    Ok(smf.to_static())
}

/// Process video -> stream sysex messages in real-time to a MIDI output port,
/// paced by the fps filter tempo (sleeps between events when ahead of schedule).
#[cfg(feature = "midi-output")]
pub fn video_to_port(input: &str, options: &Options, port_name: &str) -> Result<()> {
    use std::time::{Duration, Instant};

    use midir::MidiOutput;

    let midi_out =
        MidiOutput::new("videocanvas").map_err(|e| format!("midi init: {}", e))?;

    let ports = midi_out.ports();
    let port = ports
        .iter()
        .find(|p| {
            midi_out
                .port_name(p)
                .map(|n| n.contains(port_name))
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("midi port '{}' not found", port_name))?;

    let mut conn = midi_out
        .connect(port, "videocanvas")
        .map_err(|e| format!("midi connect: {}", e))?;

    let start = Instant::now();

    run_pipeline(input, options, |events| {
        for event in &events {
            let target = start
                + Duration::from_secs_f64(event.absolute_tick as f64 / TICKS_PER_SECOND);
            let now = Instant::now();
            if target > now {
                std::thread::sleep(target - now);
            }

            let mut msg = Vec::with_capacity(event.data.len() + 2);
            msg.push(0xF0);
            msg.extend_from_slice(&event.data);
            msg.push(0xF7);
            conn.send(&msg)
                .map_err(|e| format!("midi send: {}", e))?;
        }
        Ok(())
    })
}
