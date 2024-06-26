use stwo_prover::core::fields::m31::M31;
use stwo_prover::core::vcs::bws_sha256_hash::BWSSha256Hash;
use stwo_prover::core::vcs::bws_sha256_merkle::BWSSha256MerkleHasher;
use stwo_prover::core::vcs::ops::MerkleHasher;

mod bitcoin_script;
use crate::treepp::pushable::{Builder, Pushable};
pub use bitcoin_script::*;

/// A Merkle tree.
pub struct MerkleTree {
    /// Leaf layers, consisting of m31 elements.
    pub leaf_layer: Vec<Vec<M31>>,
    /// Intermediate layers.
    pub intermediate_layers: Vec<Vec<BWSSha256Hash>>,
    /// Root hash.
    pub root_hash: BWSSha256Hash,
}

impl MerkleTree {
    /// Create a new Merkle tree.
    pub fn new(leaf_layer: Vec<Vec<M31>>) -> Self {
        assert!(leaf_layer.len().is_power_of_two());

        let mut intermediate_layers = vec![];
        let mut cur = leaf_layer
            .chunks_exact(2)
            .map(|v| {
                let commit_1 = BWSSha256MerkleHasher::hash_node(None, &v[0]);
                let commit_2 = BWSSha256MerkleHasher::hash_node(None, &v[1]);

                BWSSha256MerkleHasher::hash_node(Some((commit_1, commit_2)), &[])
            })
            .collect::<Vec<BWSSha256Hash>>();
        intermediate_layers.push(cur.clone());

        while cur.len() > 1 {
            cur = cur
                .chunks_exact(2)
                .map(|v| BWSSha256MerkleHasher::hash_node(Some((v[0], v[1])), &[]))
                .collect::<Vec<BWSSha256Hash>>();
            intermediate_layers.push(cur.clone());
        }

        Self {
            leaf_layer,
            intermediate_layers,
            root_hash: cur[0],
        }
    }

    /// Query the Merkle tree and generate a corresponding proof.
    pub fn query(&self, mut pos: usize) -> MerkleTreeTwinProof {
        let logn = self.intermediate_layers.len();
        assert_eq!(pos & 1, 0);

        let mut merkle_tree_proof = MerkleTreeTwinProof {
            left: self.leaf_layer[pos].clone(),
            right: self.leaf_layer[pos | 1].clone(),
            ..Default::default()
        };

        for i in 0..(logn - 1) {
            pos >>= 1;
            merkle_tree_proof
                .siblings
                .push(self.intermediate_layers[i][pos ^ 1]);
        }

        merkle_tree_proof
    }

    /// Verify a Merkle tree proof.
    pub fn verify_twin(
        root_hash: &BWSSha256Hash,
        logn: usize,
        proof: &MerkleTreeTwinProof,
        mut query: usize,
    ) -> bool {
        assert_eq!(proof.siblings.len(), logn - 1);
        assert_eq!(query & 1, 0);

        let left_hash = BWSSha256MerkleHasher::hash_node(None, &proof.left);
        let right_hash = BWSSha256MerkleHasher::hash_node(None, &proof.right);

        let mut leaf_hash = BWSSha256MerkleHasher::hash_node(Some((left_hash, right_hash)), &[]);
        query >>= 1;

        for i in 0..logn - 1 {
            let (f0, f1) = if query & 1 == 0 {
                (leaf_hash, proof.siblings[i])
            } else {
                (proof.siblings[i], leaf_hash)
            };

            leaf_hash = BWSSha256MerkleHasher::hash_node(Some((f0, f1)), &[]);
            query >>= 1;
        }

        leaf_hash == *root_hash
    }
}

/// A Merkle tree proof.
#[derive(Default, Clone, Debug)]
pub struct MerkleTreeTwinProof {
    /// Leaf as an M31 array.
    pub left: Vec<M31>,
    /// Leaf sibling as an M31 array.
    pub right: Vec<M31>,
    /// All the intermediate sibling nodes.
    pub siblings: Vec<BWSSha256Hash>,
}

impl Pushable for MerkleTreeTwinProof {
    fn bitcoin_script_push(self, builder: Builder) -> Builder {
        (&self).bitcoin_script_push(builder)
    }
}

impl Pushable for &MerkleTreeTwinProof {
    fn bitcoin_script_push(self, mut builder: Builder) -> Builder {
        for v in self.left.iter() {
            builder = v.bitcoin_script_push(builder);
        }
        for v in self.right.iter() {
            builder = v.bitcoin_script_push(builder);
        }
        for elem in self.siblings.iter() {
            builder = elem.bitcoin_script_push(builder);
        }
        builder
    }
}

#[cfg(test)]
mod test {
    use crate::merkle_tree::MerkleTree;
    use crate::utils::get_rand_qm31;
    use rand::{Rng, SeedableRng};
    use rand_chacha::ChaCha20Rng;

    #[test]
    fn test_merkle_tree() {
        let mut prng = ChaCha20Rng::seed_from_u64(0);

        let mut last_layer = vec![];
        for _ in 0..1 << 12 {
            let a = get_rand_qm31(&mut prng);
            last_layer.push(a.to_m31_array().to_vec());
        }

        let merkle_tree = MerkleTree::new(last_layer.clone());

        for _ in 0..10 {
            let mut query = (prng.gen::<u32>() % (1 << 12)) as usize;
            if query & 1 != 0 {
                query ^= 1;
            }

            let proof = merkle_tree.query(query);
            assert!(MerkleTree::verify_twin(
                &merkle_tree.root_hash,
                12,
                &proof,
                query
            ));
        }
    }
}
