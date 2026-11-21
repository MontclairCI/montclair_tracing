use time::OffsetDateTime;
use time::macros::format_description;
use tracing::Level;

/// Configuration for the tracing subscriber layer that writes to stderr.
/// Writes are synchronous and blocking. Only recommended for development and testing.
#[derive(Debug, Clone)]
pub enum StderrConfig {
    Disabled,
    Enabled {
        /// The maximum level to log. Overrides `RUST_LOG` environment variable for this layer.
        level: Level,
    },
}

use tracing::field::Field;
use tracing::field::Visit;

struct FieldCollector {
    fields: Vec<(String, String)>,
}

impl Visit for FieldCollector {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        self.fields
            .push((field.name().to_string(), format!("{:?}", value)));
    }
}

use std::io::IsTerminal;
use tracing::Subscriber;
use tracing_subscriber::fmt::FmtContext;
use tracing_subscriber::fmt::FormatEvent;
use tracing_subscriber::fmt::FormatFields;
use tracing_subscriber::fmt::format::Writer;

pub struct StderrLogFormatter {
    should_use_color: bool,
}

impl StderrLogFormatter {
    pub fn new() -> Self {
        Self {
            should_use_color: std::io::stderr().is_terminal(),
        }
    }
}

impl<S, N> FormatEvent<S, N> for StderrLogFormatter
where
    S: Subscriber + for<'a> tracing_subscriber::registry::LookupSpan<'a>,
    N: for<'a> FormatFields<'a> + 'static,
{
    fn format_event(
        &self,
        _ctx: &FmtContext<'_, S, N>,
        mut writer: Writer<'_>,
        event: &tracing::Event<'_>,
    ) -> std::fmt::Result {
        // Metadata
        let meta = event.metadata();

        let (c_reset, c_dim, c_grey, c_level, c_target) = if self.should_use_color {
            (
                "\x1b[0m",  // Reset
                "\x1b[2m",  // Dim
                "\x1b[90m", // Grey
                match *meta.level() {
                    tracing::Level::ERROR => "\x1b[1;31m", // Bold Red
                    tracing::Level::WARN => "\x1b[1;33m",  // Bold Yellow
                    tracing::Level::INFO => "\x1b[32m",    // Green
                    tracing::Level::DEBUG => "\x1b[36m",   // Cyan
                    tracing::Level::TRACE => "\x1b[35m",   // Magenta
                },
                "\x1b[36m", // Cyan for target
            )
        } else {
            ("", "", "", "", "")
        };

        let ts_format = format_description!(
            "[year]-[month]-[day] [hour]:[minute]:[second].[subsecond digits:6]"
        );

        let now_ts = OffsetDateTime::now_local().unwrap();
        let now_formatted = now_ts.format(&ts_format).unwrap();
        write!(writer, "{c_grey}{}{c_reset}", now_formatted)?;

        write!(writer, " {c_level}{:<5}{c_reset} ", meta.level())?;

        let file = meta.file().unwrap_or("<unknown_file>");
        let line = meta.line().unwrap_or(0);

        write!(writer, "{c_target}{}:{}{c_reset}", file, line)?;

        writeln!(writer)?;

        // Message
        let mut visitor = FieldCollector { fields: Vec::new() };
        event.record(&mut visitor);

        if let Some(pos) = visitor.fields.iter().position(|(k, _)| k == "message") {
            let (_, msg) = visitor.fields.remove(pos);
            write!(writer, "{}", msg)?;
        } else {
            write!(writer, "{c_dim}Span: {}{c_reset}", meta.name())?;
        }

        writeln!(writer)?;

        Ok(())
    }
}
