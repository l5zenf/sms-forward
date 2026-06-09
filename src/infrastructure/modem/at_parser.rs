/// Parsed AT response or unsolicited result code (URC)
#[derive(Debug, Clone, PartialEq)]
pub enum AtLine {
    Ok,
    Error,
    CmeError(i32, String),
    Data(String),
    Cmti { mem: String, index: i32 },
    Ring,
    Clip(String),
    Cpin(String),
    Creg(i32, Option<i32>),
    Csq(i32, i32),
    Cops(String),
    Other(String),
    Empty,
}

/// Parse a single line from the modem.
/// Lines can be:
/// - AT command echo (stripped)
/// - AT response: OK, ERROR, +CME ERROR: ...
/// - URC: +CMTI, RING, +CLIP, etc.
/// - Data payload
pub fn parse_line(line: &str) -> AtLine {
    let line = line.trim();

    if line.is_empty() {
        return AtLine::Empty;
    }

    match line {
        "OK" => AtLine::Ok,
        "ERROR" => AtLine::Error,
        s if s.starts_with("+CME ERROR:") => parse_cme_error(s),
        s if s.starts_with("+CMTI:") => parse_cmti(s),
        "RING" => AtLine::Ring,
        s if s.starts_with("+CLIP:") => parse_clip(s),
        s if s.starts_with("+CPIN:") => parse_cpin(s),
        s if s.starts_with("+CREG:") => parse_creg(s),
        s if s.starts_with("+CSQ:") => parse_csq(s),
        s if s.starts_with("+COPS:") => parse_cops(s),
        s if s.starts_with('>') => AtLine::Other(">".into()),
        s => {
            // If we're in command mode, this could be a data response
            // or a raw line we don't recognize as a URC
            AtLine::Data(s.to_string())
        }
    }
}

fn parse_cme_error(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CME ERROR:").unwrap_or(s).trim();
    // Try to parse numeric error code
    if let Some(code) = rest.split(':').next().and_then(|c| c.trim().parse::<i32>().ok()) {
        let msg = rest.split(':').nth(1).map(|m| m.trim().to_string()).unwrap_or_default();
        AtLine::CmeError(code, msg)
    } else {
        AtLine::CmeError(0, rest.to_string())
    }
}

fn parse_cmti(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CMTI:").unwrap_or(s).trim();
    // Format: "SM",3
    let parts: Vec<&str> = rest.split(',').collect();
    if parts.len() >= 2 {
        let mem = parts[0].trim().trim_matches('"').to_string();
        let index = parts[1].trim().parse::<i32>().unwrap_or(0);
        AtLine::Cmti { mem, index }
    } else {
        AtLine::Other(s.to_string())
    }
}

fn parse_clip(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CLIP:").unwrap_or(s).trim();
    AtLine::Clip(rest.to_string())
}

fn parse_cpin(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CPIN:").unwrap_or(s).trim();
    AtLine::Cpin(rest.to_string())
}

fn parse_creg(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CREG:").unwrap_or(s).trim();
    let parts: Vec<&str> = rest.split(',').collect();
    let n = parts.first().and_then(|p| p.trim().parse::<i32>().ok()).unwrap_or(0);
    let stat = parts.get(1).and_then(|p| p.trim().parse::<i32>().ok());
    AtLine::Creg(n, stat)
}

fn parse_csq(s: &str) -> AtLine {
    let rest = s.strip_prefix("+CSQ:").unwrap_or(s).trim();
    let parts: Vec<&str> = rest.split(',').collect();
    let rssi = parts.first().and_then(|p| p.trim().parse::<i32>().ok()).unwrap_or(99);
    let ber = parts.get(1).and_then(|p| p.trim().parse::<i32>().ok()).unwrap_or(99);
    AtLine::Csq(rssi, ber)
}

fn parse_cops(s: &str) -> AtLine {
    let rest = s.strip_prefix("+COPS:").unwrap_or(s).trim();
    AtLine::Cops(rest.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_ok() {
        assert_eq!(parse_line("OK"), AtLine::Ok);
        assert_eq!(parse_line("  OK\r\n"), AtLine::Ok);
    }

    #[test]
    fn test_parse_error() {
        assert_eq!(parse_line("ERROR"), AtLine::Error);
    }

    #[test]
    fn test_parse_cmti() {
        assert_eq!(
            parse_line("+CMTI: \"SM\",3"),
            AtLine::Cmti {
                mem: "SM".into(),
                index: 3
            }
        );
    }

    #[test]
    fn test_parse_ring() {
        assert_eq!(parse_line("RING"), AtLine::Ring);
    }

    #[test]
    fn test_parse_csq() {
        assert_eq!(parse_line("+CSQ: 22,99"), AtLine::Csq(22, 99));
    }

    #[test]
    fn test_parse_cpin_ready() {
        assert_eq!(parse_line("+CPIN: READY"), AtLine::Cpin("READY".into()));
    }

    #[test]
    fn test_parse_empty() {
        assert_eq!(parse_line(""), AtLine::Empty);
        assert_eq!(parse_line("  "), AtLine::Empty);
    }

    #[test]
    fn test_parse_cme_error() {
        assert_eq!(
            parse_line("+CME ERROR: 10"),
            AtLine::CmeError(10, "".into())
        );
    }

    #[test]
    fn test_parse_creg() {
        assert_eq!(parse_line("+CREG: 0,1"), AtLine::Creg(0, Some(1)));
    }
}
