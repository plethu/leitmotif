use std::{error::Error, io};

use clap::{Parser, ValueEnum};
use leitmotif_recite::{Bindings, ErrorKind};
use recite_core::{SourcePosition, SourceSpan, compiled::CompiledConditionExpression};

use super::{
    analyse_all,
    export::{Export, Location},
    model::{ModelVariant, Scenario},
    protocol::Limits,
};

#[derive(Clone, Copy, ValueEnum)]
enum Variant {
    Corrected,
    Faulty,
    Unsupported,
}
#[derive(Parser)]
#[command(about = "Explore the finite archive host protocol", term_width = 80)]
struct Args {
    /// Emit an experimental CI display projection; the consumer decides its status.
    #[arg(long)]
    json: bool,
    #[arg(long, value_enum, default_value = "corrected", requires = "json")]
    variant: Variant,
    #[arg(long, default_value_t = 1000, requires = "json")]
    max_nodes: usize,
    #[arg(long, default_value_t = 32, requires = "json")]
    max_depth: usize,
}
pub(super) fn run() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    if !args.json {
        return super::run();
    }
    let mut stdout = io::stdout().lock();
    if matches!(args.variant, Variant::Unsupported) {
        let error = unsupported_guard()?;
        let view = Export::unsupported(error.to_string(), Location::repository(&error.provenance)?);
        serde_json::to_writer_pretty(&mut stdout, &view)?;
    } else {
        let variant = match args.variant {
            Variant::Faulty => ModelVariant::Faulty,
            Variant::Corrected | Variant::Unsupported => ModelVariant::Corrected,
        };
        let scenario = Scenario::new(variant)?;
        let reports = analyse_all(
            &scenario,
            Limits {
                max_nodes: args.max_nodes,
                max_depth: args.max_depth,
            },
        )?;
        serde_json::to_writer_pretty(&mut stdout, &Export::analysed(&reports)?)?;
    }
    Ok(())
}
fn unsupported_guard() -> Result<leitmotif_recite::Error, Box<dyn Error>> {
    // A compiled artifact can contain this unsupported shape. Preserve its authored source.
    let span = SourceSpan::point(file!(), SourcePosition::new(line!(), 1)?);
    match Bindings::new().guard(&CompiledConditionExpression::And(vec![]), &span) {
        Err(error) if matches!(error.kind, ErrorKind::Unsupported(_)) => Ok(error),
        Err(error) => Err(error.into()),
        Ok(_) => Err("empty compiled Boolean group unexpectedly accepted".into()),
    }
}
