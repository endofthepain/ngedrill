pub mod equix {
    // Re-export everything from `equix` to make it accessible
    pub use equix::*;
}

// Import the Keccak256 struct and Digest trait
use sha3::{Digest, Keccak256};

// Import SolverMemory from the public `equix` module
use equix::SolverMemory;

// Other code...

/// Generates a new drillx hash from a challenge and nonce.
#[inline(always)]
pub fn hash(challenge: &[u8; 32], nonce: &[u8; 8]) -> Result<Hash, DrillxError> {
    let mut memory = SolverMemory::default();  // Use SolverMemory from the public module
    let digest = digest_with_memory(&mut memory, challenge, nonce)?;
    Ok(Hash {
        d: digest,
        h: hashv(&digest, nonce),
    })
}

/// Generates a new drillx hash from a challenge and nonce using pre-allocated memory.
#[inline(always)]
pub fn hash_with_memory(
    memory: &mut SolverMemory,
    challenge: &[u8; 32],
    nonce: &[u8; 8],
) -> Result<Hash, DrillxError> {
    let digest = digest_with_memory(memory, challenge, nonce)?;
    Ok(Hash {
        d: digest,
        h: hashv(&digest, nonce),
    })
}

/// Concatenates a challenge and a nonce into a single buffer.
#[inline(always)]
pub fn seed(challenge: &[u8; 32], nonce: &[u8; 8]) -> [u8; 40] {
    let mut result = [0; 40];
    result[0..32].copy_from_slice(challenge);
    result[32..40].copy_from_slice(nonce);
    result
}

/// Constructs a keccak digest from a challenge and nonce using equix hashes and pre-allocated memory.
#[inline(always)]
fn digest_with_memory(
    memory: &mut SolverMemory,
    challenge: &[u8; 32],
    nonce: &[u8; 8],
) -> Result<[u8; 16], DrillxError> {
    let seed = seed(challenge, nonce);
    let equix = equix::EquiXBuilder::new()
        .runtime(equix::RuntimeOption::TryCompile)
        .build(&seed)
        .map_err(|_| DrillxError::BadEquix)?;
    let solutions = equix.solve_with_memory(memory);
    if solutions.is_empty() {
        return Err(DrillxError::NoSolutions);
    }
    let solution = unsafe { solutions.get_unchecked(0) };
    Ok(solution.to_bytes())
}

/// Sorts the provided digest as a list of u16 values.
#[inline(always)]
fn sorted(mut digest: [u8; 16]) -> [u8; 16] {
    unsafe {
        let u16_slice: &mut [u16; 8] = core::mem::transmute(&mut digest);
        u16_slice.sort_unstable();
        digest
    }
}

/// Returns a keccak hash of the provided digest and nonce.
/// The digest is sorted prior to hashing to prevent malleability.
/// Delegates the hash to a syscall if compiled for the solana runtime.
#[cfg(feature = "solana")]
#[inline(always)]
fn hashv(digest: &[u8; 16], nonce: &[u8; 8]) -> [u8; 32] {
    solana_program::keccak::hashv(&[sorted(*digest).as_slice(), &nonce.as_slice()]).to_bytes()
}

/// Calculates a hash from the provided digest and nonce.
/// The digest is sorted prior to hashing to prevent malleability.
#[cfg(not(feature = "solana"))]
#[inline(always)]
fn hashv(digest: &[u8; 16], nonce: &[u8; 8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();  // Create a new Keccak256 instance
    hasher.update(&sorted(*digest));    // Update the hasher with sorted digest
    hasher.update(nonce);              // Update the hasher with nonce
    hasher.finalize().into()           // Finalize the hash and convert it to a [u8; 32]
}

/// Returns true if the digest is a valid equihash construction from the challenge and nonce.
pub fn is_valid_digest(challenge: &[u8; 32], nonce: &[u8; 8], digest: &[u8; 16]) -> bool {
    let seed = seed(challenge, nonce);
    equix::verify_bytes(&seed, digest).is_ok()
}

/// Returns the number of leading zeros on a 32-byte buffer.
pub fn difficulty(hash: [u8; 32]) -> u32 {
    hash.iter().map(|&byte| byte.leading_zeros()).take_while(|&lz| lz == 8).sum()
}

/// The result of a drillx hash.
#[derive(Default)]
pub struct Hash {
    pub d: [u8; 16], // digest
    pub h: [u8; 32], // hash
}

impl Hash {
    /// The leading number of zeros on the hash.
    pub fn difficulty(&self) -> u32 {
        difficulty(self.h)
    }
}

/// A drillx solution that can be efficiently validated on-chain.
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Solution {
    pub d: [u8; 16], // digest
    pub n: [u8; 8],  // nonce
}

impl Solution {
    /// Builds a new verifiable solution from a hash and nonce.
    pub fn new(digest: [u8; 16], nonce: [u8; 8]) -> Solution {
        Solution { d: digest, n: nonce }
    }

    /// Returns true if the solution is valid.
    pub fn is_valid(&self, challenge: &[u8; 32]) -> bool {
        is_valid_digest(challenge, &self.n, &self.d)
    }

    /// Calculates the result hash for a given solution.
    pub fn to_hash(&self) -> Hash {
        let mut d = self.d;
        Hash {
            d: self.d,
            h: hashv(&mut d, &self.n),
        }
    }
}

/// Custom error types for drillx operations.
#[derive(Debug)]
pub enum DrillxError {
    BadEquix,
    NoSolutions,
}

impl std::fmt::Display for DrillxError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            DrillxError::BadEquix => write!(f, "Failed equix"),
            DrillxError::NoSolutions => write!(f, "No solutions"),
        }
    }
}

impl std::error::Error for DrillxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        None
    }
}
