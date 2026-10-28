use super::*;

#[test]
fn the_digest_of_abc_is_the_known_vector() {
    assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
}

#[test]
fn the_digest_of_nothing_is_the_known_vector() {
    assert_eq!(sha256_hex(b""), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
}

#[test]
fn a_file_digest_equals_the_digest_of_its_bytes() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("big.bin");
    let bytes: Vec<u8> = (0..200_000u32).map(|index| (index % 251) as u8).collect();
    std::fs::write(&path, &bytes).unwrap();

    assert_eq!(sha256_hex_of_file(&path).unwrap(), sha256_hex(&bytes));
}

#[test]
fn a_missing_file_is_an_io_error() {
    let directory = tempfile::tempdir().unwrap();

    let error = sha256_hex_of_file(&directory.path().join("gone")).unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::NotFound);
}
