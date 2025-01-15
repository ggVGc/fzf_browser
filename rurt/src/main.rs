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

        let output = Skim::run_with(&options, Some(rx))
            .map(|out| out.selected_items)
            .unwrap_or_default();

        throd.join().expect("panic")?;

        if output.is_empty() {
            return Ok(ExitCode::FAILURE);
        }

        let item = output.into_iter().next().expect("not empty");
        let item = (*item)
            .as_any()
            .downcast_ref::<FileName>()
            .expect("single type");
        if item.is_dir {
            here.push(&item.name);
        } else {
            println!("{}", here.join(&item.name).to_string_lossy());
            return Ok(ExitCode::SUCCESS);
        }
    }
}
