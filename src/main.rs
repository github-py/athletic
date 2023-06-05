use clap::{Parser, Subcommand};
use color_eyre::Report;
use flume::Receiver;
use ggez::graphics::ImageFormat;
use ggez::{
    event::{EventHandler},
    graphics::{Canvas, Image},
    Context, GameError,
};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::{
    native_api_backend,
    pixel_format::RgbAFormat,
    query,
    utils::{
        frame_formats, yuyv422_predicted_size, CameraFormat, CameraIndex,
        RequestedFormat, RequestedFormatType,
    },
    Buffer, Camera,
};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

struct CaptureState {
    receiver: Arc<Receiver<Buffer>>,
    buffer: Vec<u8>,
    format: CameraFormat,
}

impl EventHandler<GameError> for CaptureState {
    fn update(&mut self, _ctx: &mut Context) -> Result<(), GameError> {
        Ok(())
    }

    fn draw(&mut self, ctx: &mut Context) -> Result<(), GameError> {
        let buffer = self
            .receiver
            .recv()
            .map_err(|why| GameError::RenderError(why.to_string()))?;
        self.buffer
            .resize(yuyv422_predicted_size(buffer.buffer().len(), true), 0);
        buffer
            .decode_image_to_buffer::<RgbAFormat>(&mut self.buffer)
            .map_err(|why| GameError::RenderError(why.to_string()))?;
        let image = Image::from_pixels(
            ctx,
            &self.buffer,
            ImageFormat::Rgba8Uint,
            self.format.width(),
            self.format.height(),
        );
        let canvas = Canvas::from_image(ctx, image, None);
        canvas.finish(ctx)
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Clone)]
enum IndexKind {
    String(String),
    Index(u32),
}

impl FromStr for IndexKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.parse::<u32>() {
            Ok(p) => Ok(IndexKind::Index(p)),
            Err(_) => Ok(IndexKind::String(s.to_string())),
        }
    }
}

#[derive(Subcommand)]
enum Commands {
    ListDevices,
    ListProperties {
        device: Option<IndexKind>,
        kind: Option<PropertyKind>,
    },
}

enum CommandsProper {
    ListDevices,
    ListProperties {
        device: Option<IndexKind>,
        kind: PropertyKind,
    },
}

#[derive(Copy, Clone)]
enum PropertyKind {
    All,
    Controls,
    CompatibleFormats,
}

impl FromStr for PropertyKind {
    type Err = Report;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "All" | "ALL" | "all" => Ok(PropertyKind::All),
            "Controls" | "controls" | "CONTROLS" | "ctrls" => Ok(PropertyKind::Controls),
            "CompatibleFormats" | "compatibleformats" | "COMPATIBLEFORMATS" | "cf"
            | "compatfmts" => Ok(PropertyKind::CompatibleFormats),
            _ => Err(Report::msg(format!("unknown PropertyKind: {s}"))),
        }
    }
}

fn main() {
    nokhwa::nokhwa_initialize(|x| {
        if x {
            nokhwa_main()
        } else {
            eprintln!("failed to initialize camera library");
            std::process::exit(84);
        }
    });
    std::thread::sleep(Duration::from_millis(2000));
}

fn nokhwa_main() {
    let cli = Cli::parse();

    let cmd = match &cli.command {
        Some(cmd) => cmd,
        None => {
            println!("Unknown command \"\". Do --help for info.");
            return;
        }
    };

    let cmd = match cmd {
        Commands::ListDevices => CommandsProper::ListDevices,
        Commands::ListProperties { device, kind } => CommandsProper::ListProperties {
            device: device.clone(),
            kind: match kind {
                Some(k) => *k,
                None => {
                    println!("Expected Positional Argument \"All\", \"Controls\", or \"CompatibleFormats\"");
                    return;
                }
            },
        },
    };

    match cmd {
        CommandsProper::ListDevices => {
            let backend = native_api_backend().unwrap();
            let devices = query(backend).unwrap();
            println!("There are {} available cameras.", devices.len());
            for device in devices {
                println!("{device}");
            }
        }
        CommandsProper::ListProperties { device, kind } => {
            let index = match device.as_ref().unwrap_or(&IndexKind::Index(0)) {
                IndexKind::String(s) => CameraIndex::String(s.clone()),
                IndexKind::Index(i) => CameraIndex::Index(*i),
            };
            let mut camera = Camera::new(
                index,
                RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
            )
            .unwrap();
            match kind {
                PropertyKind::All => {
                    camera_print_controls(&camera);
                    camera_compatible_formats(&mut camera);
                }
                PropertyKind::Controls => {
                    camera_print_controls(&camera);
                }
                PropertyKind::CompatibleFormats => {
                    camera_compatible_formats(&mut camera);
                }
            }
        }
    }
}

fn camera_print_controls(cam: &Camera) {
    let ctrls = cam.camera_controls().unwrap();
    let index = cam.index();
    println!("Controls for camera {index}");
    for ctrl in ctrls {
        println!("{ctrl}")
    }
}

fn camera_compatible_formats(cam: &mut Camera) {
    for ffmt in frame_formats() {
        if let Ok(compatible) = cam.compatible_list_by_resolution(*ffmt) {
            println!("{ffmt}:");
            let mut formats = Vec::new();
            for (resolution, fps) in compatible {
                formats.push((resolution, fps));
            }
            formats.sort_by(|a, b| a.0.cmp(&b.0));
            for fmt in formats {
                let (resolution, res) = fmt;
                println!(" - {resolution}: {res:?}")
            }
        }
    }
}
