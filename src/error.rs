use self::super::util::uppercase_first;
use std::borrow::Cow;
use std::fmt;


/// An application failure.
///
/// # Examples
///
/// ```
/// # use https::Error;
/// assert_eq!(Error {
///                desc: "network",
///                op: "write",
///                more: "full buffer".into(),
///            }.to_string(),
///            "Writing network failed: full buffer.");
/// ```
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Error {
    /// The file the I/O operation regards.
    pub desc: &'static str,
    /// The failed operation.
    ///
    /// This should be lowercase and imperative ("create", "open").
    pub op: &'static str,
    /// Additional data.
    pub more: Cow<'static, str>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Strip the last 'e', if any, so we get correct inflection for continuous times
        let op = uppercase_first(if self.op.ends_with('e') {
            &self.op[..self.op.len() - 1]
        } else {
            self.op
        });

        write!(f, "{}ing {} failed: {}.", op, self.desc, self.more)
    }
}
