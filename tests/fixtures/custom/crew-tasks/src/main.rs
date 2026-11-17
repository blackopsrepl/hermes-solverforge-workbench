/* crew-tasks — SolverForge command-line shell */

use clap::{Parser, Subcommand};
use crew_tasks::api::PlanDto;
use crew_tasks::data::{default_demo_data, generate, DemoData};

#[derive(Parser)]
#[command(name = "crew-tasks")]
#[command(about = "SolverForge command-line optimizer")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Print generated demo data as JSON
    DemoData {
        /// Demo data size: small, standard, or large
        #[arg(long)]
        size: Option<String>,
    },
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    solverforge::console::init();
    let cli = Cli::parse();

    match cli.command.unwrap_or(Command::DemoData { size: None }) {
        Command::DemoData { size } => {
            let demo = match size {
                Some(size) => size.parse::<DemoData>().map_err(|_| {
                    std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        format!("unknown demo data size '{size}'"),
                    )
                })?,
                None => default_demo_data(),
            };
            let dto = PlanDto::from_plan(&generate(demo));
            println!("{}", serde_json::to_string_pretty(&dto)?);
        }
    }

    Ok(())
}
