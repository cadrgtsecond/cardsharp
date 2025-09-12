//! Base64 encoding and decoding
const ALPHABET: [char; 64] = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R', 'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i', 'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', '+', '/'] ;

pub fn to_base64(mut data: u64) -> String {
    let mut res = String::new();
    for _ in 0..11 {
        res.push(ALPHABET[(data >> 58) as usize]);
        data <<= 6;
    }
    res.push('=');
    res
}
pub fn from_base64(data: &str) -> Option<u64> {
    let mut res = 0u64;
    if data.len() != 12 {
        return None;
    }
    for (idx, i) in data.chars().enumerate() {
        // TODO: Read MSB properly and treat padding properly
        if i == '=' {
            break;
        }
        let position = ALPHABET.iter().position(|e| *e == i)?;
        if idx == data.len() - 2 {
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
#[allow(clippy::unreadable_literal)]
mod tests {
    use super::*;

    #[test]
    pub fn encoding() {
        assert_eq!(&to_base64(17917863107911671779), "+KkJkFEm3+M=");
        assert_eq!(&to_base64(7719297102838926729), "ayBz0QJqjYk=");
        assert_eq!(&to_base64(18337237242280001155), "/nr0HfQpvoM=");
        assert_eq!(&to_base64(8483997426082376746), "db02tXj37Co=");
        assert_eq!(&to_base64(17647398926863207995), "9Ogn0vVKjjs=");
    }

    #[test]
    pub fn decoding() {
        assert_eq!(from_base64("+KkJkFEm3+M="), Some(17917863107911671779));
        assert_eq!(from_base64("ayBz0QJqjYk="), Some(7719297102838926729));
        assert_eq!(from_base64("/nr0HfQpvoM="), Some(18337237242280001155));
        assert_eq!(from_base64("db02tXj37Co="), Some(8483997426082376746));
        assert_eq!(from_base64("9Ogn0vVKjjs="), Some(17647398926863207995));
        // Invalid characters
        assert_eq!(from_base64("9_gn0vVKjjs="), None);
        assert_eq!(from_base64("9!gn0vVKjjs="), None);
    }
}
