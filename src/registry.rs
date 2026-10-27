use std::{fs, path::PathBuf};

use sha2::{Digest, Sha256};

pub struct Registry {
    root: PathBuf,
}

impl Registry {
    pub fn new(root: PathBuf) -> Self {
        Registry { root }
    }

    fn blobs(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// Return the path for a content digest.
    pub fn path(&self, digest: &str) -> PathBuf {
        self.blobs().join(digest).with_extension("wasm")
    }

    /// Writes bytes to the store and returns the digest.
    // TODO: write to a tempfile and move to guarantee atomic exists+writes
    // read: https://danluu.com/file-consistency/
    pub fn put(&self, bytes: &[u8]) -> std::io::Result<String> {
        let digest = hex::encode(Sha256::digest(bytes));
        let path = self.path(&digest);

        if path.exists() {
            return Ok(digest);
        }

        fs::create_dir_all(self.blobs())?;
        fs::write(path, bytes)?;
        Ok(digest)
    }
}
