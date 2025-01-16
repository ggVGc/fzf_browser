use anyhow::{anyhow, Context};
use anyhow::{bail, Result};
use clap::Parser;
use skim::prelude::*;
use std::ffi::OsString;
use std::fs::FileType;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::{fs, thread};
use tuikit::attr::{Attr, Color};

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,
}

struct FileName {
    name: OsString,
    file_type: FileType,
}

impl SkimItem for FileName {
    fn text(&self) -> Cow<str> {
        self.name.to_string_lossy()
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        let s = self.name.to_string_lossy().to_string();
        if self.file_type.is_dir() {
            colour_whole(s, Color::LIGHT_BLUE)
        } else if self.file_type.is_symlink() {
            colour_whole(s, Color::LIGHT_CYAN)
        } else if self.file_type.is_file() {
            s.into()
        } else {
            colour_whole(s, Color::LIGHT_RED)
        }
    }
}

fn colour_whole(s: String, attr: impl Into<Attr>) -> AnsiString<'static> {
    let whole = (0, s.len() as u32);
    AnsiString::new_string(s, vec![(attr.into(), whole)])
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut here = fs::canonicalize(cli.start_path).context("start path")?;

    let mut options = SkimOptions::default();
    options.no_clear = true;

    let handled_keys = [
        Key::Left,
        Key::Ctrl('h'),
        Key::Right,
        Key::Ctrl('l'),
        Key::Ctrl('d'),
    ];

    for key in handled_keys {
        options.bind.push(format!("{}:abort", render_key(key)));
    }

    loop {
        let (tx, rx) = unbounded::<Arc<dyn SkimItem>>();
        options.prompt = format!("{} > ", here.to_string_lossy());
        let here_copy = here.clone();
        let streamer = thread::spawn(move || stream_content(tx, here_copy));

        let output = Skim::run_with(&options, Some(rx)).ok_or_else(|| anyhow!("skim said NONE"))?;

        streamer.join().expect("panic")?;

        let mut requested_navigation = false;

        match output.final_key {
            Key::Left | Key::Ctrl('h') => {
                here.pop();
                continue;
            }
            Key::Right | Key::Ctrl('l') => {
                requested_navigation = true;
            }
            Key::Ctrl('d') => {
                here = dirs::home_dir()
                    .ok_or_else(|| anyhow!("but you don't even have a home dir"))?;
                continue;
            }
            _ => {
                if output.is_abort {
                    return Ok(ExitCode::FAILURE);
                }
            }
        }

        let item = match output.selected_items.into_iter().next() {
            Some(item) => item,
            None => return Ok(ExitCode::FAILURE),
        };

        let item = (*item)
            .as_any()
            .downcast_ref::<FileName>()
            .expect("single type");

        if requested_navigation {
            if let Ok(cand) = ensure_directory(here.join(&item.name)) {
                here = cand;
            }
        } else {
            println!("{}", here.join(&item.name).to_string_lossy());
            return Ok(ExitCode::SUCCESS);
        }
    }
}

fn stream_content(tx: Sender<Arc<dyn SkimItem>>, src: impl AsRef<Path>) -> Result<()> {
    for f in fs::read_dir(src)? {
        let f = f?;
        let name = f.file_name();
        let file_type = f
            .file_type()
            .with_context(|| anyhow!("retrieving type of {:?}", &name))?;

        // err: disconnected
        if tx.send(Arc::new(FileName { name, file_type })).is_err() {
            break;
        }
    }
    Ok(())
}

fn ensure_directory(p: impl AsRef<Path>) -> Result<PathBuf> {
    let canon = fs::canonicalize(p)?;
    if !fs::metadata(&canon)?.is_dir() {
        bail!("resolved path is not a directory");
    }

    Ok(canon)
}

fn render_key(key: Key) -> String {
    use Key::*;
    match key {
        Left => "left".to_string(),
        Right => "right".to_string(),

        Ctrl(c) => format!("ctrl-{c}"),
        other => unimplemented!("no rendering for {other:?}"),
    }
}
