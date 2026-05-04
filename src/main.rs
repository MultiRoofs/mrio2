mod io;
mod model;
mod ops;
mod stats;
mod tui;

use clap::Parser;
use model::OutputFormat;

#[derive(Parser)]
#[command(name = "mrio2", about = "CityJSON editor — view and modify CityJSON files")]
struct Cli {
    /// Input file (.city.json or .city.jsonl)
    input: String,

    /// Output file (omit to prompt in TUI)
    #[arg(short, long)]
    output: Option<String>,

    /// Output format: cityjson, cityjsonseq, or auto (default: auto)
    #[arg(long)]
    output_format: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let doc = match io::read_file(&cli.input) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    };

    let output_format = match cli.output_format.as_deref() {
        Some("cityjson") => OutputFormat::CityJSON,
        Some("cityjsonseq") => OutputFormat::CityJSONSeq,
        Some("auto") | None => doc.original_format.into(),
        Some(other) => {
            eprintln!(
                "Unknown output format: '{}'. Use 'cityjson' or 'cityjsonseq'.",
                other
            );
            std::process::exit(1);
        }
    };

    if let Err(e) = tui::run(doc, &cli.input, output_format, cli.output) {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
