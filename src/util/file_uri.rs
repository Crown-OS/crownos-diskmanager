use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

const SCHEME: &str = "file://";
const LOCAL_HOST: &str = "localhost";
const HEX_RADIX: u32 = 16;

fn hex_value(digit: u8) -> Option<u8> {
    char::from(digit)
        .to_digit(HEX_RADIX)
        .and_then(|value| u8::try_from(value).ok())
}

fn percent_decode(encoded: &str) -> Option<Vec<u8>> {
    let mut bytes = encoded.bytes();
    let mut decoded = Vec::with_capacity(encoded.len());
    while let Some(byte) = bytes.next() {
        decoded.push(match byte {
            b'%' => (hex_value(bytes.next()?)? << 4) | hex_value(bytes.next()?)?,
            byte => byte,
        });
    }
    Some(decoded)
}

/// The local path a `file://` URI names, decoding percent escapes; `None` for other schemes.
pub fn path_from_file_uri(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix(SCHEME)?;
    let path = match rest.find('/')? {
        0 => rest,
        host_end if &rest[..host_end] == LOCAL_HOST => &rest[host_end..],
        _ => return None,
    };
    Some(PathBuf::from(OsString::from_vec(percent_decode(path)?)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_escaped_paths_decode() {
        assert_eq!(
            path_from_file_uri("file:///home/me/disk.img"),
            Some("/home/me/disk.img".into())
        );
        assert_eq!(
            path_from_file_uri("file:///home/me/My%20Images/ubuntu%2B1.iso"),
            Some("/home/me/My Images/ubuntu+1.iso".into())
        );
        assert_eq!(
            path_from_file_uri("file://localhost/tmp/a.img"),
            Some("/tmp/a.img".into())
        );
    }

    #[test]
    fn foreign_or_broken_uris_are_refused() {
        assert_eq!(path_from_file_uri("https://example.org/a.img"), None);
        assert_eq!(path_from_file_uri("file://server/share/a.img"), None);
        assert_eq!(path_from_file_uri("file:///broken%2"), None);
        assert_eq!(path_from_file_uri("file:///bad%zz"), None);
    }

    #[test]
    fn non_utf8_names_survive() {
        let path = path_from_file_uri("file:///tmp/%FF.img");
        assert_eq!(
            path.map(PathBuf::into_os_string),
            Some(OsString::from_vec(b"/tmp/\xFF.img".to_vec()))
        );
    }
}
