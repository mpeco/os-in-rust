pub fn eight_bit_modulo(byte_array: &[u8]) -> u64 {
    let mut sum: u64 = 0;
    for byte in byte_array {
        sum += *byte as u64;
    }
    sum % ((u8::MAX as u64) + 1)
}
