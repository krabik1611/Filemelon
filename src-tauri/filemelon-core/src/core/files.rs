use sha2::{Digest, Sha256};
use std::{fs::{self, File, OpenOptions}, io::{self, Read, Seek, SeekFrom, Write}, path::{Path, PathBuf}, sync::atomic::{AtomicBool, Ordering}, time::{Duration, SystemTime}};
#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

#[derive(Debug)]
pub enum MoveError { Cancelled, Failed(String) }

#[cfg(windows)]
pub fn recycle_file(path: &std::path::Path) -> Result<(), MoveError> {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::UI::Shell::{SHFileOperationW, SHFILEOPSTRUCTW, FO_DELETE, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT};
    let mut from: Vec<u16> = path.as_os_str().encode_wide().collect();
    from.push(0); from.push(0);
    let mut op = SHFILEOPSTRUCTW { hwnd: std::ptr::null_mut(), wFunc: FO_DELETE, pFrom: from.as_ptr(), pTo: std::ptr::null(), fFlags: (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16, fAnyOperationsAborted: 0, hNameMappings: std::ptr::null_mut(), lpszProgressTitle: std::ptr::null() };
    let code = unsafe { SHFileOperationW(&mut op) };
    if code == 0 && op.fAnyOperationsAborted == 0 { Ok(()) } else { Err(MoveError::Failed(format!("Recycle Bin operation failed ({code})"))) }
}

#[cfg(not(windows))]
pub fn recycle_file(_path: &std::path::Path) -> Result<(), MoveError> { Err(MoveError::Failed("Deleting to the Recycle Bin is supported on Windows only".into())) }
impl From<io::Error> for MoveError {
    fn from(error: io::Error) -> Self {
        if error.kind() == io::ErrorKind::Interrupted { Self::Cancelled } else { Self::Failed(error.to_string()) }
    }
}
impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Cancelled => write!(f, "Cancelled"), Self::Failed(message) => write!(f, "{message}") }
    }
}
fn check_cancel(cancel: &AtomicBool) -> io::Result<()> {
    if cancel.load(Ordering::Acquire) { Err(io::Error::new(io::ErrorKind::Interrupted, "Cancelled")) } else { Ok(()) }
}
fn locked_read(path: &Path, delete_access: bool) -> io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)] {
        options.share_mode(0);
        if delete_access { options.access_mode(windows_sys::Win32::Foundation::GENERIC_READ | windows_sys::Win32::Storage::FileSystem::DELETE); }
    }
    #[cfg(not(windows))] let _ = delete_access;
    options.open(path)
}
fn delete_source(file: &File, path: &Path) -> io::Result<()> {
    #[cfg(windows)] {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{SetFileInformationByHandle, FileDispositionInfo, FILE_DISPOSITION_INFO};
        let info = FILE_DISPOSITION_INFO { DeleteFile: true };
        // SAFETY: the File owns a live DELETE-access handle and info has the required ABI.
        let result = unsafe { SetFileInformationByHandle(file.as_raw_handle(), FileDispositionInfo, std::ptr::from_ref(&info).cast(), std::mem::size_of::<FILE_DISPOSITION_INFO>() as u32) };
        let _ = path;
        if result == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }
    #[cfg(not(windows))] { let _ = file; fs::remove_file(path) }
}
// No replacement: a raced destination is never overwritten. Windows renames the locked file itself.
fn rename_locked(file: &File, source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(windows)] {
        use std::os::windows::{ffi::OsStrExt, io::AsRawHandle};
        use windows_sys::Win32::Storage::FileSystem::{SetFileInformationByHandle, FileRenameInfo, FILE_RENAME_INFO};
        let name: Vec<u16> = destination.as_os_str().encode_wide().collect();
        let offset = std::mem::offset_of!(FILE_RENAME_INFO, FileName);
        let bytes = (offset + (name.len() + 1) * 2).max(std::mem::size_of::<FILE_RENAME_INFO>());
        let mut storage = vec![0usize; bytes.div_ceil(std::mem::size_of::<usize>())];
        // SAFETY: usize storage provides native alignment, sufficient initialized backing bytes,
        // and remains live through the synchronous call. The flexible UTF-16 tail fits the allocation.
        let result = unsafe {
            let info = storage.as_mut_ptr().cast::<FILE_RENAME_INFO>();
            (*info).FileNameLength = (name.len() * 2) as u32;
            std::ptr::copy_nonoverlapping(name.as_ptr(), storage.as_mut_ptr().cast::<u8>().add(offset).cast::<u16>(), name.len());
            SetFileInformationByHandle(file.as_raw_handle(), FileRenameInfo, info.cast(), bytes as u32)
        };
        let _ = source;
        if result == 0 { Err(io::Error::last_os_error()) } else { Ok(()) }
    }
    #[cfg(not(windows))] {
        let _ = file;
        fs::hard_link(source, destination)?;
        if let Err(error) = fs::remove_file(source) { let _ = fs::remove_file(destination); return Err(error); }
        Ok(())
    }
}
fn cross_device(error: &io::Error) -> bool {
    #[cfg(windows)] { error.raw_os_error() == Some(17) }
    #[cfg(not(windows))] { error.kind() == io::ErrorKind::CrossesDevices }
}
fn exists_error(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::AlreadyExists || matches!(error.raw_os_error(), Some(80 | 183))
}
fn digest(file: &mut File, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    file.seek(SeekFrom::Start(0))?;
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        check_cancel(cancel)?;
        let n = file.read(&mut buffer)?;
        if n == 0 { break; }
        hash.update(&buffer[..n]);
    }
    check_cancel(cancel)?;
    Ok(hash.finalize().to_vec())
}
fn copy_hash(input: &mut impl Read, output: &mut impl Write, cancel: &AtomicBool) -> io::Result<Vec<u8>> {
    let mut hash = Sha256::new();
    let mut buffer = vec![0; 1024 * 1024];
    loop {
        check_cancel(cancel)?;
        let n = input.read(&mut buffer)?;
        if n == 0 { break; }
        output.write_all(&buffer[..n])?;
        hash.update(&buffer[..n]);
    }
    check_cancel(cancel)?;
    Ok(hash.finalize().to_vec())
}
// Hash while copying rather than pre-reading the entire source.
fn copy_verified(input: &mut File, output: &mut File, cancel: &AtomicBool) -> io::Result<()> {
    let before = input.metadata()?;
    input.seek(SeekFrom::Start(0))?;
    let copied_hash = copy_hash(input, output, cancel)?;
    check_cancel(cancel)?;
    output.flush()?;
    output.sync_all()?;
    let after = input.metadata()?;
    if before.len() != after.len() || before.modified()? != after.modified()? || digest(output, cancel)? != copied_hash {
        return Err(io::Error::other("File changed or copied content failed verification"));
    }
    // Windows denies writes for the entire operation. Other platforms require an additional check.
    #[cfg(not(windows))]
    if digest(input, cancel)? != copied_hash { return Err(io::Error::other("Source changed during copy")); }
    check_cancel(cancel)
}
pub fn stable(meta: &fs::Metadata, age: u64) -> bool {
    meta.modified().ok().and_then(|t| SystemTime::now().duration_since(t).ok()).is_some_and(|d| d >= Duration::from_secs(age.max(2)))
}
pub fn move_file(source: &Path, directory: &Path, age: u64) -> Result<(PathBuf, &'static str), String> {
    move_file_cancellable(source, directory, age, &AtomicBool::new(false)).map_err(|e| e.to_string())
}
pub fn move_file_cancellable(source: &Path, directory: &Path, age: u64, cancel: &AtomicBool) -> Result<(PathBuf, &'static str), MoveError> {
    check_cancel(cancel)?;
    let mut input = locked_read(source, true)?;
    let before = input.metadata()?;
    if !before.is_file() || !stable(&before, age) { return Err(MoveError::Failed("File is too recent or unstable".into())); }
    fs::create_dir_all(directory)?;
    let source_real = fs::canonicalize(source)?;
    let dir_real = fs::canonicalize(directory)?;
    if source_real.parent() == Some(dir_real.as_path()) { return Ok((source_real, "unchanged")); }
    let filename = source.file_name().ok_or_else(|| MoveError::Failed("Invalid filename".into()))?;
    let stem = source.file_stem().unwrap_or_default().to_string_lossy();
    let extension = source.extension().map(|v| format!(".{}", v.to_string_lossy())).unwrap_or_default();
    let mut source_hash = None;
    let mut must_copy = false;
    for n in 0..100000u32 {
        check_cancel(cancel)?;
        let destination = if n == 0 { dir_real.join(filename) } else { dir_real.join(format!("{stem}_{n}{extension}")) };
        match fs::symlink_metadata(&destination) {
            Ok(meta) => {
                if !meta.is_file() || meta.file_type().is_symlink() || meta.len() != before.len() { continue; }
                let mut existing = locked_read(&destination, false)?;
                if existing.metadata()?.len() != before.len() { continue; }
                if source_hash.is_none() { source_hash = Some(digest(&mut input, cancel)?); }
                if Some(digest(&mut existing, cancel)?) == source_hash {
                    check_cancel(cancel)?;
                    delete_source(&input, source)?;
                    return Ok((destination, "duplicate"));
                }
                continue;
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        if !must_copy {
            check_cancel(cancel)?;
            match rename_locked(&input, source, &destination) {
                Ok(()) => return Ok((destination, "moved")),
                Err(e) if exists_error(&e) => continue,
                Err(e) if cross_device(&e) => must_copy = true,
                Err(e) => return Err(e.into()),
            }
        }
        check_cancel(cancel)?;
        let mut options = OpenOptions::new();
        options.write(true).read(true).create_new(true);
        #[cfg(windows)] options.share_mode(0);
        let mut output = match options.open(&destination) {
            Ok(file) => file,
            Err(e) if exists_error(&e) => continue,
            Err(e) => return Err(e.into()),
        };
        let result = copy_verified(&mut input, &mut output, cancel).and_then(|()| delete_source(&input, source));
        if let Err(error) = result {
            drop(output);
            if let Err(cleanup) = fs::remove_file(&destination) { return Err(MoveError::Failed(format!("{error}; partial destination cleanup failed: {cleanup}"))); }
            return Err(error.into());
        }
        return Ok((destination, "moved"));
    }
    Err(MoveError::Failed("Too many filename conflicts".into()))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn aged_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
        File::options().write(true).open(path).unwrap().set_times(fs::FileTimes::new().set_modified(SystemTime::now() - Duration::from_secs(10))).unwrap();
    }
    #[test] fn conflicts_and_duplicates_preserve_content() {
        let temp = tempfile::tempdir().unwrap(); let source = temp.path().join("a.txt"); let dest = temp.path().join("out"); fs::create_dir(&dest).unwrap();
        fs::write(dest.join("a.txt"), b"old").unwrap(); aged_file(&source, b"new");
        let (path, kind) = move_file(&source, &dest, 0).unwrap(); assert_eq!(kind, "moved"); assert_eq!(path.file_name().unwrap(), "a_1.txt"); assert_eq!(fs::read(dest.join("a.txt")).unwrap(), b"old");
        aged_file(&source, b"new"); let (_, kind) = move_file(&source, &dest, 0).unwrap(); assert_eq!(kind, "duplicate"); assert!(!source.exists()); assert_eq!(fs::read(path).unwrap(), b"new");
    }
    #[test] fn rejects_recent_files_and_same_directory_is_noop() {
        let temp = tempfile::tempdir().unwrap(); let source = temp.path().join("a.txt"); fs::write(&source, b"x").unwrap();
        assert!(move_file(&source, temp.path(), 86400).is_err()); aged_file(&source, b"x");
        assert_eq!(move_file(&source, temp.path(), 0).unwrap().1, "unchanged"); assert!(source.exists());
    }
    #[test] fn native_move_preserves_metadata_and_never_replaces() {
        let temp = tempfile::tempdir().unwrap(); let source = temp.path().join("source"); let dest = temp.path().join("dest");
        aged_file(&source, b"source"); fs::write(&dest, b"existing").unwrap();
        let input = locked_read(&source, true).unwrap();
        assert!(rename_locked(&input, &source, &dest).is_err()); assert_eq!(fs::read(&dest).unwrap(), b"existing");
        let target = temp.path().join("target"); let before = input.metadata().unwrap().modified().unwrap();
        rename_locked(&input, &source, &target).unwrap(); drop(input);
        assert!(!source.exists()); assert_eq!(fs::read(&target).unwrap(), b"source"); assert_eq!(target.metadata().unwrap().modified().unwrap(), before);
    }
    #[test] fn cancelled_move_keeps_source_and_creates_no_destination() {
        let temp = tempfile::tempdir().unwrap(); let source = temp.path().join("source"); let dest = temp.path().join("out"); aged_file(&source,b"source");
        assert!(matches!(move_file_cancellable(&source,&dest,0,&AtomicBool::new(true)),Err(MoveError::Cancelled)));
        assert_eq!(fs::read(source).unwrap(),b"source"); assert!(!dest.exists());
    }
    #[test] fn copy_verifies_content_and_observes_cancellation() {
        let temp = tempfile::tempdir().unwrap(); let source=temp.path().join("source"); let dest=temp.path().join("dest"); fs::write(&source,vec![42u8; 3*1024*1024]).unwrap();
        let mut input=File::open(&source).unwrap(); let mut output=File::options().read(true).write(true).create_new(true).open(&dest).unwrap();
        let error=copy_verified(&mut input,&mut output,&AtomicBool::new(true)).unwrap_err(); assert_eq!(error.kind(),io::ErrorKind::Interrupted); assert_eq!(output.metadata().unwrap().len(),0);
        copy_verified(&mut input,&mut output,&AtomicBool::new(false)).unwrap(); drop(output); assert_eq!(fs::read(source).unwrap(),fs::read(dest).unwrap());
    }
}

#[cfg(test)]
mod mid_copy_tests {
    use super::*;
    #[test]
    fn cancellation_is_observed_between_copy_chunks() {
        struct CancellingWriter<'a> { token: &'a AtomicBool, bytes: usize }
        impl Write for CancellingWriter<'_> {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> { self.bytes += bytes.len(); self.token.store(true, Ordering::Release); Ok(bytes.len()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let token = AtomicBool::new(false);
        let mut input = io::Cursor::new(vec![7u8; 4 * 1024 * 1024]);
        let mut output = CancellingWriter {token:&token,bytes:0};
        assert_eq!(copy_hash(&mut input,&mut output,&token).unwrap_err().kind(),io::ErrorKind::Interrupted);
        assert_eq!(output.bytes,1024 * 1024);
    }
}
