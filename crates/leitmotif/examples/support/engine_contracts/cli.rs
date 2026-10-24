use std::{
    error::Error as StdError,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    process::ExitCode,
};

use clap::{Parser, Subcommand, ValueEnum};

use super::{
    contract::Contract,
    error::Error,
    host::Host,
    input::{Limits, Query},
    output::Output,
};

#[derive(Clone, Copy, ValueEnum)]
enum EndingQuery {
    Sheltered,
    Departed,
    StormPassed,
}
impl EndingQuery {
    const fn query(self) -> Query {
        match self {
            Self::Sheltered => Query::Sheltered,
            Self::Departed => Query::Departed,
            Self::StormPassed => Query::StormPassed,
        }
    }
}
#[derive(Parser)]
#[command(
    about = "Compare a game-owned harbour host with offline finite analysis",
    term_width = 80
)]
struct Args {
    /// Source/build revision attested by this consumer, distinct from semantic model identity.
    #[arg(long, value_name = "REVISION")]
    source_revision: String,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    /// Emit a complete six-fact host projection for offline checking.
    Request {
        #[arg(long, value_enum, default_value = "sheltered")]
        query: EndingQuery,
        #[arg(long, default_value_t = Limits::COMPLETE.max_states)]
        max_states: usize,
        #[arg(long, default_value_t = Limits::COMPLETE.max_depth)]
        max_depth: usize,
    },
    /// Validate a projection, audit every action, and replay against independent Rust state.
    Check {
        /// Complete projected host state request; use - to read JSON from stdin.
        input: PathBuf,
        /// Demonstrate a real host effect bug; input data never selects host behavior.
        #[arg(long)]
        lighting_fault: bool,
    },
}
pub(super) fn run() -> Result<ExitCode, Box<dyn StdError>> {
    let args = Args::parse();
    let contract = match Contract::new(args.source_revision) {
        Ok(contract) => contract,
        Err(error) => return write(&Output::error(error)),
    };
    match args.command {
        Command::Request {
            query,
            max_states,
            max_depth,
        } => {
            let request = contract.request(
                Host::initial().snapshot().clone(),
                query.query(),
                Limits {
                    max_states,
                    max_depth,
                },
            );
            serde_json::to_writer_pretty(&mut io::stdout().lock(), &request)?;
            Ok(ExitCode::SUCCESS)
        }
        Command::Check {
            input,
            lighting_fault,
        } => {
            let result = read(&input)
                .map_err(|error| Error::RejectedInput(error.to_string()))
                .and_then(|text| check(&contract, &text, lighting_fault));
            let output = match result {
                Ok(output) => output,
                Err(error) => Output::error(error),
            };
            write(&output)
        }
    }
}
fn write(output: &Output) -> Result<ExitCode, Box<dyn StdError>> {
    serde_json::to_writer_pretty(&mut io::stdout().lock(), &output)?;
    Ok(ExitCode::from(output.status()))
}
fn check(contract: &Contract, text: &str, lighting_fault: bool) -> Result<Output, Error> {
    let request = contract.decode(text)?;
    let mut host = if lighting_fault {
        Host::with_lighting_fault(request.initial.clone())
    } else {
        Host::new(request.initial.clone())
    };
    let audit = contract.audit(&request, &host)?;
    let analysis = contract.analyse(&request)?;
    let replay = contract.replay(&request, &analysis, &mut host)?;
    Ok(Output::checked(analysis, audit, replay))
}

fn read(path: &Path) -> io::Result<String> {
    if path.as_os_str() == "-" {
        let mut text = String::new();
        io::stdin().read_to_string(&mut text)?;
        Ok(text)
    } else {
        fs::read_to_string(path)
    }
}
