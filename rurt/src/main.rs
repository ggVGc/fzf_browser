use anyhow::anyhow;
use anyhow::Result;
use clap::Parser;
use skim::prelude::*;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;
use std::{fs, thread};
use tuikit::attr::Color;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[clap(default_value = ".")]
    start_path: OsString,
}

struct FileName {
    name: OsString,
    is_dir: bool,
}

impl SkimItem for FileName {
    fn text(&self) -> Cow<str> {
        self.name.to_string_lossy()
    }

    fn display<'a>(&'a self, _context: DisplayContext<'a>) -> AnsiString<'a> {
        let s = self.name.to_string_lossy().to_string();
        if self.is_dir {
            let whole = (0, s.len() as u32);
            AnsiString::new_string(s, vec![(Color::RED.into(), whole)])
        } else {
            AnsiString::from(s)
        }
    }
}

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();
    let mut here = PathBuf::from(cli.start_path);

    let mut options = SkimOptions::default();
    options.no_clear = true;
    options.bind.push("left:abort".to_string());
    options.bind.push("right:accept".to_string());

    loop {
        let (tx, rx) = unbounded::<Arc<dyn SkimItem>>();
        options.prompt = format!("{} > ", here.to_string_lossy());
        let here_copy = here.clone();
        let throd = thread::spawn(move || -> Result<()> {
            tx.send(Arc::new(FileName {
                name: OsString::from(".."),
                is_dir: true,
            }))?;

            for f in fs::read_dir(here_copy)? {
                let f = f?;
                let is_dir = f.file_type()?.is_dir();
                let name = f.file_name();
                tx.send(Arc::new(FileName { name, is_dir }))?;
            }
            Ok(())
        });

        let output = Skim::run_with(&options, Some(rx)).ok_or_else(|| anyhow!("skim said NONE"))?;

        // if output.is_abort {
        //     return Ok(ExitCode::FAILURE);
        // }

        let mut requested_directory = false;

        match output.final_key {
            Key::Left => {
                here.push("..");
                continue;
            }
            Key::Right => {
                requested_directory = true;
            }
            _ => {
                if output.is_abort {
                    return Ok(ExitCode::FAILURE);
                }
            }
        }

        throd.join().expect("panic")?;

        let item = match output.selected_items.into_iter().next() {
            Some(item) => item,
            None => return Ok(ExitCode::FAILURE),
        };

        let item = (*item)
            .as_any()
            .downcast_ref::<FileName>()
            .expect("single type");

        if requested_directory && item.is_dir {
            here.push(&item.name);
        } else if !requested_directory {
            println!("{}", here.join(&item.name).to_string_lossy());
            return Ok(ExitCode::SUCCESS);
        }
    }
}
