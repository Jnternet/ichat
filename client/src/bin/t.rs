use sha2::Digest;

fn main() {
    let s = "123";
    let h = sha2::Sha256::digest(s);
    let he = hex::encode(h.as_slice());

    dbg!(&he);
}
