// Zen Engine: Row-based GPU Filter Shader
// This shader uses pre-computed line offsets to evaluate patterns per-row.

@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read> pattern: array<u32>;
@group(0) @binding(2) var<storage, read_write> results: array<u32>;

struct Params {
    data_len: u32,
    pattern_len: u32,
    num_rows: u32,
    unused: u32,
};

@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read> offsets: array<u32>;

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let row_idx = global_id.x;
    
    // Boundary check
    if (row_idx >= params.num_rows) {
        return;
    }

    let start = offsets[row_idx];
    let end = select(params.data_len, offsets[row_idx + 1u], row_idx + 1u < params.num_rows);
    let line_len = end - start;

    if (line_len < params.pattern_len) {
        return;
    }

    // Substring search within the row
    var found = false;
    let search_limit = line_len - params.pattern_len + 1u;
    
    for (var i: u32 = 0u; i < search_limit; i = i + 1u) {
        var match = true;
        for (var j: u32 = 0u; j < params.pattern_len; j = j + 1u) {
            let data_pos = start + i + j;
            let data_byte = (data[data_pos / 4u] >> (8u * (data_pos % 4u))) & 0xFFu;
            let pattern_byte = (pattern[j / 4u] >> (8u * (j % 4u))) & 0xFFu;

            if (data_byte != pattern_byte) {
                match = false;
                break;
            }
        }
        if (match) {
            found = true;
            break;
        }
    }

    if (found) {
        results[row_idx / 32u] |= (1u << (row_idx % 32u)); 
    }
}
