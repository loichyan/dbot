use clap::Parser;
use cli::{Cli, Command};
use dbot::{compile::CompilerOptions, Merge};
use directories::{BaseDirs, ProjectDirs};
use history::HistoryManager;
use once_cell::unsync::OnceCell;
use options::Options;
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use template::TeraRenderer;
use thisctx::WithContext;
use tracing::info;

mod cli;
mod error;
mod history;
mod options;
mod profile;
mod template;

const F_CONFIG: &str = "config.yaml";
const F_HISTORY: &str = "history.yaml";
const F_PROFILE: &str = "dbot.yaml";

struct Dirs {
    home: PathBuf,
    config: PathBuf,
    data: PathBuf,
}

#[derive(Default)]
struct Runtime {
    dirs: OnceCell<Dirs>,
    options: OnceCell<Options>,
    history: OnceCell<HistoryManager>,
}

#[extend::ext]
impl PathBuf {
    fn expand_tilde(&mut self, home: &Path) {
        if self.starts_with("~") {
            let expanded = home.join(self.strip_prefix("~").unwrap());
            *self = expanded;
        }
    }
}

impl Runtime {
    fn dirs(&self) -> error::Result<&Dirs> {
        self.dirs.get_or_try_init(|| {
            let base_dirs = BaseDirs::new().context(error::CannotGetDirectory)?;
            let proj_dirs = ProjectDirs::from("", "", "Dbot").context(error::CannotGetDirectory)?;
            Ok(Dirs {
                home: base_dirs.home_dir().to_owned(),
                config: proj_dirs.config_dir().to_owned(),
                data: proj_dirs.data_local_dir().to_owned(),
            })
        })
    }

    fn options(&self) -> error::Result<&Options> {
        let dirs = self.dirs()?;
        self.options.get_or_try_init(|| {
            let path = dirs.config.join(F_CONFIG);
            let mut opts = if path.exists() {
                let content = std::fs::read_to_string(&path).context(error::Io(&path))?;
                serde_yaml::from_str::<Options>(&content).context(error::Yaml(&path))?
            } else {
                Default::default()
            };
            opts.source
                .get_or_insert_with(|| dirs.data.clone())
                .expand_tilde(&dirs.home);
            opts.target
                .get_or_insert_with(|| dirs.home.clone())
                .expand_tilde(&dirs.home);
            Ok(opts)
        })
    }

    fn options_mut(&mut self) -> error::Result<&mut Options> {
        self.options()?;
        Ok(self.options.get_mut().unwrap())
    }

    fn history(&self) -> error::Result<&HistoryManager> {
        let dirs = self.dirs()?;
        self.history.get_or_try_init(|| {
            let path = dirs.data.join(F_HISTORY);
            if !path.exists() {
                return Ok(Default::default());
            }
            let content = std::fs::read_to_string(&path).context(error::Io(&path))?;
            serde_yaml::from_str(&content).context(error::Yaml(&path))
        })
    }

    fn save_histroy(&self) -> error::Result<()> {
        let history = self.history()?;
        let data_dir = &self.dirs()?.data;
        let path = data_dir.join(F_HISTORY);
        let content = serde_yaml::to_string(history).context(error::Yaml(&path))?;
        (|| -> std::io::Result<_> {
            std::fs::create_dir_all(data_dir)?;
            std::fs::File::create(&path)?.write_all(content.as_bytes())?;
            Ok(())
        })()
        .context(error::Io(&path))?;
        Ok(())
    }

    fn history_mut(&mut self) -> error::Result<&mut HistoryManager> {
        self.history()?;
        Ok(self.history.get_mut().unwrap())
    }

    fn apply(&mut self) -> error::Result<()> {
        // TODO: remove files on conflicts
        self.clean()?;
        let options = self.options()?;
        let source = options.source();
        let target = options.target();
        let profile = self.load_profile(source)?;
        let mut renderer = TeraRenderer::default();
        renderer.add_data("data", &profile.content.data);
        let entries = dbot::compile(
            &CompilerOptions { source, target },
            profile.content.profile.unwrap().into_entries()?,
        )?;
        dbot::apply(&mut renderer, &entries)?;
        self.history_mut()?.push(entries);
        self.save_histroy()?;
        Ok(())
    }

    fn clean(&mut self) -> error::Result<()> {
        let Some(last) = self.history_mut()?.pop() else { return Ok(()) };
        info!("Clean history created at '{}'", last.timespan);
        for (target, _) in last.entries.iter() {
            if target.exists() {
                std::fs::remove_file(target).context(error::Io(&target))?;
            }
        }
        Ok(())
    }

    fn ls(&self) -> error::Result<()> {
        let Some(last) = self.history()?.last() else { return Ok(()) };
        for (target, _) in last.entries.iter() {
            println!("{}", target.display());
        }
        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    let subscriber = tracing_subscriber::FmtSubscriber::new();
    tracing::subscriber::set_global_default(subscriber)?;
    let args = Cli::parse();
    let mut rt = Runtime::default();
    // Override default options.
    rt.options_mut()?.merge(args.options);
    match args.cmd {
        Command::Apply => rt.apply()?,
        Command::Clean => rt.clean()?,
        Command::Ls => rt.ls()?,
    }
    Ok(())
}
