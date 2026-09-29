//! `cargo xtask bindings` runs this to regenerate `apps/desktop/ui/src/ipc/bindings.ts`.

use std::process::ExitCode;

use wolluf_desktop::bindings::{committed_path, export_bindings};

fn main() -> ExitCode {
    let path = committed_path();
    match export_bindings(&path) {
        Ok(()) => {
            println!("bindings: wrote {}", path.display());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!(
                "error[INTERNAL]: bindings export to {}: {e}",
                path.display()
            );
            ExitCode::FAILURE
        }
    }
}
