#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! D9: an `ExportPermit` is minted only by `app::export::confirm`, so no code outside the module
//! can write into the osu! folder without a confirmed preview.

#[test]
fn export_permit_cannot_be_built_outside_export() {
    trybuild::TestCases::new().compile_fail("tests/ui/export_permit_*.rs");
}
