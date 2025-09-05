//! Base64 encoding and decoding

pub type Base64 = [u8; 11];
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn to_base64(mut data: u64) -> Base64 {
    let mut res = [0u8; 11];
    for i in &mut res {
        *i = ALPHABET[(data >> 58) as usize];
        data <<= 6;
    }
    res
}
pub fn from_base64(data: Base64) -> Option<u64> {
    let mut res = 0u64;
    for (idx, i) in data.iter().enumerate() {
        let position = ALPHABET.iter().position(|e| e == i)?;
        // TODO: Implement padding properly
        if idx == data.len()-1 {
            res <<= 4;
            res |= (position >> 2) as u64;
        } else {
            res <<= 6;
            res |= position as u64;
        }
    }
    Some(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn encoding() {
        assert_eq!(&to_base64(17917863107911671779), b"+KkJkFEm3+M");
        assert_eq!(&to_base64(7719297102838926729), b"ayBz0QJqjYk");
        assert_eq!(&to_base64(18337237242280001155), b"/nr0HfQpvoM");
        assert_eq!(&to_base64(8483997426082376746), b"db02tXj37Co");
        assert_eq!(&to_base64(17647398926863207995), b"9Ogn0vVKjjs");
    }

    #[test]
    pub fn decoding() {
        assert_eq!(from_base64(*b"+KkJkFEm3+M"), Some(17917863107911671779));
        assert_eq!(from_base64(*b"ayBz0QJqjYk"), Some(7719297102838926729));
        assert_eq!(from_base64(*b"/nr0HfQpvoM"), Some(18337237242280001155));
        assert_eq!(from_base64(*b"db02tXj37Co"), Some(8483997426082376746));
        assert_eq!(from_base64(*b"9Ogn0vVKjjs"), Some(17647398926863207995));
        // Invalid characters
        assert_eq!(from_base64(*b"9_gn0vVKjjs"), None);
        assert_eq!(from_base64(*b"9!gn0vVKjjs"), None);
    }
}
