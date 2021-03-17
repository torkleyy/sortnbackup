use std::path::PathBuf;

pub fn fix_cross_path(path: &str) -> PathBuf {
    let mut buf = [0; 4];
    path.replace("/", std::path::MAIN_SEPARATOR.encode_utf8(&mut buf))
        .into()
}
