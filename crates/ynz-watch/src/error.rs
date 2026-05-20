use std::path::PathBuf;

#[derive(Debug)]
pub enum WatchError {
    /// File watcher could not subscribe to the given path.
    WatcherInit { path: PathBuf, reason: String },
    /// A file event was delivered but reading the source failed.
    SourceRead { path: PathBuf, reason: String },
    /// No yinz.toml found at the project root in project mode.
    NoProjectFile { root: PathBuf },
    /// The compiled binary could not be spawned as a child process.
    ChildSpawnFailed { binary: PathBuf, reason: String },
    /// A codegen write failed (e.g., tempdir full or permissions).
    CodegenWrite { path: PathBuf, reason: String },
    /// RSS polling returned an unexpected error (distinct from None/unavailable).
    RssError { reason: String },
    /// An I/O error not covered by more specific variants.
    Io(std::io::Error),
}

impl std::fmt::Display for WatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WatchError::WatcherInit { path, reason } => {
                write!(
                    f,
                    "WHAT: ynz watch could not subscribe to `{}`.\n\
                     WHAT INSTEAD: Check that the path exists and is readable.\n\
                     WHY: {reason}",
                    path.display()
                )
            }
            WatchError::SourceRead { path, reason } => {
                write!(
                    f,
                    "WHAT: Could not read `{}`.\n\
                     WHAT INSTEAD: Check that the file exists and is readable.\n\
                     WHY: {reason}",
                    path.display()
                )
            }
            WatchError::NoProjectFile { root } => {
                write!(
                    f,
                    "WHAT: `ynz watch` in project mode requires a `yinz.toml` at `{}`.\n\
                     WHAT INSTEAD: Pass a single `.ynz` file instead (`ynz watch foo.ynz`), \
                     or create a `yinz.toml` project file at the directory root.\n\
                     WHY: Project mode discovers which `.ynz` files to watch by reading \
                     `yinz.toml`. Without it, the project boundary is undefined.",
                    root.display()
                )
            }
            WatchError::ChildSpawnFailed { binary, reason } => {
                write!(
                    f,
                    "WHAT: The compiled binary `{}` could not be executed.\n\
                     WHAT INSTEAD: Check that the binary has execute permissions and \
                     the temp directory is accessible.\n\
                     WHY: {reason}",
                    binary.display()
                )
            }
            WatchError::CodegenWrite { path, reason } => {
                write!(
                    f,
                    "WHAT: The compiler could not write the compiled binary to `{}`.\n\
                     WHAT INSTEAD: Check that the temp directory has free space and \
                     write permissions. Run `df -h $TMPDIR` to check.\n\
                     WHY: {reason}",
                    path.display()
                )
            }
            WatchError::RssError { reason } => {
                write!(
                    f,
                    "WHAT: Memory polling failed.\n\
                     WHAT INSTEAD: Set `YNZ_WATCH_MAX_RSS_MB=0` to disable the memory \
                     hard-stop, or restart `ynz watch` to reset the polling state.\n\
                     WHY: {reason}"
                )
            }
            WatchError::Io(e) => {
                write!(
                    f,
                    "WHAT: An I/O operation failed in the watch daemon.\n\
                     WHAT INSTEAD: Check that the file system is accessible and the process \
                     has the required read/write permissions. The specific error is: {e}\n\
                     WHY: The watch daemon reads source files and writes compiled binaries to \
                     a temp directory. If either operation fails, the rebuild cannot complete."
                )
            }
        }
    }
}

impl From<std::io::Error> for WatchError {
    fn from(e: std::io::Error) -> Self {
        WatchError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, WatchError>;
