//! Hash SHA-256 por streaming (nunca se carga un archivo completo en memoria).

use crate::paths::long_path;
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

pub const HASH_BUFFER: usize = 1024 * 1024;
const PARTIAL_CHUNK: u64 = 64 * 1024;

/// SHA-256 completo en hexadecimal. `on_bytes` recibe los bytes leídos en cada bloque.
pub fn sha256_file(path: &Path, cancel: &AtomicBool, mut on_bytes: impl FnMut(u64)) -> io::Result<String> {
    let mut f = File::open(long_path(path))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; HASH_BUFFER];
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelado"));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        on_bytes(n as u64);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Hash rápido: tamaño + primeros y últimos 64 KB. Sirve como filtro previo:
/// si difiere, el contenido es distinto con certeza.
pub fn partial_hash(path: &Path) -> io::Result<String> {
    let mut f = File::open(long_path(path))?;
    let size = f.metadata()?.len();
    let mut hasher = Sha256::new();
    hasher.update(size.to_le_bytes());
    let mut buf = vec![0u8; PARTIAL_CHUNK as usize];
    let n = read_up_to(&mut f, &mut buf)?;
    hasher.update(&buf[..n]);
    if size > PARTIAL_CHUNK * 2 {
        f.seek(SeekFrom::Start(size - PARTIAL_CHUNK))?;
        let n = read_up_to(&mut f, &mut buf)?;
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut total = 0;
    while total < buf.len() {
        let n = f.read(&mut buf[total..])?;
        if n == 0 {
            break;
        }
        total += n;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_vector() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("abc.txt");
        std::fs::write(&p, b"abc").unwrap();
        assert_eq!(
            sha256_file(&p, &AtomicBool::new(false), |_| {}).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn partial_hash_distinguishes_head_and_tail() {
        let d = tempfile::tempdir().unwrap();
        let a = d.path().join("a");
        let b = d.path().join("b");
        let mut data = vec![7u8; 500_000];
        std::fs::write(&a, &data).unwrap();
        *data.last_mut().unwrap() = 8;
        std::fs::write(&b, &data).unwrap();
        assert_ne!(partial_hash(&a).unwrap(), partial_hash(&b).unwrap());
        assert_eq!(partial_hash(&a).unwrap(), partial_hash(&a).unwrap());
    }
}
