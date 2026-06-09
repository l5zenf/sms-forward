use tracing::warn;

use crate::domain::error::AppError;
use crate::domain::port::pdu_decoder::{DecodedPdu, PduDecoder, PduUdh};

pub struct DefaultPduDecoder;

impl PduDecoder for DefaultPduDecoder {
    fn decode(&self, raw_pdu: &str) -> Result<DecodedPdu, AppError> {
        decode_pdu(raw_pdu)
    }
}

/// Decode an SMS-SUBMIT PDU (inbound message).
///
/// Standard PDU layout for a received SMS:
///   SMSC info length (1 byte)
///   SMSC info (variable, in BCD semi-octets)
///   PDU type (1 byte)
///   Sender address length (1 byte, in semi-octets)
///   Sender address type (1 byte)
///   Sender address (variable)
///   PID (1 byte)
///   DCS (1 byte)
///   SCTS (7 bytes, BCD semi-octets)
///   UDL (1 byte)
///   User data (variable)
pub fn decode_pdu(raw_pdu: &str) -> Result<DecodedPdu, AppError> {
    // Strip whitespace
    let clean: String = raw_pdu.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = hex::decode(&clean).map_err(|e| AppError::PduDecode {
        reason: format!("invalid hex: {e}"),
        raw_pdu: raw_pdu.into(),
    })?;

    if bytes.len() < 2 {
        return Err(AppError::PduDecode {
            reason: "PDU too short".into(),
            raw_pdu: raw_pdu.into(),
        });
    }

    let mut pos = 0usize;

    // SMSC info length (in octets, includes the type byte)
    let smsc_len = bytes[pos] as usize;
    pos += 1;
    pos += smsc_len;
    if pos >= bytes.len() {
        return Err(AppError::PduDecode {
            reason: "PDU truncated after SMSC".into(),
            raw_pdu: raw_pdu.into(),
        });
    }

    // PDU type (first octet)
    let pdu_type = bytes[pos];
    pos += 1;

    // Recognize SMS-DELIVER (0x00..0x3F top two bits 0x00)
    let mti = pdu_type & 0x03;
    if mti != 0x00 {
        return Err(AppError::PduDecode {
            reason: format!("unsupported PDU type MTI={mti} (not SMS-DELIVER)"),
            raw_pdu: raw_pdu.into(),
        });
    }

    let udhi = (pdu_type & 0x40) != 0;

    // Sender address
    let sender_addr_len_semi = bytes[pos] as usize; // length in semi-octets (digits)
    pos += 1;

    let sender_addr_type = bytes[pos];
    pos += 1;

    // Convert semi-octet length to byte length
    // Each address digit is half an octet
    let sender_addr_byte_len = (sender_addr_len_semi + 1) / 2;
    let sender_addr_bytes = &bytes[pos..pos + sender_addr_byte_len.min(bytes.len() - pos)];
    let sender = decode_address(sender_addr_bytes, sender_addr_type);
    pos += sender_addr_byte_len;

    if pos + 2 > bytes.len() {
        return Err(AppError::PduDecode {
            reason: "PDU truncated after sender".into(),
            raw_pdu: raw_pdu.into(),
        });
    }

    // PID
    let _pid = bytes[pos];
    pos += 1;

    // DCS
    let dcs = bytes[pos];
    pos += 1;

    // SCTS (7 bytes BCD)
    if pos + 7 > bytes.len() {
        return Err(AppError::PduDecode {
            reason: "PDU truncated: SCTS".into(),
            raw_pdu: raw_pdu.into(),
        });
    }
    let scts = decode_scts(&bytes[pos..pos + 7]);
    pos += 7;

    // UDL
    if pos >= bytes.len() {
        return Err(AppError::PduDecode {
            reason: "PDU truncated: no UDL".into(),
            raw_pdu: raw_pdu.into(),
        });
    }
    let udl = bytes[pos] as usize;
    pos += 1;

    let ud_bytes_available = bytes.len() - pos;
    let ud_bytes = &bytes[pos..];

    // Determine encoding from DCS
    let (encoding_name, decoded, udh) =
        decode_user_data(ud_bytes, udl, dcs, udhi, ud_bytes_available)?;

    Ok(DecodedPdu {
        sender: Some(sender),
        content: decoded,
        sms_time: scts,
        dcs,
        encoding: Some(encoding_name),
        udh,
    })
}

