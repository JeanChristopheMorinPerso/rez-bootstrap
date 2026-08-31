use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "rezup", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Bootstrap Rez and its managed Python runtime.
    Bootstrap,
    /// Install a Rez version into a prefix.
    Install(InstallArgs),
    /// List Rez versions available for installation.
    List(ListArgs),
    /// Update the active Rez installation.
    Update,
    /// Manage packages used by Rez.
    Package(PackageArgs),
    /// Manage rezup itself.
    Self_(SelfArgs),
}

#[derive(Debug, Args)]
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
    /// Rez version to install.
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
    /// Path to the Rez executable (PATH lookup will be implemented later).
    #[arg(long, global = true)]
    rez: Option<PathBuf>,
    #[command(subcommand)]
    command: PackageCommand,
}

#[derive(Debug, Subcommand)]
enum PackageCommand {
    /// Install a Rez system package.
    Install(PackageInstallArgs),
    /// List Rez system packages.
    List(PackageListArgs),
}

#[derive(Debug, Args)]
struct PackageInstallArgs {
    #[command(subcommand)]
    command: PackageInstallCommand,
}

#[derive(Debug, Subcommand)]
enum PackageInstallCommand {
    /// Install an architecture package.
    Arch(SystemPackageArgs),
    /// Install an operating-system package.
    Os(SystemPackageArgs),
    /// Install a platform package.
    Platform(SystemPackageArgs),
    /// Install a Python package.
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

impl Cli {
    fn action(self) -> &'static str {
        match self.command {
            Command::Bootstrap => "rezup bootstrap",
            Command::Install(_) => "rezup install",
            Command::List(_) => "rezup list",
            Command::Update => "rezup update",
            Command::Package(package) => match package.command {
                PackageCommand::Install(install) => match install.command {
                    PackageInstallCommand::Arch(_) => "rezup package install arch",
                    PackageInstallCommand::Os(_) => "rezup package install os",
                    PackageInstallCommand::Platform(_) => "rezup package install platform",
                    PackageInstallCommand::Python(_) => "rezup package install python",
                },
                PackageCommand::List(_) => "rezup package list",
            },
            Command::Self_(self_command) => match self_command.command {
                SelfCommand::Update => "rezup self update",
            },
        }
    }
}

fn main() -> ExitCode {
    let action = Cli::parse().action();
    eprintln!("error: {action} is not implemented");
    ExitCode::FAILURE
}
