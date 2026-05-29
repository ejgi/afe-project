use std::fs::File;
use std::io::{Read, BufReader};
use std::path::Path;

pub enum Decompressor {
    Gzip(flate2::read::GzDecoder<File>),
    Zstd(zstd::Decoder<'static, BufReader<File>>),
    Xz(xz2::read::XzDecoder<File>),
    Plain(File),
}

impl Read for Decompressor {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            Self::Gzip(r) => r.read(buf),
            Self::Zstd(r) => r.read(buf),
            Self::Xz(r) => r.read(buf),
            Self::Plain(r) => r.read(buf),
        }
    }
}

pub fn get_decompressor(path: &Path) -> std::io::Result<Decompressor> {
    let file = File::open(path)?;
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    
    match ext {
        "gz" => Ok(Decompressor::Gzip(flate2::read::GzDecoder::new(file))),
        "zst" | "zstd" => {
            let reader = BufReader::new(file);
            Ok(Decompressor::Zstd(zstd::Decoder::with_buffer(reader)?))
        },
        "xz" => Ok(Decompressor::Xz(xz2::read::XzDecoder::new(file))),
        _ => Ok(Decompressor::Plain(file)),
    }
}
