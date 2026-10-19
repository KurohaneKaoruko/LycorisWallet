//! Emit the rendered QR SVG for `tools/verify_qr_svg.py`.
//!
//! Each line is `qrsvg_<name>=<payload>|<svg>` where `<svg>` is exactly what
//! the app puts on screen. The Python verifier parses the rounded modules out
//! of the SVG, rasterises them and asks an independent scanner (pyzbar / zbar)
//! to read the payload back — in both the light and the dark palette, so an
//! inverted (light-on-dark) symbol is proven to still scan.

use lycoris_core::qr;

fn main() {
    let payloads = [
        ("bitcoin", "bc1qx0q0wdh4z5a64twecyg0kqyqkpc3h66nv8n2gt"),
        ("ethereum", "0x9231e3Fa53ABF00CeC08DB45a7dEDe19f939a0CB"),
        ("monero", "46XrxrAa5aXcfkFrdSACS2EfmfxHdJkXLcGuDD2ezLNFfrfMgBJPoP5NAEz3N9WSJuhPox5S78AY3PGVeZQczueyDDT66tY"),
    ];
    // 与前端一致的两套配色：始终浅底深码（反色二维码部分扫码器读不出来）
    let palettes = [
        ("light", "#201e1b", "#fdfaf4"),
        ("dark", "#201e1b", "#f6f3ea"),
    ];

    for (name, payload) in payloads {
        for (palette, dark, light) in palettes {
            let svg = qr::svg(payload, dark, light).expect("qr svg");
            // SVG 里不能出现换行，否则夹具文件会被拆行
            println!("qrsvg_{name}_{palette}={payload}|{}", svg.replace('\n', ""));
        }
    }
}
