use wolluf_app::export::{ExportPermit, SetFolder, WriteOutcome, write_new};

fn forge_and_write(folder: SetFolder) -> WriteOutcome {
    let permit = ExportPermit {
        preview_id: String::new(),
        folder,
        files: vec!["x.osu".to_owned()],
    };
    write_new(&permit, "x.osu", b"").unwrap()
}

fn main() {
    let _ = forge_and_write;
}
