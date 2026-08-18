//! The two models, behind the two calls the bench actually makes.
//!
//! Everything here was pinned by reading the fastembed 5.17.4 source in the cargo
//! registry cache, not its docs and not memory: `Bgem3Embedding::embed` returns
//! `Bgem3EmbeddingOutput { dense, sparse, colbert }` from one forward pass, and
//! `TextRerank::rerank` returns scored indices. Dense rows come straight off the
//! ONNX output with no normalisation step visible in the crate, so cosine is
//! computed here after normalising ourselves. Normalising an already normalised
//! vector is the identity, so this is safe in both worlds and assumes nothing.

use std::path::Path;

use fastembed::{
    Bgem3Embedding, Bgem3InitOptions, Bgem3Model, RerankInitOptions, RerankerModel, TextRerank,
};

/// Both heads from the one forward pass. ADR-0017 judged BGE-M3 on its dense head
/// alone, and the sparse head is the one that attacks the failure class we
/// actually measured: vocabulary mismatch at the first stage. It is a learned
/// lexical scorer over the shared XLM-R subword vocabulary, so whether "dormir"
/// lights any of the same tokens as a note keyed "sleep" is exactly what it tests.
pub struct Heads {
    pub dense: Vec<Vec<f32>>,
    /// Token id to weight, sorted by id so scoring is a linear merge.
    pub sparse: Vec<Vec<(usize, f32)>>,
}

pub struct Embedder {
    inner: Bgem3Embedding,
}

impl Embedder {
    /// BGE-M3 quantized to INT8 (`gpahal/bge-m3-onnx-int8`), the crate's default
    /// and the CPU-appropriate one. Note the quantization when comparing against
    /// ADR-0017's numbers: the Python run was full precision, so small score
    /// differences are expected and rank agreement is the check, not score equality.
    pub fn new(cache: &Path) -> Result<Self, String> {
        let opts = Bgem3InitOptions::new(Bgem3Model::BGEM3Q)
            .with_cache_dir(cache.to_path_buf())
            .with_show_download_progress(true);
        Ok(Self { inner: Bgem3Embedding::try_new(opts).map_err(|e| e.to_string())? })
    }

    /// Embeds in small outer batches, keeping dense and sparse and dropping the
    /// ColBERT head per batch. The crate's `embed` accumulates **all three heads
    /// for every input in one call**, and ColBERT is one vector per token: on this
    /// corpus that is roughly 2.8 GB nobody asked for, which on a 16 GB single
    /// channel machine was a real `bad allocation`, not a theoretical one.
    pub fn embed(&mut self, texts: Vec<String>) -> Result<Heads, String> {
        let mut heads = Heads { dense: Vec::with_capacity(texts.len()), sparse: Vec::with_capacity(texts.len()) };

        for batch in texts.chunks(16) {
            let out = self.inner.embed(batch, None).map_err(|e| e.to_string())?;

            for mut v in out.dense {
                let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
                if norm > 0.0 {
                    for x in v.iter_mut() {
                        *x /= norm;
                    }
                }
                heads.dense.push(v);
            }
            for s in out.sparse {
                let mut pairs: Vec<(usize, f32)> = s.indices.into_iter().zip(s.values).collect();
                pairs.sort_by_key(|(i, _)| *i);
                heads.sparse.push(pairs);
            }
            // out.colbert dropped here, which is the point.
        }

        Ok(Heads { dense: heads.dense, sparse: heads.sparse })
    }
}

/// Dot product of two sparse vectors, by linear merge over sorted ids.
pub fn sparse_dot(a: &[(usize, f32)], b: &[(usize, f32)]) -> f32 {
    let (mut i, mut j, mut sum) = (0usize, 0usize, 0f32);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                sum += a[i].1 * b[j].1;
                i += 1;
                j += 1;
            }
        }
    }
    sum
}

/// Which cross encoder to measure. Two, because they answer different questions:
///
/// `Bge` (bge-reranker-v2-m3, Apache 2.0) is the one the product could actually
/// ship. `Jina` (jina-reranker-v2-base-multilingual) carries the only published
/// number on exactly our task shape, MKQA: queries in 26 languages, Portuguese
/// included, against English content. It is also **CC-BY-NC**, so it can inform
/// the decision and cannot be the decision.
#[derive(Clone, Copy)]
pub enum RerankChoice {
    Bge,
    Jina,
}

pub struct Reranker {
    inner: TextRerank,
}

impl Reranker {
    pub fn new(cache: &Path, which: RerankChoice) -> Result<Self, String> {
        let model = match which {
            RerankChoice::Bge => RerankerModel::BGERerankerV2M3,
            // The misspelling is the crate's, verbatim.
            RerankChoice::Jina => RerankerModel::JINARerankerV2BaseMultiligual,
        };
        let opts = RerankInitOptions::new(model)
            .with_cache_dir(cache.to_path_buf())
            .with_show_download_progress(true);
        Ok(Self { inner: TextRerank::try_new(opts).map_err(|e| e.to_string())? })
    }

    /// Scores for every passage against the query, in the passages' own order.
    /// A cross encoder reads question and passage together, so its score means
    /// "does this text answer this question" rather than "are these texts about
    /// similar things", and that difference is the whole reason this mode exists.
    pub fn score(&mut self, query: &str, passages: &[String]) -> Result<Vec<f32>, String> {
        let refs: Vec<&str> = passages.iter().map(|p| p.as_str()).collect();
        let mut results = self
            .inner
            .rerank(query, &refs, false, None)
            .map_err(|e| e.to_string())?;
        // The crate returns them sorted by score; the caller wants passage order.
        results.sort_by_key(|r| r.index);
        Ok(results.into_iter().map(|r| r.score).collect())
    }
}
