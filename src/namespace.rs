use serde::Serialize;
use std::fs;
use std::io;
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum NamespaceError {
    PermissionDenied,
    InvalidFormat,
    InvalidId,
    Io,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProcessNamespaces {
    pub pid: Result<u64, NamespaceError>,
    pub net: Result<u64, NamespaceError>,
    pub mnt: Result<u64, NamespaceError>,
    pub user: Result<u64, NamespaceError>,
}

pub fn parse_namespace_target(target: &str) -> Result<u64, NamespaceError> {
    let start = target.find('[').ok_or(NamespaceError::InvalidFormat)? + 1;
    let end = target.find(']').ok_or(NamespaceError::InvalidFormat)?;

    if start > end {
        return Err(NamespaceError::InvalidFormat);
    }

    target[start..end]
        .parse::<u64>()
        .map_err(|_| NamespaceError::InvalidId)
}

pub fn read_namespace(pid: u32, namespace: &str) -> Result<u64, NamespaceError> {
    let path = format!("/proc/{}/ns/{}", pid, namespace);

    let target = fs::read_link(path).map_err(|e| {
        if e.kind() == io::ErrorKind::PermissionDenied {
            NamespaceError::PermissionDenied
        } else {
            NamespaceError::Io
        }
    })?;

    parse_namespace_target(&target.to_string_lossy())
}

pub fn read_namespaces(pid: u32) -> ProcessNamespaces {
    ProcessNamespaces {
        pid: read_namespace(pid, "pid"),
        net: read_namespace(pid, "net"),
        mnt: read_namespace(pid, "mnt"),
        user: read_namespace(pid, "user"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_namespace_targets() {
        assert_eq!(parse_namespace_target("pid:[4026531836]"), Ok(4026531836));
        assert_eq!(parse_namespace_target("net:[4026531992]"), Ok(4026531992));
        assert_eq!(parse_namespace_target("mnt:[4026531840]"), Ok(4026531840));
        assert_eq!(parse_namespace_target("user:[4026531837]"), Ok(4026531837));
    }

    #[test]
    fn fails_on_malformed_targets() {
        assert_eq!(
            parse_namespace_target("pid:4026531836]"),
            Err(NamespaceError::InvalidFormat)
        );
        assert_eq!(
            parse_namespace_target("pid:[4026531836"),
            Err(NamespaceError::InvalidFormat)
        );
        assert_eq!(
            parse_namespace_target("pid:no_brackets"),
            Err(NamespaceError::InvalidFormat)
        );
        assert_eq!(
            parse_namespace_target("pid:]reversed["),
            Err(NamespaceError::InvalidFormat)
        );
    }

    #[test]
    fn fails_on_invalid_numeric_ids() {
        assert_eq!(
            parse_namespace_target("pid:[not_a_num]"),
            Err(NamespaceError::InvalidId)
        );
        assert_eq!(
            parse_namespace_target("pid:[]"),
            Err(NamespaceError::InvalidId)
        );
    }
}
