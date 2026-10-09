//! Chart difficulty calculators (architecture §3).

pub mod minacalc;

/// The engine may not depend on `wolluf-minacalc` (ADR 0022: difficulty is its only consumer),
/// so the calculator handle and its constants reach the `difficulty` stage through here.
pub use wolluf_minacalc::{CALC_VERSION, Calc, CalcError, NoteRow, SKILLSET_IDS};
