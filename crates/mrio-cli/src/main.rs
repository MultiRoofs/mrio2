use clap::Parser;
use mrio_core::model::OutputFormat;

#[derive(Parser)]
#[command(
    name = "mrio",
    about = "Editor to prepare CityJSON files for the MultiRoofs project"
)]
struct Cli {
    input: String,

    #[arg(short, long)]
    output: Option<String>,

    #[arg(long)]
    output_format: Option<String>,
}

fn main() {
    let cli = Cli::parse();

    let doc = match mrio_core::io::read_file(&cli.input) {
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

mod tui;
