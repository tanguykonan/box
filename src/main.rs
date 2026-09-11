mod builder;
mod config;
mod format;
mod inspector;
mod manifest;
mod registry;
mod runner;
mod runtimes;
mod ui;
mod volume;

use clap::{Parser, Subcommand};
use console::style;
use std::process::exit;

#[derive(Parser, Debug)]
#[command(
    name = "box",
    version = "0.2.0",
    disable_version_flag = true,
    about = "Box - Autonomous Application Packaging & Runtime Engine",
    long_about = "Box • Autonomous Application Packaging & Runtime Engine\nPackage, distribute, and execute Python and Node.js/TypeScript applications\nas hermetic, self-contained .box archives without virtual environments or host runtime installs.",
    after_help = "Commands:
  build       Package a project directory into a standalone .box archive
  run         Execute a .box archive locally or auto-pull & run from registry
  info        Inspect .box archive manifest, runtime specs, volumes, and integrity
  push        Publish a .box package to Box Cloud Registry
  pull        Download a .box package from Box Cloud Registry
  login       Authenticate CLI with Box Cloud Registry
  logout      Log out and clear stored registry credentials
  whoami      Display current authenticated user identity
  volume      Manage persistent sandbox storage volumes
  config      Manage global settings, mirrors, and defaults

Environment Variables:
  BOX_HUB_URL             Default Box Registry Hub URL
  BOX_HOME                Base storage directory (default: ~/.box)
  BOX_RUNTIMES_DIR        Cached runtimes directory (default: ~/.box/runtimes)
  BOX_VOLUMES_DIR         Persistent volumes directory (default: ~/.box/volumes)
  BOX_COMPRESSION_LEVEL   Zstandard compression level [1-22] (default: 3)
  BOX_NETWORK_TIMEOUT     Registry network timeout in seconds (default: 60)

Quick Examples:
  $ box build                             # Build .box from current directory
  $ box info myapp.box                    # Inspect manifest & environment schema
  $ box run myapp.box                     # Execute local .box application
  $ box run myapp.box -v data:/app/data   # Mount persistent named volume
  $ box run myapp.box -p 8080 -H 0.0.0.0  # Run with port & host binding
  $ box run user/myapp:latest             # Auto-pull and run from registry
  $ box push user/myapp:1.0.0 --public    # Publish public box to registry
  $ box volume create my_data             # Create named persistent volume

Online Documentation:
  https://boxhub.paxiz.org/docs
"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Show program version
    #[arg(short = 'v', short_alias = 'V', long = "version", action = clap::ArgAction::Version)]
    version: Option<bool>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build a hermetic .box archive from a project directory
    Build {
        /// Path to project directory containing boxconfig.yml (default: current directory)
        #[arg(default_value = ".")]
        path: String,
    },

    /// Inspect .box archive metadata, runtime specs, volumes, and integrity
    Info {
        /// Path to the .box file to inspect
        box_file: String,

        /// Output inspection result as raw JSON
        #[arg(long)]
        json: bool,
    },

    /// Execute a .box file locally or auto-pull & run from registry
    Run {
        /// Path to .box file, local package name, or registry tag (<user>/<pkg>:<tag>)
        box_file: String,

        /// Path to environment file (default: .env.local)
        #[arg(long, default_value = ".env.local")]
        env: String,

        /// Mount a persistent named volume or host directory (e.g. -v my_db:data)
        #[arg(short = 'v', long = "volume", value_name = "NAME:TARGET")]
        volumes: Vec<String>,

        /// Working directory inside the sandbox (default from package manifest)
        #[arg(short = 'w', long = "workdir", value_name = "DIR")]
        workdir: Option<String>,

        /// Port number to bind application server (e.g. 8000, 3000)
        #[arg(short = 'p', long = "port")]
        port: Option<u16>,

        /// Host IP address to bind application server (default: 127.0.0.1)
        #[arg(short = 'H', long = "host")]
        host: Option<String>,

        /// Perform deep per-file SHA-256 integrity verification before launch
        #[arg(long)]
        verify_files: bool,
    },

    /// Log in and authenticate with Box Cloud Registry
    Login {
        /// Box Personal Access Token
        #[arg(long)]
        token: Option<String>,

        /// Custom Box Hub URL (overrides BOX_HUB_URL)
        #[arg(long)]
        hub: Option<String>,
    },

    /// Log out and remove stored registry credentials
    Logout {
        /// Specific Box Hub URL to log out from
        #[arg(long)]
        hub: Option<String>,
    },

    /// Display currently authenticated Box Hub user and endpoint
    Whoami,

    /// Push and publish a .box package to Box Cloud Registry
    Push {
        /// Registry tag to push (<username>/<package>:<version> or <package>)
        tag: String,

        /// Explicit path to .box file to push (auto-detected if omitted)
        file: Option<String>,

        /// Publish package with public visibility
        #[arg(long)]
        public: bool,

        /// Publish package with private visibility (default)
        #[arg(long)]
        private: bool,
    },

    /// Pull and download a .box package from Box Cloud Registry
    Pull {
        /// Package tag to pull (<username>/<package>:<version>)
        tag: String,

        /// Output destination file path (default: <package>.box)
        #[arg(short = 'o', long = "output")]
        output: Option<String>,
    },

    /// Manage Box persistent storage volumes
    Volume {
        #[command(subcommand)]
        cmd: Option<VolumeCommands>,
    },

    /// Manage Box global settings, mirrors, and defaults
    Config {
        #[command(subcommand)]
        cmd: Option<ConfigCommands>,
    },
}

