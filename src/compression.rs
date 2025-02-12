use std::io::Write as _;

use flate2::Compression;

const MIN_GZIP_LEVEL: Compression = Compression::none();
const MAX_GZIP_LEVEL: Compression = Compression::best();

pub fn pt_gzip(data: &[u8], level: Option<i32>) -> Result<Vec<u8>, std::io::Error> {
    let level = if let Some(level) = level {
        let level = u32::try_from(level).map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "pt_gzip compression level must be non-negative",
            )
        })?;
        if level < MIN_GZIP_LEVEL.level() || level > MAX_GZIP_LEVEL.level() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!(
                    "pt_gzip compression level must be between {} and {}",
                    MIN_GZIP_LEVEL.level(),
                    MAX_GZIP_LEVEL.level()
                ),
            ));
        }
        Compression::new(level)
    } else {
        Compression::default()
    };

    let mut encoder = flate2::write::GzEncoder::new(Vec::new(), level);
    encoder.write_all(data)?;
    encoder.finish()
}

pub fn pt_brotli(data: &[u8]) -> Result<Vec<u8>, std::io::Error> {
    let mut encoder = brotli::CompressorWriter::new(Vec::new(), 4096, 11, 22);
    encoder.write_all(data)?;
    Ok(encoder.into_inner())
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use flate2::read::GzDecoder;

    use super::*;

    fn round_trip_gzip(data: &[u8]) {
        let encoded = pt_gzip(data, None).unwrap();
        let mut decompressed = Vec::new();
        GzDecoder::new(encoded.as_slice())
            .read_to_end(&mut decompressed)
            .unwrap();
        assert_eq!(data, decompressed);
    }

    fn round_trip_brotli(data: &[u8]) {
        let encoded = pt_brotli(data).unwrap();
        let mut decoder = brotli::Decompressor::new(encoded.as_slice(), 4096);
        let mut decompressed = Vec::new();
        decoder.read_to_end(&mut decompressed).unwrap();
        assert_eq!(data, decompressed);
    }

    #[test]
    fn test_compression() {
        round_trip_gzip(b"");
        round_trip_gzip(b"Hello");

        round_trip_brotli(b"");
        round_trip_brotli(b"Hello");
    }
}
