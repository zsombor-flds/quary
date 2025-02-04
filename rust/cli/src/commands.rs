use clap::{Args, Parser, Subcommand, ValueEnum};
use quary_proto::TestRunner;

#[derive(Debug, Parser)]
#[command(name = "quary")]
#[command(about = "Quary is a very fast and useful database tool", long_about = None, version=env!("CARGO_PKG_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
    /// Project file location
    #[arg(long, short = 'p', global = true, default_value = "quary.yaml")]
    pub(crate) project_file: String,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    #[command(
        name = "init",
        about = "Initialize a new project in the current directory"
    )]
    Init(InitArgs),
    #[command(name = "compile", about = "Compile the project")]
    Compile,
    #[command(name = "build", about = "Build the seed and model views")]
    Build(BuildArgs),
    #[command(name = "test", about = "Run the tests")]
    Test(TestArgs),
    #[command(
        name = "convert-dbt-project",
        about = "Convert a dbt core project to a quary project and place in the specified path"
    )]
    ConvertDbt(ConvertDbtArgs),
}

#[derive(Args, Debug)]
pub struct ConvertDbtArgs {
    /// Output path for the converted project
    pub quary_project_path: String,
}

#[derive(Args, Debug)]
pub struct InitArgs {
    #[arg(value_enum, long = "type", short = 't', default_value_t = InitType::Sqlite)]
    /// Select which sample initialisation to show
    pub mode: InitType,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum InitType {
    Sqlite,
    Duckdb,
}

#[derive(Args, Debug)]
pub struct GenerateSourcesArgs {
    /// Optional file path where to write the sources to, if not provided, will write to stdout.
    pub file_path: Option<String>,
}

#[derive(Args, Debug)]
pub struct RenderSqlArgs {
    /// Model to render sql statement for
    pub(crate) model: String,
}

#[derive(Args, Debug)]
pub struct BuildArgs {
    #[arg(long = "dry-run", short = 'd', default_value = "false")]
    /// Print rendered sql statements to the terminal without running them against the database
    pub dry_run: bool,
    #[arg(long = "cache_views", short = 'c', default_value = "false")]
    /// Build the cache views for the extension
    pub cache_views: bool,
}

#[derive(Args, Debug)]
pub struct TestArgs {
    #[arg(long = "dry-run", short = 'd', default_value = "false")]
    /// Print rendered sql tests to the terminal without running them against the database
    pub dry_run: bool,
    #[arg(value_enum, long = "mode", short = 'm', default_value_t = TestMode::All)]
    /// Select which test runner mode to use
    pub mode: TestMode,
    #[arg(long = "reference-source", short = 's', default_value = "false")]
    /// Rather than running the tests against the views,s run them against the source tables by building the model in CTEs
    pub full_source: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum TestMode {
    All,
    Skip,
}

pub fn mode_to_test_runner(mode: &TestMode) -> TestRunner {
    match mode {
        TestMode::All => TestRunner::All,
        TestMode::Skip => TestRunner::Skip,
    }
}
