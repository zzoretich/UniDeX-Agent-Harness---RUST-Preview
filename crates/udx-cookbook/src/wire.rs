//! Reading line-delimited messages from a peer without letting it make the reader buffer without bound.
//!
//! The browser trial's helper channel (decision 0012) and the rendering trial's automation socket (decision 0013) both
//! speak one message per line and refuse a line over a fixed limit, closing the channel: a peer that sends one is
//! misbehaving. The limit is the only check this layer makes; what a line means is checked by its parser.

use std::io::{self, BufRead};

/// Reads one `\n`-terminated line of at most `max` bytes, without the line break. `Ok(None)` at the end of the
/// stream. A longer line, a last line with no line break, or a line that is not UTF-8 is an error.
pub fn read_line_limited(reader: &mut impl BufRead, max: usize) -> io::Result<Option<String>> {
    let too_long = || io::Error::new(io::ErrorKind::InvalidData, "line too long");
    let mut line = Vec::new();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return if line.is_empty() {
                Ok(None)
            } else {
                Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "line not terminated",
                ))
            };
        }
        if let Some(end) = available.iter().position(|b| *b == b'\n') {
            if line.len() + end > max {
                return Err(too_long());
            }
            line.extend_from_slice(&available[..end]);
            reader.consume(end + 1);
            break;
        }
        let taken = available.len();
        line.extend_from_slice(available);
        reader.consume(taken);
        if line.len() > max {
            return Err(too_long());
        }
    }
    String::from_utf8(line)
        .map(Some)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "line is not UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufReader, Cursor};

    #[test]
    fn reads_lines_and_then_the_end() {
        let mut input = Cursor::new(b"{\"cmd\":\"hello\"}\nsecond\n".to_vec());
        assert_eq!(
            read_line_limited(&mut input, 100).unwrap().as_deref(),
            Some("{\"cmd\":\"hello\"}")
        );
        assert_eq!(
            read_line_limited(&mut input, 100).unwrap().as_deref(),
            Some("second")
        );
        assert_eq!(read_line_limited(&mut input, 100).unwrap(), None);
    }

    #[test]
    fn a_line_exactly_at_the_limit_passes_and_one_byte_more_fails() {
        let mut ok = Cursor::new(b"abcd\n".to_vec());
        assert_eq!(
            read_line_limited(&mut ok, 4).unwrap().as_deref(),
            Some("abcd")
        );
        let mut long = Cursor::new(b"abcde\n".to_vec());
        let error = read_line_limited(&mut long, 4).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidData);
    }

    #[test]
    fn stops_reading_a_long_line_early() {
        // A tiny buffer: the reader must give up after about `max` bytes, not read the whole line first.
        let data = vec![b'a'; 1_000_000];
        let mut input = BufReader::with_capacity(16, Cursor::new(data));
        assert!(read_line_limited(&mut input, 100).is_err());
        assert!(input.get_ref().position() < 200);
    }

    #[test]
    fn refuses_an_unterminated_last_line_and_bad_utf8() {
        let mut cut = Cursor::new(b"partial".to_vec());
        assert_eq!(
            read_line_limited(&mut cut, 100).unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
        let mut bad = Cursor::new(vec![0xff, 0xfe, b'\n']);
        assert_eq!(
            read_line_limited(&mut bad, 100).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }
}
