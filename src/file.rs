use std::path::PathBuf;
use time::OffsetDateTime;
use time::macros::format_description;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tracing::Level;

use crate::TError;

/// Configuration for the tracing subscriber layer that writes to a local file.
/// Writes are asynchronous and non-blocking.
#[derive(Debug, Clone)]
pub enum FileConfig {
    Disabled,
    Enabled {
        /// The maximum level to log. Overrides `RUST_LOG` environment variable for this layer.
        level: Level,
        /// The directory that the log file will be created in.
        dir_path: PathBuf,
        /// The suffix of the log file. A UNIX epoch will be appended at file creation time.
        file_name: String,
    },
}

/// A layer writer that writes tracing events to a local file.
///
/// Events are sent asynchronously and requires a [`tokio::runtime::Handle`] to spawn the actor task.
/// If the actor's channel gets full, then events will be blocked.
#[derive(Clone)]
pub struct FileWriter {
    tx: mpsc::Sender<Vec<u8>>,
}

impl FileWriter {
    pub fn spawn(dir_path: PathBuf, file_name: String) -> Self {
        let (tx, mut rx) = mpsc::channel::<Vec<u8>>(1024);

        tokio::spawn(async move {
            std::fs::create_dir_all(&dir_path)?;

            let file_path = {
                let ts_format =
                    format_description!("[year]-[month]-[day]_[hour]-[minute]-[second]");
                let prefix = OffsetDateTime::now_local()?.format(&ts_format)?;
                dir_path.join(format!("{prefix}_{file_name}.log"))
            };

            let mut file = match tokio::fs::File::create(&file_path).await {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("Failed to create log file at {file_path:?}: {e}");
                    eprintln!("FileWriter will be disabled.");
                    return Err(e.into());
                }
            };

            while let Some(bytes) = rx.recv().await {
                if let Err(e) = file.write_all(&bytes).await {
                    eprintln!("Failed to write log to file: {e}");
                }
            }

            file.sync_all().await?;

            Ok::<(), TError>(())
        });

        Self { tx }
    }
}

impl std::io::Write for FileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let bytes = buf.to_vec();
        match self.tx.blocking_send(bytes) {
            Ok(()) => Ok(buf.len()),
            Err(_) => Err(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "FileWriter's mpsc channel receiver has dropped",
            )),
        }
    }

    /// This is a no-op. The mpsc channel handle delivery asynchronously.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::writer::MakeWriter<'a> for FileWriter {
    type Writer = Self;

    fn make_writer(&self) -> Self::Writer {
        self.clone()
    }
}
