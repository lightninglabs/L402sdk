#![no_main]
use l402_proto::{L402Challenge, L402Token, decode_bolt11_amount};
use libfuzzer_sys::fuzz_target;
fuzz_target!(|input: &str| {
    let _ = L402Challenge::from_header(input);
    let _ = L402Token::from_header(input);
    if let Ok(Some(amount)) = decode_bolt11_amount(input) {
        assert!(amount.satoshis() >= amount.satoshis_floor());
    }
});
