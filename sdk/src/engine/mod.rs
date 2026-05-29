pub mod ingest;
pub mod search;
pub mod analysis;
pub mod helpers;

use std::fs::File;
use std::io::Result;
use std::path::{Path, PathBuf};
use memmap2::Mmap;
use crate::parsers::Dispatcher;
use std::sync::Arc;
use crate::compute::GpuProcessor;
use crate::analytics::zenscan::ZenScan;

pub enum OffsetIndex {
    Mmapped {
        mmap: Mmap,
        start_offset: usize,
        count: usize,
    },
    Memory(Vec<u64>),
}

impl std::ops::Deref for OffsetIndex {
    type Target = [u64];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Mmapped { mmap, start_offset, count } => unsafe {
                std::slice::from_raw_parts(mmap.as_ptr().add(*start_offset) as *const u64, *count)
            },
            Self::Memory(v) => v.as_slice(),
        }
    }
}

/// The underlying atomic compute unit of the Zen Engine.
///
/// A `BigDataEngine` instance represents a single file on disk mapped directly into 
/// virtual memory using `mmap2`. It abstracts the I/O layer, providing the higher-level
/// modules with a contiguous, zero-copy byte slice `[u8]` of the entire file.
/// 
/// It holds the memory map and an optional vector of line offsets (`OffsetIndex`) 
/// which allows O(1) random access to any row in a massive dataset.
pub struct BigDataEngine {
    /// The memory-mapped file data. Operating system manages paging automatically.
    pub mmap: Mmap,
    pub offsets: OffsetIndex,
    pub(crate) path: PathBuf,
    pub delimiter: u8,
    pub has_header: bool,
    pub rfc_4180: bool,
    pub skip_rows: usize,
    pub dispatcher: Dispatcher,
    pub gpu: Option<Arc<GpuProcessor>>,
    pub forced_format: Option<String>,
    pub is_compressed: bool,
    pub block_hashes: Vec<([u8; 32], usize)>, 
    pub delta: Option<crate::dataset::delta::DeltaManager>,
    pub zenscan: ZenScan,
    pub strip_quotes: bool,
    pub hardware_mode: crate::types::HardwareMode,
}

impl BigDataEngine {
    pub fn new(path: &Path, hardware_mode: crate::types::HardwareMode) -> Result<Self> {
        let file = File::open(path)?;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_compressed = matches!(ext, "gz" | "zst" | "zstd" | "xz");
        
        let mmap = if is_compressed {
            memmap2::MmapOptions::new().len(1).map_anon()?.make_read_only()?
        } else {
            unsafe { Mmap::map(&file)? }
        };
        
        Ok(Self {
            mmap,
            offsets: OffsetIndex::Memory(Vec::new()),
            path: path.to_path_buf(),
            delimiter: b',',
            has_header: true,
            rfc_4180: false,
            skip_rows: 0,
            dispatcher: Dispatcher::new(),
            gpu: None,
            forced_format: None,
            is_compressed,
            block_hashes: Vec::new(),
            delta: None,
            zenscan: ZenScan::new(),
            strip_quotes: false,
            hardware_mode,
        })
    }

    pub fn load_delta(&mut self) -> Result<()> {
        self.delta = Some(crate::dataset::delta::DeltaManager::new(self.path.clone())
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?);
        Ok(())
    }

    pub fn try_enable_gpu(&mut self) -> bool {
        if let Ok(ctx) = crate::compute::GpuContext::new() {
            if let Ok(processor) = GpuProcessor::new(Arc::new(ctx)) {
                self.gpu = Some(Arc::new(processor));
                return true;
            }
        }
        false
    }

    pub fn path(&self) -> &Path { &self.path }

    // --- DELEGATED METHODS ---

    pub fn build_index(&mut self) -> Result<()> {
        ingest::build_index_impl(self)
    }

    pub fn search_rows(&self, query: &str, col: Option<usize>, limit: usize, ignore_case: bool, indices_only: bool) -> Result<Vec<(usize, String)>> {
        search::search_rows_impl(self, query, col, limit, ignore_case, indices_only)
    }