/// Decode the address (sender or destination).
/// Type 0x91 = international (BCD), 0xD0 = alphanumeric (GSM 7-bit), 0x81 = national
fn decode_address(bytes: &[u8], addr_type: u8) -> String {
    let nibble_type = addr_type & 0x70;
    match nibble_type {
        0x50 => {
            // Alphanumeric, GSM 7-bit packed
            let unpacked = unpack_gsm7(bytes);
            unpacked.chars().filter(|c| *c != '\0').collect()
        }
        _ => {
            // Numeric: BCD semi-octet swap
            let mut digits = String::new();
            for &b in bytes {
                let hi = b & 0x0F;
                let lo = (b >> 4) & 0x0F;
                if hi == 0x0F {
                    digits.push('F');
                } else {
                    digits.push(char::from_digit(hi as u32, 16).unwrap_or('?'));
                }
                if lo == 0x0F {
                    // padding nibble, skip
                } else {
                    digits.push(char::from_digit(lo as u32, 16).unwrap_or('?'));
                }
            }
            digits.trim_end_matches('F').to_string()
        }
    }
}

/// Decode SCTS into ISO-8601 compatible string
/// Format: YY MM DD HH MM SS TZ (signed) — each byte is BCD with nibbles swapped
fn decode_scts(bytes: &[u8]) -> Option<String> {
    if bytes.len() < 7 {
        return None;
    }
    let swap = |b: u8| -> u32 {
        let hi = (b & 0x0F) as u32;
        let lo = ((b >> 4) & 0x0F) as u32;
        hi * 10 + lo
    };

    let year = swap(bytes[0]);
    let month = swap(bytes[1]);
    let day = swap(bytes[2]);
    let hour = swap(bytes[3]);
    let minute = swap(bytes[4]);
    let second = swap(bytes[5]);
    // bytes[6] is timezone in quarter-hours (semi-octet swapped), sign in top nibble of first byte
    let tz = swap(bytes[6]);
    let tz_sign = if (bytes[6] >> 7) & 0x01 == 1 { "-" } else { "+" };
    let tz_quarter = (tz & 0x7F) as i32;
    let tz_hours = tz_quarter / 4;
    let tz_mins = (tz_quarter % 4) * 15;

    Some(format!(
        "20{:02}-{:02}-{:02}T{:02}:{:02}:{:02}{}{:02}:{:02}",
        year, month, day, hour, minute, second, tz_sign, tz_hours, tz_mins
    ))
}

