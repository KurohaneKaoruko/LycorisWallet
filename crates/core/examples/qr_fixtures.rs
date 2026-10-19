//! Emit QR fixtures for `tools/verify_qr.py`.
//!
//! Each line is `qr_<name>=<payload>|<modules>` where `<modules>` is the
//! row-major module matrix (`1` = dark) without the quiet zone. The Python
//! verifier rasterises the matrix and decodes it with an independent scanner
//! (pyzbar / zbar), then compares the text with the payload.

use lycoris_core::qr;

fn main() {
    let payloads = [
        ("bitcoin", "bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt"),
        ("litecoin", "ltc1qg06z6x74fe6f2qyqkfqk7kle8llgty3s78tfql"),
        ("ethereum", "0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB"),
        ("solana", "5HeimWLA7nee4kKKL8mWbPjMHPXKYTY6gM43LgMk9Q4a"),
    ];
    for (name, payload) in payloads {
        let (width, modules) = qr::matrix(payload).expect("qr encode");
        let encoded: String = modules
            .into_iter()
            .map(|dark| if dark { '1' } else { '0' })
            .collect();
        println!("qr_{name}={payload}|{width}|{encoded}");
    }
}
