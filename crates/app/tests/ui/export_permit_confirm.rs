use wolluf_app::export::{ExportParams, PreviewRegistry, confirm};

fn main() {
    let registry: PreviewRegistry<()> = PreviewRegistry::new(ExportParams::default());
    let _ = confirm(&registry, "id", wolluf_core::UnixUs(0));
}
