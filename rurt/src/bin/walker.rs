use anyhow::Result;
use anyhow::Context;
use clap::Parser;
use skim::prelude::*;
use std::ffi::OsString;
use std::process::ExitCode;
use std::{fs, thread};

use rurt::walk::{stream_content, Mode, ReadOpts, Recursion};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,

    #[clap(short, long)]
    query: Option<String>,

    #[clap(short, long)]
    recursive: bool,

    /// default: mixed (when non-recursive), files (when recursive)
    #[clap(short, long)]
    mode: Option<Mode>,
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let here = fs::canonicalize(cli.start_path).context("start path")?;

    let mut options = SkimOptionsBuilder::default()
        .reverse(true)
        .no_clear(true)
        .query(cli.query)
        .build()
        .unwrap();

    let mut read_opts = ReadOpts {
        target_dir: here.clone(),
        ..Default::default()
    };

    read_opts.sort = true;

    if let Some(mode) = cli.mode {
        read_opts.mode_index = mode as usize;
    } else if cli.recursive {
        read_opts.mode_index = Mode::Files as usize;
    } else {
        read_opts.mode_index = Mode::Mixed as usize;
    }

    if cli.recursive {
        read_opts.recursion_index = Recursion::All as usize;
    }

    if here.as_os_str().as_encoded_bytes().len()
        < read_opts.target_dir.as_os_str().as_encoded_bytes().len()
    {
        read_opts.target_dir.clone_from(&here);
    }

    let (tx, rx) = unbounded::<Arc<dyn SkimItem>>();
    options.prompt = format!("{} > ", here.to_string_lossy());
    let here_copy = here.clone();
    let read_opts_copy = read_opts.clone();
    let streamer = thread::spawn(move || stream_content(tx, here_copy, &read_opts_copy));

    while let Ok(entry) = rx.recv() {
        println!("|{}|", entry.text());
    }

    streamer.join().expect("panic");

    Ok(ExitCode::SUCCESS)
}
