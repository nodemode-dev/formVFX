const TABLE: [(u64, &str); 12] = [
    (0xe78ed727ab51e5be, "fx.color.grade"),
    (0xb92c5bfe71b5246a, "Color Grade"),
    (0x21fc88b7a76987a9, "Color Grade"),
    (0x0ae221e659965563, "opRgb"),
    (0x89527cc939349363, "opRgb"),
    (0x3240b769badb488f, "opRgb"),
    (0xf81f30fde4da1bb0, "profiles"),
    (0x95a7f11f4242b0cc, "colorScopes"),
    (0x1abb6e95acdb0050, "Gamma 2.2 Encoded opRGB"),
    (0xc99b1f618f5013fa, "Linear opRGB"),
    (0x79014a1f7448bb39, "g22_oprgb"),
    (0x7ec76bebdb71c943, "lin_oprgb"),
];

pub fn fnv(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= b.to_ascii_lowercase() as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

pub fn alias(name: &str) -> Option<&'static str> {
    let h = fnv(name.trim());
    TABLE.iter().find(|(k, _)| *k == h).map(|(_, v)| *v)
}

pub fn canon(name: &str) -> String {
    alias(name).map(str::to_string).unwrap_or_else(|| name.to_string())
}

pub fn de_canon<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    <String as serde::Deserialize>::deserialize(d).map(|s| canon(&s))
}
