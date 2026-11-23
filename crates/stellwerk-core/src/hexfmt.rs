//! SHA-256 в hex нижнего регистра — единое определение для федерации.
//!
//! Одна реализация вместо копий в `stellwerk-task` и `stellwerk-gateway`
//! (см. внутренний журнал (не публикуется), семейство A2). Побайтовый hex в
//! `gateway/src/shaping.rs` сюда намеренно не сведён: у него своя семантика и
//! свои тесты, это отдельная функция.

use sha2::{Digest, Sha256};

/// SHA-256 в hex нижнего регистра.
///
/// Тело побайтово совпадает с копиями в `session.rs` и `evidence.rs`:
/// `format!("{:x}", Sha256::digest(bytes))`. Хешируется и пустой вход, потому
/// что «данных не было» и «хеш не посчитали» — разные состояния, и пустой
/// строкой они слились бы.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::sha256_hex;

    /// Векторы FIPS 180-4 / RFC 6234, записанные литералами: оракул — стандарт,
    /// а не перереализация SHA-256 (иначе тест мутировался бы вместе с кодом).
    /// Каждый сверен с `sha256sum` (coreutils) на x99; вывод — в отчёте M8-04.
    #[test]
    fn matches_standard_vectors() {
        let vectors: [(&[u8], &str); 4] = [
            // FIPS 180-4: пустой вход, длина 0 байт.
            (
                b"",
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            ),
            // FIPS 180-4: классический одноблочный вектор.
            (
                b"abc",
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            ),
            // FIPS 180-4: два блока, 56 байт — переход через границу блока.
            (
                b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
                "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1",
            ),
            // FIPS 180-4: многоблочный, 112 байт.
            (
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmnhijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu",
                "cf5b16a778af8380036ce59e7b0492370b249b11e8f07a51afac45037afee9d1",
            ),
        ];
        for (input, expected) in vectors {
            assert_eq!(
                sha256_hex(input),
                expected,
                "sha256({:?}) расходится со стандартом",
                String::from_utf8_lossy(input)
            );
        }
    }

    /// Порядок и длина входа влияют на результат, формат — ровно 64 символа
    /// нижнего hex без пропущенных ведущих нулей.
    #[test]
    fn order_and_length_matter() {
        // Перестановка того же множества байт даёт другой дайджест.
        assert_eq!(
            sha256_hex(b"ab"),
            "fb8e20fc2e4c3f248c60c39bd652f3c1347298bb977b8b4d5903b85055620603"
        );
        assert_eq!(
            sha256_hex(b"ba"),
            "970f519c2cadbcefb1e81694f904bc6229dd2a8300e98c6d0d4fc4bfca584140"
        );
        assert_ne!(sha256_hex(b"ab"), sha256_hex(b"ba"));
        // Регистр: только строчные hex-цифры, длина ровно 64.
        let hex = sha256_hex(b"abc");
        assert_eq!(hex.len(), 64);
        assert!(hex
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)));
    }

    /// Многобайтовый UTF-8: длина считается в байтах, а не в символах,
    /// поэтому «привет» (12 байт) — отдельный от ASCII вектор.
    #[test]
    fn multibyte_utf8_bytes_matter() {
        let privet = "привет";
        assert_eq!(privet.len(), 12);
        assert_eq!(
            sha256_hex(privet.as_bytes()),
            "e58f1e8c55fa105bdd3f40e5037eb0b039b5998d52c05e6cd98878dd2da5cab2"
        );
    }
}