#[derive(Subcommand, Debug)]
enum VolumeCommands {
    /// Create a new named volume
    Create {
        /// Name of the volume to create
        name: String,
    },
    /// List all persistent volumes
    #[command(alias = "list")]
    Ls,
    /// Display detailed volume metadata and size
    Inspect {
        /// Name of the volume to inspect
        name: String,
    },
    /// Remove one or more named volumes
    Rm {
        /// Name(s) of volume(s) to remove
        #[arg(required = true)]
        names: Vec<String>,
    },
    /// Remove all empty or unused volumes
    Prune,
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// List all global configuration settings
    #[command(alias = "ls")]
    List,
    /// Get value of a configuration key
    Get {
        /// Configuration key name
        key: String,
    },
    /// Set value of a configuration key
    Set {
        /// Configuration key name
        key: String,
        /// Configuration value
        value: String,
    },
    /// Delete / unset a configuration key
    #[command(alias = "unset", alias = "rm")]
    Delete {
        /// Configuration key name
        key: String,
    },
}

fn main() {
    config::bootstrap_dotenv();

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Build { path }) => {
            if let Err(e) = builder::build(Some(&path)) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Info { box_file, json }) => {
            if let Err(e) = inspector::inspect_box(&box_file, json) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Run {
            box_file,
            env,
            volumes,
            workdir,
            port,
            host,
            verify_files,
        }) => {
            match runner::run(
                &box_file,
                Some(&env),
                &volumes,
                port,
                host.as_deref(),
                workdir.as_deref(),
                verify_files,
            ) {
                Ok(code) => exit(code),
                Err(e) => {
                    eprintln!("{}: {}", style("Error").red().bold(), e);
                    exit(1);
                }
            }
        }
        Some(Commands::Login { token, hub }) => {
            if let Err(e) = registry::login(token.as_deref(), hub.as_deref()) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Logout { hub }) => {
            if let Err(e) = registry::logout(hub.as_deref()) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Whoami) => {
            if let Err(e) = registry::whoami() {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Push {
            tag,
            file,
            public,
            private,
        }) => {
            let is_priv = if private { Some(true) } else { None };
            if let Err(e) = registry::push(&tag, file.as_deref(), public, is_priv) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Pull { tag, output }) => {
            if let Err(e) = registry::pull(&tag, output.as_deref()) {
                eprintln!("{}: {}", style("Error").red().bold(), e);
                exit(1);
            }
        }
        Some(Commands::Volume { cmd }) => match cmd {
            Some(VolumeCommands::Create { name }) => volume::handle_volume_create(&name),
            Some(VolumeCommands::Ls) | None => volume::handle_volume_ls(),
            Some(VolumeCommands::Inspect { name }) => volume::handle_volume_inspect(&name),
            Some(VolumeCommands::Rm { names }) => volume::handle_volume_rm(&names),
            Some(VolumeCommands::Prune) => volume::handle_volume_prune(),
        },
        Some(Commands::Config { cmd }) => match cmd {
            Some(ConfigCommands::List) | None => {
                let cfg = config::load_config();
                println!(
                    "{:<25} {}",
                    style("KEY").bold().cyan(),
                    style("VALUE").bold().cyan()
                );
                println!("{}", style("-".repeat(50)).dim());
                if let Some(h) = cfg.hub_url {
                    println!("{:<25} {}", "hub_url", style(h).bold().white());
                }
                if let Some(c) = cfg.compression_level {
                    println!("{:<25} {}", "compression_level", c);
                }
                if let Some(w) = cfg.build_workers {
                    println!("{:<25} {}", "build_workers", w);
                }
                if let Some(t) = cfg.network_timeout {
                    println!("{:<25} {}", "network_timeout", t);
                }
                if let Some(m) = cfg.python_mirror {
                    println!("{:<25} {}", "python_mirror", m);
                }
                if let Some(m) = cfg.node_mirror {
                    println!("{:<25} {}", "node_mirror", m);
                }
                for (k, v) in cfg.custom {
                    println!("{:<25} {}", k, v);
                }
            }
            Some(ConfigCommands::Get { key }) => {
                if let Some(val) = config::get_setting(&key) {
                    println!(
                        "{} = {}",
                        style(&key).cyan().bold(),
                        style(val).green().bold()
                    );
                } else {
                    println!("{}", style(format!("Key '{}' is not set.", key)).dim());
                }
            }
            Some(ConfigCommands::Set { key, value }) => {
                if let Err(e) = config::set_setting(&key, &value) {
                    eprintln!(
                        "{}: Could not save setting: {}",
                        style("Error").red().bold(),
                        e
                    );
                    exit(1);
                }
                println!("{} Set {} = {}", style("[+]").green().bold(), key, value);
            }
            Some(ConfigCommands::Delete { key }) => {
                if let Err(e) = config::delete_setting(&key) {
                    eprintln!(
                        "{}: Could not delete setting: {}",
                        style("Error").red().bold(),
                        e
                    );
                    exit(1);
                }
                println!("{} Unset setting '{}'", style("[+]").green().bold(), key);
            }
        },
        None => {
            eprintln!("Type 'box --help' for available commands.");
            exit(1);
        }
    }
}
