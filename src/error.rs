use self::super::util::uppercase_first;
use std::io::Write;


/// Enum representing all possible ways the application can fail.
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Error {
    /// An I/O error occured.
    ///
    /// This includes higher-level I/O errors like FS ones.
    Io {
        /// The file the I/O operation regards.
        desc: &'static str,
        /// The failed operation.
        ///
        /// This should be lowercase and imperative ("create", "open").
        op: &'static str,
        /// Additional data.
        more: Option<&'static str>,
    },
}

impl Error {
    /// Write the error message to the specified output stream.
    ///
    /// # Examples
    ///
    /// ```
    /// # use https::Error;
    /// # use std::iter::FromIterator;
    /// let mut out = Vec::new();
    /// Error::Io {
    ///     desc: "network",
    ///     op: "write",
    ///     more: Some("full buffer"),
    /// }.print_error(&mut out);
    /// assert_eq!(String::from_iter(out.iter().map(|&i| i as char)),
    ///            "Writing network failed: full buffer.\n".to_string());
    /// ```
    pub fn print_error<W: Write>(&self, err_out: &mut W) {
        match *self {
            Error::Io { desc, op, more } => {
                // Strip the last 'e', if any, so we get correct inflection for continuous times
                let op = uppercase_first(if op.ends_with('e') {
                    &op[..op.len() - 1]
                } else {
                    op
                });
                write!(err_out, "{}ing {} failed", op, desc).unwrap();
                if let Some(more) = more {
                    write!(err_out, ": {}", more).unwrap();
                }
                writeln!(err_out, ".").unwrap();
            }
        }
    }

    /// Get the executable exit value from an `Error` instance.
    ///
    /// # Examples
    ///
    /// ```
    /// # use https::Error;
    /// assert_eq!(Error::Io {
    ///     desc: "",
    ///     op: "",
    ///     more: None,
    /// }.exit_value(), 1);
    /// ```
    pub fn exit_value(&self) -> i32 {
        match *self {
            Error::Io { .. } => 1,
        }
    }
}
