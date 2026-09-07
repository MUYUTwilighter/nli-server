use std::process::ExitCode;

fn main() -> ExitCode {
    eprintln!(
        "nli-server {} is a Phase 0 bootstrap and exposes no API",
        nli_server::VERSION
    );
    ExitCode::FAILURE
}