    pub fn search_raw(&self, query: &str, limit: usize, use_gpu: bool, ignore_case: bool, indices_only: bool) -> Result<Vec<(usize, String)>> {
        search::search_raw_impl(self, query, limit, use_gpu, ignore_case, indices_only)
    }

    pub fn search_iocs(&self, ctx: &aho_corasick::AhoCorasick, limit: usize, iocs: &[String]) -> Result<Vec<(usize, String)>> {
        search::search_iocs_impl(self, ctx, limit, iocs)
    }

    pub fn extract_ips(&self, token: Option<&std::sync::atomic::AtomicBool>, mode: crate::types::IpScanMode) -> Result<std::collections::HashMap<crate::analytics::ioc::IpValue, crate::analytics::ioc::IpMetadata>> {
        analysis::extract_ips_impl(self, token, mode)
    }

    pub fn infer_schema(&self, chunk_size: usize, enable_network: bool, skip_rows: usize) -> Result<Vec<crate::types::ColumnSchema>> {
        analysis::infer_schema_impl(self, chunk_size, enable_network, skip_rows)
    }

    pub fn analyze_csv(
        &self,
        options: crate::types::AnalysisOptions,
        filter_col: Option<usize>, filter_min: Option<f64>, filter_max: Option<f64>,
        zone_map: Option<&crate::types::ZoneMap>,
        filter_text: Option<&str>, filter_text_col: Option<usize>,
        filter_ast: Option<&crate::filter::Expr>,
        date_col: Option<usize>, date_from: Option<u32>, date_to: Option<u32>,
        progress_tx: Option<&std::sync::mpsc::Sender<crate::types::FileMetadata>>,
    ) -> Result<crate::types::FileMetadata> {
        analysis::analyze_csv_impl(self, options, filter_col, filter_min, filter_max, zone_map, filter_text, filter_text_col, filter_ast, date_col, date_from, date_to, progress_tx)
    }

    pub fn group_by(&self, group_col: usize, agg_col: usize, filter_ast: Option<&crate::filter::Expr>, use_gpu: bool) -> Result<Vec<crate::types::GroupResult>> {
        analysis::group_by_impl(self, group_col, agg_col, filter_ast, use_gpu)
    }

    pub fn top_n(&self, col: usize, n: usize, desc: bool, filter_ast: Option<&crate::filter::Expr>, use_gpu: bool) -> Result<Vec<String>> {
        analysis::top_n_impl(self, col, n, desc, filter_ast, use_gpu)
    }

    pub fn get_rows(&self, start: usize, end: usize) -> Vec<String> {
        let mut results = Vec::new();
        let data = &self.mmap;
        let limit = end.min(self.offsets.len());
        for i in start..limit {
            let s = self.offsets[i] as usize;
            let mut e = s;
            while e < data.len() && data[e] != b'\n' { e += 1; }
            results.push(String::from_utf8_lossy(&data[s..e]).into_owned());
        }
        results
    }

    pub fn extract_field<'a>(&self, line: &'a [u8], col: usize) -> Option<&'a [u8]> {
        helpers::extract_field(self, line, col)
    }

    pub fn extract_field_rfc4180<'a>(&self, line: &'a [u8], col: usize) -> Option<&'a [u8]> {
        helpers::extract_field_rfc4180(self, line, col)
    }

    pub fn row_matches(&self, line: &[u8], f_col: Option<usize>, f_min: Option<f64>, f_max: Option<f64>, ft_col: Option<usize>, ft: Option<&str>, f_ast: Option<&crate::filter::Expr>, d_col: Option<usize>, d_f: Option<u32>, d_t: Option<u32>) -> bool {
        helpers::row_matches(self, line, f_col, f_min, f_max, ft_col, ft, f_ast, d_col, d_f, d_t)
    }

    pub fn is_json_format(&self) -> bool { helpers::is_json_format(self) }
    pub fn auto_detect_preamble(&self) -> usize { helpers::auto_detect_preamble(self) }

    pub fn export_raw_matches(&self, query: &str, output_path: &std::path::Path, _use_gpu: bool, _ignore_case: bool) -> Result<u64> {
        let options = crate::types::AnalysisOptions { no_index: true, ..Default::default() };
        self.export_rows(output_path, "csv", options, None, None, None, None, Some(query), None, None, None, None, None)
    }
}
