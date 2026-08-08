use std::path::Path;

use clap::Parser;

#[derive(Parser)]
#[command(about = "Convert videos to MIDI for Roland Sound Canvas")]
struct Args {
    #[arg(required_unless_present = "list_ports")]
    input_video: Option<String>,

    #[arg(short, long, default_value = "30")]
    framerate: u32,

    #[arg(short = 's', long, help = "Enable SC-8850 mode (higher resolution)")]
    sc8850: bool,

    #[arg(short, long, help = "Enable interlace mode (SC-8850 only)")]
    interlace: bool,

    #[arg(short = 'e', long, help = "Enable SD-90 reduced columns (SC-8850 only)")]
    sd90: bool,

    #[arg(short = 'd', long, help = "Enable Floyd-Steinberg dithering (SC-8850/SD-90)")]
    dither: bool,

    #[arg(long = "edge", value_name = "THRESHOLD", num_args = 0..=1, default_missing_value = "50", help = "Sobel edge detection (SC-8850/SD-90 only)")]
    edge: Option<u8>,

    #[arg(short, long, help = "Output MIDI file path (auto-generated if omitted)")]
    output: Option<String>,

    #[cfg(feature = "midi-output")]
    #[arg(long, help = "Stream to MIDI output port in real-time")]
    midi_port: Option<String>,

    #[cfg(feature = "midi-output")]
    #[arg(long, help = "List available MIDI output ports")]
    list_ports: bool,
}

fn output_path(input_video: &str, options: &videocanvas::Options) -> Result<String, Box<dyn std::error::Error>> {
    let stem = Path::new(input_video)
        .file_stem()
        .ok_or("invalid input path")?
        .to_str()
        .ok_or("non-UTF-8 filename")?;

    let suffix = format!(
        "{}{}{}f{}",
        if options.mode != videocanvas::DeviceMode::Sc55 { "s" } else { "" },
        if options.mode == videocanvas::DeviceMode::Sd90 { "e" } else { "" },
        if options.interlace { "i" } else { "" },
        options.framerate,
    );

    Ok(format!("{}_{}.mid", stem, suffix))
}

#[cfg(feature = "midi-output")]
fn list_midi_ports() -> Result<(), Box<dyn std::error::Error>> {
    use midir::MidiOutput;
    let midi_out = MidiOutput::new("videocanvas")
        .map_err(|e| format!("midi init: {}", e))?;
    for port in midi_out.ports() {
        let name = midi_out.port_name(&port).unwrap_or_else(|_| "unknown".into());
        println!("{}", name);
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let mode = if args.sc8850 && args.sd90 {
        videocanvas::DeviceMode::Sd90
    } else if args.sc8850 {
        videocanvas::DeviceMode::Sc8850
    } else {
        videocanvas::DeviceMode::Sc55
    };

    let options = videocanvas::Options {
        framerate: args.framerate,
        mode,
        interlace: args.interlace,
        dither: args.dither,
        edge: args.edge,
    };

    #[cfg(feature = "midi-output")]
    if args.list_ports {
        return list_midi_ports();
    }

    let input = args.input_video.as_deref().ok_or("no input video specified")?;

    #[cfg(feature = "midi-output")]
    if let Some(port) = args.midi_port {
        return videocanvas::stream_video(input, &options, &port);
    }

    let smf = videocanvas::convert_video(input, &options)?;
    let path = match args.output {
        Some(p) => p,
        None => output_path(input, &options)?,
    };
    smf.save(&path)?;
    eprintln!("Saved to {}", path);

    Ok(())
}