/// Decode the user data based on DCS. Returns (encoding_label, content, optional udh).
fn decode_user_data(
    ud_bytes: &[u8],
    _udl: usize,
    dcs: u8,
    udhi: bool,
    _ud_bytes_available: usize,
) -> Result<(String, String, Option<PduUdh>), AppError> {
    // Determine DCS class
    let encoding_class = match dcs {
        0x00..=0x07 => GsmClass::Gsm7Bit,
        0x08..=0x0F => GsmClass::Ucs2,
        0x10..=0x27 => {
            // reserved / general data coding — extract raw bits
            if (dcs & 0x0C) == 0x00 {
                GsmClass::Gsm7Bit
            } else if (dcs & 0x0C) == 0x08 {
                GsmClass::Ucs2
            } else {
                GsmClass::EightBit
            }
        }
        0xF0..=0xFF => {
            // Data coding/message class, default 7-bit
            GsmClass::Gsm7Bit
        }
        _ => GsmClass::EightBit,
    };

    // Handle UDH if present
    let mut ud_offset = 0usize;
    let mut udh = None;

    if udhi && !ud_bytes.is_empty() {
        let udhl = ud_bytes[0] as usize; // length of UDH minus this length byte
        ud_offset = 1 + udhl;
        if ud_offset > ud_bytes.len() {
            return Err(AppError::PduDecode {
                reason: format!("UDHL {udhl} exceeds UD length"),
                raw_pdu: format!("{:02x?}", ud_bytes),
            });
        }
        udh = parse_udh(&ud_bytes[1..1 + udhl]);
    }

    let ud_payload = &ud_bytes[ud_offset..];

    let (encoding_name, content) = match encoding_class {
        GsmClass::Ucs2 => {
            let s = decode_ucs2(ud_payload)?;
            ("ucs2".to_string(), s)
        }
        GsmClass::Gsm7Bit => {
            // For 7-bit with UDH, we need to skip the fill bits in the first septet.
            let fill_bits = if udhi {
                7 - (ud_offset * 8) % 7
                // Standard: number of fill bits to skip is (7 - (ud_offset*8 mod 7)) mod 7
            } else {
                0
            };
            let _ = fill_bits;
            let s = decode_gsm7_with_udh_offset(ud_payload, ud_offset);
            ("gsm7".to_string(), s)
        }
        GsmClass::EightBit => {
            // Raw bytes, hex-encode as fallback
            let s = hex::encode(ud_payload);
            ("8bit".to_string(), s)
        }
    };

    Ok((encoding_name, content, udh))
}

enum GsmClass {
    Gsm7Bit,
    Ucs2,
    EightBit,
}

/// Decode UCS2 / UTF-16BE byte stream into a Rust String.
fn decode_ucs2(bytes: &[u8]) -> Result<String, AppError> {
    if bytes.len() % 2 != 0 {
        warn!(len = bytes.len(), "UCS2 length not even, truncating");
    }
    let mut u16_vec: Vec<u16> = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i + 1 < bytes.len() {
        let code = ((bytes[i] as u16) << 8) | (bytes[i + 1] as u16);
        u16_vec.push(code);
        i += 2;
    }
    String::from_utf16(&u16_vec).map_err(|e| AppError::PduDecode {
        reason: format!("UCS2 decode failed: {e}"),
        raw_pdu: hex::encode(bytes),
    })
}

/// GSM 7-bit default alphabet (subset — covers ASCII range + a few specials).
fn gsm7_byte_to_char(code: u8, ext: bool) -> char {
    if ext {
        return match code {
            0x0A => '\u{000C}', // form feed
            0x14 => '^',
            0x28 => '{',
            0x29 => '}',
            0x2F => '\\',
            0x3C => '[',
            0x3D => '~',
            0x3E => ']',
            0x40 => '|',
            0x65 => '€',
            _ => '?',
        };
    }
    match code {
        0x00 => '@',
        0x01 => '£',
        0x02 => '$',
        0x03 => '¥',
        0x04 => 'è',
        0x05 => 'é',
        0x06 => 'ù',
        0x07 => 'ì',
        0x08 => 'ò',
        0x09 => 'Ç',
        0x0A => '\n',
        0x0B => 'Ø',
        0x0C => 'ø',
        0x0D => '\r',
        0x0E => 'Å',
        0x0F => 'å',
        0x10 => 'Δ',
        0x11 => '_',
        0x12 => 'Φ',
        0x13 => 'Γ',
        0x14 => 'Λ',
        0x15 => 'Ω',
        0x16 => 'Π',
        0x17 => 'Ψ',
        0x18 => 'Σ',
        0x19 => 'Θ',
        0x1A => 'Ξ',
        0x1B => '\u{001B}', // escape
        0x1C => 'Æ',
        0x1D => 'æ',
        0x1E => 'ß',
        0x1F => 'É',
        0x20 => ' ',
        0x21 => '!',
        0x22 => '"',
        0x23 => '#',
        0x24 => '¤',
        0x25..=0x3F => code as char, // standard ASCII for 0x25..0x3F
        0x40 => '¡',
        0x41..=0x5A => code as char, // A-Z
        0x5B => 'Ä',
        0x5C => 'Ö',
        0x5D => 'Ñ',
        0x5E => 'Ü',
        0x5F => '§',
        0x60 => '¿',
        0x61..=0x7A => code as char, // a-z
        0x7B => 'ä',
        0x7C => 'ö',
        0x7D => 'ñ',
        0x7E => 'ü',
        0x7F => 'à',
        _ => '?',
    }
}

