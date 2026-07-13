use md5::Md5;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hashes {
    pub md5: String,
    pub sha1: String,
    pub sha256: String,
}

pub fn compute_content_hashes(content: impl AsRef<[u8]>) -> Hashes {
    let bytes = content.as_ref();
    Hashes {
        md5: format!("{:x}", Md5::digest(bytes)),
        sha1: format!("{:x}", Sha1::digest(bytes)),
        sha256: format!("{:x}", Sha256::digest(bytes)),
    }
}
