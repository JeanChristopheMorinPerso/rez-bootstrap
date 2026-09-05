use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

mod http;
mod install;
mod list;
mod package;

#[derive(Debug, Parser)]
#[command(
    name = "rezup",
    version,
    about,
    disable_version_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Print version.
    #[arg(short = 'v', long = "version", action = clap::ArgAction::Version)]
    version: Option<bool>,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Bootstrap rez and its managed Python runtime.
    Bootstrap,
    /// Install a rez version into a prefix.
    Install(InstallArgs),
    /// List rez versions available for installation.
    List(ListArgs),
    /// Update the active rez installation.
    Update,
    /// Manage packages used by rez.
    Package(PackageArgs),
    /// Manage rezup itself.
    Self_(SelfArgs),
}

#[derive(Debug, Args)]
#[command(allow_missing_positional = true)]
struct InstallArgs {
    /// Python version selector.
    #[arg(long)]
    python_version: Option<String>,
    /// Python platform selector.
    #[arg(long)]
    python_platform: Option<String>,
    /// Python architecture selector.
    #[arg(long)]
    python_arch: Option<String>,
    /// Python microarchitecture selector.
    #[arg(long)]
    python_microarch: Option<String>,
    /// Python build mode selector.
    #[arg(long, value_enum)]
    python_mode: Option<BuildMode>,
    /// Python C library selector.
    #[arg(long)]
    python_libc: Option<String>,
    /// Rez version to install (defaults to latest).
    #[arg(default_value = "latest")]
    version: String,
    /// Installation prefix.
    path: PathBuf,
}

#[derive(Debug, Args)]
struct ListArgs {
    /// Emit machine-readable JSON.
    #[arg(long)]
    json: bool,
}

#[derive(Debug, Args)]
struct PackageArgs {
    /// Path to the rez executable (PATH lookup will be implemented later).
    #[arg(long, global = true)]
    rez: Option<PathBuf>,
    #[command(subcommand)]
    command: PackageCommand,
}

#[derive(Debug, Subcommand)]
enum PackageCommand {
    /// Create a rez system package.
    Create(PackageCreateArgs),
    /// List rez system packages.
    List(PackageListArgs),
}

#[derive(Debug, Args)]
struct PackageCreateArgs {
    #[command(subcommand)]
    command: PackageCreateCommand,
}

#[derive(Debug, Subcommand)]
enum PackageCreateCommand {
    /// Create an architecture package.
    Arch(SystemPackageArgs),
    /// Create an operating-system package.
    Os(SystemPackageArgs),
    /// Create a platform package.
    Platform(SystemPackageArgs),
    /// Create a Python package.
    Python(PythonPackageArgs),
}

#[derive(Debug, Args)]
struct SystemPackageArgs {
    /// Package version.
    version: Option<String>,
    /// Select the release build.
    #[arg(short, long)]
    release: bool,
}

#[derive(Debug, Args)]
struct PythonPackageArgs {
    /// Python version.
    version: Option<String>,
    /// Python build mode.
    #[arg(short, long, value_enum)]
    mode: Option<BuildMode>,
    /// Target architecture.
    #[arg(short, long)]
    arch: Option<String>,
    /// Target microarchitecture.
    #[arg(long)]
    microarch: Option<String>,
    /// Target platform.
    #[arg(short, long)]
    platform: Option<String>,
    /// Target C library.
    #[arg(long)]
    libc: Option<String>,
    /// Select the release build.
    #[arg(short, long)]
    release: bool,
}

#[derive(Clone, Debug, ValueEnum)]
enum BuildMode {
    Debug,
    Release,
}

#[derive(Clone, Debug, ValueEnum)]
enum PackageComponent {
    Python,
    Os,
    Arch,
    Platform,
}

#[derive(Debug, Args)]
struct PackageListArgs {
    /// Limit output to one component type.
    #[arg(value_enum)]
    component: Option<PackageComponent>,
}

#[derive(Debug, Args)]
struct SelfArgs {
    #[command(subcommand)]
    command: SelfCommand,
}

#[derive(Debug, Subcommand)]
enum SelfCommand {
    /// Update rezup itself.
    Update,
}

impl Command {
    fn action(self) -> &'static str {
        match self {
            Command::Bootstrap => "rezup bootstrap",
            Command::Install(_) => "rezup install",
            Command::Update => "rezup update",
            Command::Package(package) => match package.command {
                PackageCommand::Create(create) => match create.command {
                    PackageCreateCommand::Arch(_) => "rezup package create arch",
                    PackageCreateCommand::Os(_) => "rezup package create os",
                    PackageCreateCommand::Platform(_) => "rezup package create platform",
                    PackageCreateCommand::Python(_) => "rezup package create python",
                },
                PackageCommand::List(_) => "rezup package list",
            },
            Command::Self_(self_command) => match self_command.command {
                SelfCommand::Update => "rezup self update",
            },
            Command::List(_) => "rezup list",
        }
    }
}

impl Cli {
    fn run(self) -> Result<(), String> {
        match self.command {
            Command::Install(args) => {
                install::run(args).map_err(|error| format!("rezup install failed: {error:#}"))
            }
            Command::List(args) => {
                list::run(args.json).map_err(|error| format!("rezup list failed: {error}"))
            }
            Command::Package(PackageArgs {
                rez,
                command:
                    PackageCommand::Create(PackageCreateArgs {
                        command: PackageCreateCommand::Python(args),
                    }),
            }) => package::create_python(rez, args)
                .map_err(|error| format!("rezup package create python failed: {error:#}")),
            command => Err(format!("{} is not implemented", command.action())),
        }
    }
}

fn main() -> ExitCode {
    match Cli::parse().run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
