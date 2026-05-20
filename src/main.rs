use anyhow::Error;
use ashpd::desktop::{
    Session, SessionPortal,
    global_shortcuts::{Activated, GlobalShortcuts, NewShortcut},
};
use clap::{Parser, ValueEnum};
use ftail::Ftail;
use futures_util::{StreamExt, pin_mut};
use image::{GrayImage, ImageReader};
use imageproc::contrast::{ThresholdType, otsu_level, threshold_mut};
use log::{LevelFilter, debug, error, info, trace};
use std::{
    fs::{self, File, OpenOptions},
    path::Path,
};
use tesseract::Tesseract;
#[derive(Default, Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Path to warframe's EE.log file
    #[arg(short, long)]
    log_file: Option<String>,
    /// How to output messages from the application
    #[arg(short, long, value_enum, default_value_t = OutputType::default())]
    output: OutputType,

    /// Application name to screenshot
    #[arg(short, long, default_value = "KCalc")]
    app_name: String,
}
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutputType {
    Console,
    #[default]
    Formatted,
    File,
}

impl std::fmt::Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut needs_comma: bool = false;
        if self.log_file.is_some() {
            let _ = write!(f, "--log-file \"{}\"", self.log_file.as_deref().unwrap());
            needs_comma = true;
        }

        let _ = write!(
            f,
            "{}--output {:?}",
            if needs_comma { ", " } else { "" },
            self.output
        );

        let _ = write!(f, ", --app-name {:?}", self.app_name);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments = Args::parse();
    setup_logger(&arguments);
    info!("Running with arguments: {}", arguments);
    let keybinds = setup_keybinds().await.unwrap();
    debug!("Connected to keybinds");
    debug!("Listening to keybinds");
    let _ = key_event_loop(keybinds.0, keybinds.1, keybinds.2, &arguments.app_name).await;
    Ok(())
}

fn setup_logger(arguments: &Args) {
    let log_path = Path::new("logs/demo.log");
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let logger = match arguments.output {
        OutputType::Console => Ftail::new().console(LevelFilter::Debug).init(),
        OutputType::File => Ftail::new()
            .single_file(log_path, true, LevelFilter::Trace)
            .init(),
        OutputType::Formatted => Ftail::new().formatted_console(LevelFilter::Debug).init(),
        _ => Ftail::new().console(LevelFilter::Debug).init(),
    };
    match logger {
        Ok(_) => info!("Logger created"),
        Err(_) => println!("Failed to create logger: {}", "test"),
    };
}

async fn setup_keybinds() -> Result<
    (
        GlobalShortcuts,
        Session<GlobalShortcuts>,
        impl futures_util::Stream<Item = Activated>,
    ),
    ashpd::Error,
> {
    debug!("Setting up keybinds");
    let proxy = GlobalShortcuts::new().await?;
    debug!("Creating global shortcut session");
    let session = proxy.create_session(Default::default()).await?;
    debug!("Creating shortcuts");
    let shortcut = NewShortcut::new("manual_capture", "Perform Manual Capture");
    debug!("Binding shortcuts");
    let request = proxy
        .bind_shortcuts(&session, &[shortcut], None, Default::default())
        .await?;
    request.response()?;
    debug!("Receiving stream");
    let stream = proxy.receive_activated().await?;
    Ok((proxy, session, stream))
}

async fn key_event_loop(
    _proxy: GlobalShortcuts,
    _session: Session<GlobalShortcuts>,
    stream: impl futures_util::Stream<Item = Activated>,
    window_name: &str,
) -> ashpd::Result<()> {
    pin_mut!(stream);
    let mut image_number: u16 = 0;
    while let Some(signal) = stream.next().await {
        match signal.shortcut_id() {
            "manual_capture" => {
                image_number += 1;
                capture_pressed(window_name, image_number)
            },
            _ => println!("Unknown"),
        }
    }

    Ok(())
}

fn capture_pressed(window_name: &str, image_number: u16) {
    debug!("Capture pressed");
    let window = find_window_by_name(window_name);
    if window.is_some() {
        take_screenshot(&window.unwrap(), image_number);
        let path_str = format!("target/windows/window-{}-original.png", &image_number);
        let text = ocr(Path::new(&path_str), image_number);
        match text{
            Ok(text) => debug!("Extracted text: \"{}\"", text),
            Err(err) => debug!("Could not extract text: {:?}", err)
        }
        
    }
}
use fs_extra::dir;
use xcap::Window;
fn find_window_by_name(name: &str) -> Option<Window> {
    let windows = Window::all().unwrap().to_owned();
    let windows_string = windows
        .iter()
        .map(|w| match w.title() {
            Ok(val) => format!("\"{}\"", val),
            _ => String::from("Unknown"),
        })
        .reduce(|mut string, title| {
            string.push_str(&format!(", {}", &title));
            return string;
        });
    debug!(
        "Found windows: {}",
        windows_string.unwrap_or("No windows found".to_string())
    );
    let window = windows.into_iter().find(|w| {
        w.title()
            .as_deref()
            .unwrap_or_default()
            .trim()
            .eq_ignore_ascii_case(name)
    });
    if window.is_some() {
        debug!("Found window titled {}", name);
    } else {
        debug!("Could not find window titled {}", name);
    }
    return window;
}

fn take_screenshot(window: &Window, image_number: u16) {
    dir::create_all("target/windows", false).unwrap();
    if !window.is_minimized().unwrap() {
        let image = window.capture_image().unwrap();
        image
            .save(format!(
                "target/windows/capture-{}-original.png",
                image_number
            ))
            .unwrap();
        debug!("Took screenshot");
    }
}
fn preprocess(image_path: &Path) -> Result<GrayImage, Error> {
    let image = ImageReader::open(image_path)?.decode()?;
    let mut grayscale = image.to_luma8();
    let threshold = otsu_level(&grayscale);
    threshold_mut(&mut grayscale, threshold, ThresholdType::Binary);
    return Ok(grayscale);
}
fn ocr(image_path: &Path, image_number: u16) -> Result<String, Error> {
    let mut tesseract = Tesseract::new(None, Some("eng"))?;
    let grayscale = preprocess(image_path)?;
    // let temp_path = std::env::temp_dir().join("warframe_util_processed.png");
    let temp_path = format!("target/windows/capture-{}-processed.png", image_number);
    let _ = grayscale.save(&temp_path);
    tesseract = tesseract.set_image(&temp_path.to_string())?;
    tesseract = tesseract.set_variable(
        "tessedit_char_whitelist",
        // "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz ",
        "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz 123456789,+",
    )?;
    let text = tesseract.get_text()?;
    return Ok(text);
}