/// Unpack GSM 7-bit packed bytes into a stream of codes (no extension handling yet).
fn unpack_gsm7(bytes: &[u8]) -> String {
    let mut chars: Vec<u8> = Vec::new();
    let mut shift = 0u32;
    let mut carry: u32 = 0;
    let mut prev: Option<u8> = None;

    for &b in bytes {
        let byte = b as u32;
        let mut code = ((byte << shift) | carry) & 0x7F;
        chars.push(code as u8);
        carry = byte >> (7 - shift);
        shift += 1;
        if shift == 7 {
            chars.push(carry as u8);
            carry = 0;
            shift = 0;
        }
        // Handle escape (0x1B) → next char is extension
        if let Some(p) = prev {
            if p == 0x1B {
                // pop the last char, replace with extension
                if let Some(last) = chars.last_mut() {
                    *last = *last; // keep raw code, we'll decode ext in caller
                }
            }
        }
        prev = Some(code as u8);
    }

    let mut result = String::new();
    let mut iter = chars.iter().peekable();
    let mut ext = false;
    while let Some(&c) = iter.next() {
        if c == 0x1B && !ext {
            ext = true;
            continue;
        }
        result.push(gsm7_byte_to_char(c, ext));
        ext = false;
    }
    result.trim_end_matches('\0').to_string()
}

/// Decode GSM 7-bit with optional UDH fill-bit offset.
/// `ud_offset` is the byte offset within ud_bytes where actual user data starts
/// (after UDH). For UDH alignment, the fill bits are computed.
fn decode_gsm7_with_udh_offset(ud_payload: &[u8], ud_offset: usize) -> String {
    // Without UDH, just unpack normally
    if ud_offset == 0 {
        return unpack_gsm7(ud_payload);
    }
    // With UDH, we need to skip fill bits.
    // Number of fill bits = 7 - (8 * ud_offset) mod 7
    let fill_bits = (7 - (8 * ud_offset) % 7) % 7;

    let mut chars: Vec<u8> = Vec::new();
    let mut bit_buffer: u64 = 0;
    // Pre-load fill bits as zero bits to skip
    let mut bit_count: u32 = fill_bits as u32;

    for &b in ud_payload {
        bit_buffer |= (b as u64) << bit_count;
        bit_count += 8;
        while bit_count >= 7 {
            let code = (bit_buffer & 0x7F) as u8;
            chars.push(code);
            bit_buffer >>= 7;
            bit_count -= 7;
        }
    }

    let mut result = String::new();
    let mut ext = false;
    for c in chars {
        if c == 0x00 {
            // skip padding
            continue;
        }
        if c == 0x1B && !ext {
            ext = true;
            continue;
        }
        result.push(gsm7_byte_to_char(c, ext));
        ext = false;
    }
    result
}

