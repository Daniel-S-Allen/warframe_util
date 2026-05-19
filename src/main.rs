use ashpd::desktop::{
    SessionPortal,
    global_shortcuts::{GlobalShortcuts, NewShortcut},
};
use clap::Parser;
use ftail::Ftail;
use futures_util::{StreamExt, pin_mut};
use log::{LevelFilter, debug, error, info, trace};
use std::{
    collections::HashMap, fs::{self, File, OpenOptions}, path::Path
};

// const SHORTCUTS: &[(&str, &str)] = &[("manual_capture", "Perform Manual Capture")];

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    log_file: Option<String>,
}
impl std::fmt::Display for Args {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "--log_file: {}",
            self.log_file.clone().unwrap_or("".to_string()).to_string()
        )
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    setup_logger();
    let arguments = Args::parse();
    for arg in arguments.log_file.iter() {
        debug!("{}", arg);
    }
    info!("Running with arguments: {}", arguments);
    let keybinds = setup_keybinds().await.unwrap();
    debug!("Connected to keybinds");
    debug!("Listening to keybinds");
    let _ = key_event_loop(keybinds.0, keybinds.1, keybinds.2).await;
    Ok(())
}

fn setup_logger() {
    let log_path = Path::new("logs/demo.log");
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let logger = Ftail::new()
        .single_file(log_path, true, LevelFilter::Trace)
        .init();
    match logger {
        Ok(_) => info!("Logger created"),
        Err(_) => println!("Failed to create logger: {}", "test"),
    };
}

async fn setup_keybinds() -> Result<
    (
        GlobalShortcuts,
        ashpd::desktop::Session<GlobalShortcuts>,
        impl futures_util::Stream<Item = ashpd::desktop::global_shortcuts::Activated>
    ),
    ashpd::Error> {
    debug!("Setting up keybinds");
    let proxy = GlobalShortcuts::new().await?;
    debug!("Creating global shortcut session");
    let session = proxy.create_session(Default::default()).await?;
    debug!("Creating shortcuts");
    let shortcut = NewShortcut::new("manual_capture", "Perform Manual Capture");
    debug!("Binding shortcuts");
    let request = proxy.bind_shortcuts(&session, &[shortcut], None, Default::default()) .await?;
    request.response()?;
    debug!("Receiving stream");
    let stream = proxy.receive_activated().await?;
    Ok((proxy, session, stream))
}

async fn key_event_loop(
    proxy: ashpd::desktop::global_shortcuts::GlobalShortcuts, 
    session: ashpd::desktop::Session<ashpd::desktop::global_shortcuts::GlobalShortcuts>, 
    stream: impl futures_util::Stream<Item = ashpd::desktop::global_shortcuts::Activated>
) -> ashpd::Result<()> {
    pin_mut!(stream);
    while let Some(signal) = stream.next().await {
        match signal.shortcut_id(){
            "manual_capture" => capture_pressed(),
            _ => println!("Unknown"),
        }
    }
    
    Ok(())
}

fn capture_pressed(){
    debug!("Capture pressed");
}