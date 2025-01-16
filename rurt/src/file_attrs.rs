use std::path::Path;

#[cfg(target_family = "unix")]
pub fn hidden(p: impl AsRef<Path>) -> bool {
    use std::os::unix::ffi::OsStrExt;
    let bytes = p.as_ref().as_os_str().as_bytes();
    !bytes.is_empty() && bytes[0] == b'.'
}

#[cfg(target_family = "windows")]
pub fn hidden(p: impl AsRef<Path>) -> bool {
    use std::os::windows::fs::MetadataExt;
    match p.as_ref().metadata() {
        Ok(meta) => meta.file_attributes() & 2 != 0,
        Err(_) => false,
    }
}