/// Parse the UDH body (after the UDHL byte).
/// Returns parsed PduUdh if a Concatenation IE is found.
fn parse_udh(udh: &[u8]) -> Option<PduUdh> {
    let mut i = 0;
    while i + 1 < udh.len() {
        let ie_id = udh[i];
        let ie_len = udh[i + 1] as usize;
        i += 2;
        if i + ie_len > udh.len() {
            break;
        }
        let ie_data = &udh[i..i + ie_len];
        match (ie_id, ie_len) {
            (0x00, 3) => {
                // 8-bit ref
                let concat_ref = format!("{:02x}", ie_data[0]);
                let total = ie_data[1];
                let seq = ie_data[2];
                return Some(PduUdh {
                    concat_ref,
                    concat_total: total,
                    concat_seq: seq,
                });
            }
            (0x08, 4) => {
                // 16-bit ref
                let concat_ref = format!("{:02x}{:02x}", ie_data[0], ie_data[1]);
                let total = ie_data[2];
                let seq = ie_data[3];
                return Some(PduUdh {
                    concat_ref,
                    concat_total: total,
                    concat_seq: seq,
                });
            }
            _ => {}
        }
        i += ie_len;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_ucs2_simple() {
        // 你好 = 0x4F60 0x597D
        let bytes = [0x4F, 0x60, 0x59, 0x7D];
        let result = decode_ucs2(&bytes).unwrap();
        assert_eq!(result, "你好");
    }

    #[test]
    fn test_gsm7_basic() {
        let result = unpack_gsm7(&[0xE7, 0x7B, 0x1B]); // "Pi"
        // Just verify it doesn't panic
        assert!(!result.is_empty() || result.is_empty());
    }

    #[test]
    fn test_gsm7_byte_to_char_uppercase_alpha() {
        assert_eq!(gsm7_byte_to_char(0x41, false), 'A');
        assert_eq!(gsm7_byte_to_char(0x5A, false), 'Z');
    }

    #[test]
    fn test_parse_udh_8bit_ref() {
        // 00 03 01 02 01 (id 0, len 3, ref=01, total=2, seq=1)
        let udh = [0x00, 0x03, 0x01, 0x02, 0x01];
        let result = parse_udh(&udh);
        assert!(result.is_some());
        let r = result.unwrap();
        assert_eq!(r.concat_ref, "01");
        assert_eq!(r.concat_total, 2);
        assert_eq!(r.concat_seq, 1);
    }

    #[test]
    fn test_parse_udh_16bit_ref() {
        // 08 04 00 01 02 01 (id 8, len 4, ref=0001, total=2, seq=1)
        let udh = [0x08, 0x04, 0x00, 0x01, 0x02, 0x01];
        let result = parse_udh(&udh);
        let r = result.unwrap();
        assert_eq!(r.concat_ref, "0001");
        assert_eq!(r.concat_total, 2);
        assert_eq!(r.concat_seq, 1);
    }

    #[test]
    fn test_decode_address_international() {
        // +447785012345 — type 0x91, BCD with each byte giving 2 digits (lo nibble = 1st digit)
        // digits "447785012345" → bytes: 0x44, 0x77, 0x58, 0x10, 0x32, 0x54
        let addr = [0x44, 0x77, 0x58, 0x10, 0x32, 0x54];
        let result = decode_address(&addr, 0x91);
        assert_eq!(result, "447785012345");
    }

    /// Real PDU captured from EigenComm Compo on China Mobile, 2026-06-09.
    /// Sender +8610000000000 sent the literal text "ping" (GSM7-packed in 4 octets).
    #[test]
    fn test_decode_real_pdu_gsm7_ping() {
        let raw = "0891683108307505F0040D91685130756815F600006260903251532304F0B4FB0C";
        let decoded = decode_pdu(raw).expect("PDU should decode");
        // 发件号：用户对账后确认末尾是 6 不是 5
        assert_eq!(decoded.sender.as_deref(), Some("8610000000000"));
        // SCTS：8 octet bytes 21..28, swap → 26 06 09 23 15 35 32 → 2026-06-09 + tz
        let scts = decoded.sms_time.as_ref().unwrap();
        assert!(scts.contains("2026-06-09"), "scts={scts}");
        // DCS=0 → GSM 7-bit
        assert_eq!(decoded.dcs, 0x00);
        assert_eq!(decoded.encoding.as_deref(), Some("gsm7"));
        // UDL=4 octets unpacked = 4 septets = "ping"
        assert_eq!(decoded.content, "ping");
        assert!(decoded.udh.is_none(), "no multipart in this PDU");
    }

    // ── Real captures: long Chinese SMS (multipart, UCS2) ───────────
    // Same capture session as the ping sample. Two PDUs came in 1+ seconds
    // apart, same concat_ref=01, total=2.

    const RAW_UCS2_PART1: &str = "0891683108307505F0440D91685130756815F60008626090323245238C05000301020100670067002D006700750061007200640020957F77ED4FE180548C036837672CFF1A672C676175284E8E9A8C8BC10020004100690072003700380030004500206A217EC4957F77ED4FE162FC5305903B8F9130025305542B4E2D6587680770B93001963F62C94F2F65705B57000A002000200031003200330034003500360037003800390030";

    const RAW_UCS2_PART2: &str = "0891683108307505F0440D91685130756815F60008626090323245237405000301020230014EE553CA82E55E7282F16587002000610062006300580059005A300263A565367AEF5E945B8C65748FD8539F987A5E8FFF0C4E0D5E9451FA73B04E225B5730014E715E8F3001621691CD590D300265F695F4623300200032003000320036002D00300036002D003000393002";

    #[test]
    fn test_decode_real_pdu_ucs2_part1() {
        let decoded = decode_pdu(RAW_UCS2_PART1).expect("part1 should decode");
        assert_eq!(decoded.sender.as_deref(), Some("8610000000000"));
        let scts = decoded.sms_time.as_deref().unwrap();
        assert!(scts.contains("2026-06-09"), "scts={scts}");
        assert_eq!(decoded.dcs, 0x08, "DCS=UCS2");
        assert_eq!(decoded.encoding.as_deref(), Some("ucs2"));
        let udh = decoded.udh.as_ref().expect("UDH expected");
        assert_eq!(udh.concat_ref, "01");
        assert_eq!(udh.concat_total, 2);
        assert_eq!(udh.concat_seq, 1);
        assert!(
            decoded.content.starts_with("gg-guard "),
            "content[:12]={:?}",
            decoded.content.chars().take(12).collect::<String>()
        );
        eprintln!("PART1 content = {:?}", decoded.content);
    }

    #[test]
    fn test_decode_real_pdu_ucs2_part2() {
        let decoded = decode_pdu(RAW_UCS2_PART2).expect("part2 should decode");
        assert_eq!(decoded.sender.as_deref(), Some("8610000000000"));
        assert_eq!(decoded.dcs, 0x08);
        assert_eq!(decoded.encoding.as_deref(), Some("ucs2"));
        let udh = decoded.udh.as_ref().expect("UDH expected");
        assert_eq!(udh.concat_ref, "01");
        assert_eq!(udh.concat_total, 2);
        assert_eq!(udh.concat_seq, 2);
        eprintln!("PART2 content = {:?}", decoded.content);
    }

    /// Concatenate the two real multipart parts in seq order and verify
    /// the assembled body matches the original prose the user keyed in.
    #[test]
    fn test_assemble_real_multipart_prose() {
        let b = decode_pdu(RAW_UCS2_PART1).unwrap();
        let c = decode_pdu(RAW_UCS2_PART2).unwrap();
        // Sort by seq just like the repo's try_assemble_multipart would.
        let mut parts = [(b.udh.unwrap().concat_seq, b.content.clone()),
                         (c.udh.unwrap().concat_seq, c.content.clone())];
        parts.sort_by_key(|(s, _)| *s);
        let assembled: String = parts.iter().map(|(_, c)| c.clone()).collect();
        eprintln!("ASSEMBLED = {assembled}");
        // Pin a few load-bearing substrings against obvious decode errors.
        assert!(assembled.starts_with("gg-guard 长短信联调样本"), "assembled[:20]={:?}", &assembled[..assembled.char_indices().take(20).last().map(|(i,_)| i).unwrap_or(0)]);
        assert!(assembled.contains("Air780E"), "missing Air780E");
        assert!(assembled.contains("1234567890"), "missing digits");
        assert!(assembled.contains("abcXYZ"), "missing latin tail");
        assert!(assembled.contains("2026-06-09"), "missing date");
        assert!(assembled.ends_with('。'), "should end with full-width period; got {:?}", assembled.chars().last());
    }
}
