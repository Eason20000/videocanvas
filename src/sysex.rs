//! Roland Sound Canvas MIDI System Exclusive message construction.
//!
//! Bit-packing and Roland-specific address/checksum formatting for
//! SC-55 (16x16), SC-8850 (160x64), and SD-90 (128x64) modes.

/// Pack a slice of booleans into a u8 (MSB-first bit order).
pub fn pack_bits(bits: &[bool]) -> u8 {
    bits.iter().fold(0u8, |acc, &bit| (acc << 1) | bit as u8)
}

/// SC-55/SC-88 mode: 16x16 binary image -> 64 bytes of sysex payload.
pub fn calculate_data(binary: &[bool]) -> Vec<u8> {
    let mut data: Vec<u8> = Vec::with_capacity(64);
    for col in [0, 5, 10] {
        for row in 0..16 {
            data.push(pack_bits(&binary[row * 16 + col..row * 16 + col + 5]));
        }
    }
    for row in 0..16 {
        data.push(pack_bits(&binary[row * 16 + 15..row * 16 + 16]) << 4);
    }
    data
}

/// SC-8850 mode: 160x64 binary image -> 16 sections x 108 bytes each.
pub fn calculate_data_8850(binary: &[bool]) -> Vec<Vec<u8>> {
    let mut sections: Vec<Vec<u8>> = Vec::with_capacity(16);
    for section in 0..16 {
        let mut sysex: Vec<u8> = Vec::with_capacity(108);
        for row in section * 4..section * 4 + 4 {
            for col in (0..156).step_by(6) {
                sysex.push(pack_bits(&binary[row * 160 + col..row * 160 + col + 6]));
            }
            sysex.push(pack_bits(&binary[row * 160 + 156..row * 160 + 160]) << 2);
        }
        sections.push(sysex);
    }
    sections
}

/// SD-90 mode: 128x64 binary image -> 16 sections x 108 bytes each.
pub fn calculate_data_sd90(binary: &[bool]) -> Vec<Vec<u8>> {
    let mut sections: Vec<Vec<u8>> = Vec::with_capacity(16);
    for section in 0..16 {
        let mut sysex: Vec<u8> = Vec::with_capacity(108);
        for row in section * 4..section * 4 + 4 {
            for col in (0..126).step_by(6) {
                sysex.push(pack_bits(&binary[row * 128 + col..row * 128 + col + 6]));
            }
            sysex.push(pack_bits(&binary[row * 128 + 126..row * 128 + 128]) << 4);
            sysex.extend([0; 5]);
        }
        sections.push(sysex);
    }
    sections
}

/// Roland checksum: (0x80 - sum(address ++ data) % 0x80) & 0x7F.
pub fn calculate_checksum(address: &[u8], data: &[u8]) -> u8 {
    (128u8 - (address.iter().chain(data.iter())
        .fold(0u8, |acc, &b| acc.wrapping_add(b)) % 128)) & 127
}

/// Pack a single sysex message (SC-55 mode): header + address + data + checksum.
pub fn pack_sysex_message(data: &[u8]) -> Vec<u8> {
    let mut sysex: Vec<u8> = Vec::with_capacity(72);

    const HEADER: [u8; 4] = [0x41, 0x10, 0x45, 0x12];
    const ADDRESS: [u8; 3] = [0x10, 0x01, 0x00];

    let checksum = calculate_checksum(&ADDRESS, data);

    sysex.extend_from_slice(&HEADER);
    sysex.extend_from_slice(&ADDRESS);
    sysex.extend_from_slice(data);
    sysex.push(checksum);

    sysex
}

/// Pack 16 section sysex messages (SC-8850/SD-90):
/// each section gets its own address `[0x20, section, 0x00]`.
pub fn pack_sysex_message_8850(sections: &[Vec<u8>]) -> Vec<Vec<u8>> {
    const HEADER: [u8; 4] = [0x41, 0x10, 0x45, 0x12];

    sections.iter().enumerate().map(|(section, data)| {
        let mut sysex: Vec<u8> = Vec::with_capacity(116);
        let address: [u8; 3] = [0x20, section as u8, 0x00];
        let checksum = calculate_checksum(&address, data);

        sysex.extend_from_slice(&HEADER);
        sysex.extend_from_slice(&address);
        sysex.extend_from_slice(data);
        sysex.push(checksum);

        sysex
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_bits_basic() {
        assert_eq!(pack_bits(&[true, false, true]), 0b101);
        assert_eq!(pack_bits(&[false, false, false, false, true]), 0b00001);
        assert_eq!(pack_bits(&[true; 6]), 0b111111);
        assert_eq!(pack_bits(&[]), 0);
    }

    #[test]
    fn test_checksum_zero_sum() {
        assert_eq!(calculate_checksum(&[], &[]), 0);
    }

    #[test]
    fn test_checksum_known() {
        assert_eq!(calculate_checksum(&[0x10, 0x01, 0x00], &[0x00]), 0x6F);
        assert_eq!(calculate_checksum(&[0x20, 0x00, 0x00], &[]), 0x60);
    }

    #[test]
    fn test_checksum_boundary_127() {
        assert_eq!(calculate_checksum(&[], &[0x7F]), 0x01);
    }

    #[test]
    fn test_calculate_data_all_false() {
        let input = vec![false; 256];
        let output = calculate_data(&input);
        assert_eq!(output.len(), 64);
        assert_eq!(&output, &vec![0u8; 64]);
    }

    #[test]
    fn test_calculate_data_all_true() {
        let input = vec![true; 256];
        let output = calculate_data(&input);
        assert_eq!(output.len(), 64);
        assert_eq!(&output[..48], &vec![0x1Fu8; 48]);
        assert_eq!(&output[48..], &vec![0x10u8; 16]);
    }

    #[test]
    fn test_calculate_data_8850_dimensions() {
        let input = vec![false; 160 * 64];
        let sections = calculate_data_8850(&input);
        assert_eq!(sections.len(), 16);
        for section in &sections {
            assert_eq!(section.len(), 108);
            assert_eq!(&section[..], &vec![0u8; 108]);
        }
    }

    #[test]
    fn test_calculate_data_sd90_dimensions() {
        let input = vec![false; 128 * 64];
        let sections = calculate_data_sd90(&input);
        assert_eq!(sections.len(), 16);
        for section in &sections {
            assert_eq!(section.len(), 108);
        }
    }

    #[test]
    fn test_pack_sysex_message_header() {
        let msg = pack_sysex_message(&[]);
        assert_eq!(&msg[..4], &[0x41, 0x10, 0x45, 0x12]);
        assert_eq!(&msg[4..7], &[0x10, 0x01, 0x00]);
    }

    #[test]
    fn test_pack_sysex_8850_section_addresses() {
        let sections: Vec<Vec<u8>> = (0..16).map(|_| vec![0u8; 10]).collect();
        let sysexes = pack_sysex_message_8850(&sections);
        assert_eq!(sysexes.len(), 16);
        assert_eq!(&sysexes[0][4..7], &[0x20, 0x00, 0x00]);
        assert_eq!(&sysexes[15][4..7], &[0x20, 0x0F, 0x00]);
    }
}
