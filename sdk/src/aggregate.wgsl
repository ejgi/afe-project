// Zen Engine: Partial Aggregation Shader (Sum, Min, Max)
// This shader processes chunks of rows and computes partial aggregates for a specific column.

@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read> offsets: array<u32>;
@group(0) @binding(2) var<storage, read_write> results: array<f32>; // [sum, sum_sq, sum_cu, sum_qu, min, max, count] per workgroup

struct Params {
    data_len: u32,
    num_rows: u32,
    col_index: u32,
    delimiter: u32, // b',' usually
};

@group(0) @binding(3) var<uniform> params: Params;
@group(0) @binding(4) var<storage, read> selection_mask: array<u32>;

// Workgroup shared memory for local reduction
var<workgroup> local_sums: array<f32, 256>;
var<workgroup> local_sum_sqs: array<f32, 256>;
var<workgroup> local_sum_cus: array<f32, 256>;
var<workgroup> local_sum_qus: array<f32, 256>;
var<workgroup> local_mins: array<f32, 256>;
var<workgroup> local_maxs: array<f32, 256>;
var<workgroup> local_counts: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) wg_id: vec3<u32>
) {
    let row_idx = global_id.x;
    let lid = local_id.x;
    
    var val: f32 = 0.0;
    var sum_sq: f32 = 0.0;
    var sum_cu: f32 = 0.0;
    var sum_qu: f32 = 0.0;
    var count: f32 = 0.0;
    var r_min: f32 = 3.40282347e38; // f32 MAX
    var r_max: f32 = -3.40282347e38; // f32 MIN

    if (row_idx < params.num_rows) {
        // Check selection mask if provided (bitset)
        let word_idx = row_idx / 32u;
        let bit_idx = row_idx % 32u;
        let is_selected = (selection_mask[word_idx] & (1u << bit_idx)) != 0u;

        if (is_selected) {
            let start = offsets[row_idx];
            let end = select(params.data_len, offsets[row_idx + 1u], row_idx + 1u < params.num_rows);
            
            var current_col: u32 = 0u;
            var field_start: u32 = start;
            var found = false;

            for (var i: u32 = start; i < end; i = i + 1u) {
                let byte = (data[i / 4u] >> (8u * (i % 4u))) & 0xFFu;
                if (byte == params.delimiter) {
                    if (current_col == params.col_index) {
                        val = parse_f32(field_start, i);
                        found = true;
                        break;
                    }
                    current_col = current_col + 1u;
                    field_start = i + 1u;
                }
            }
            if (!found && current_col == params.col_index) {
                val = parse_f32(field_start, end);
                found = true;
            }

            if (found) {
                count = 1.0;
                let v2 = val * val;
                sum_sq = v2;
                sum_cu = v2 * val;
                sum_qu = v2 * v2;
                r_min = val;
                r_max = val;
            }
        }
    }

    // Local reduction
    local_sums[lid] = val;
    local_sum_sqs[lid] = sum_sq;
    local_sum_cus[lid] = sum_cu;
    local_sum_qus[lid] = sum_qu;
    local_mins[lid] = r_min;
    local_maxs[lid] = r_max;
    local_counts[lid] = count;
    storageBarrier();

    // Standard tree reduction
    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (lid < s) {
            local_sums[lid] += local_sums[lid + s];
            local_sum_sqs[lid] += local_sum_sqs[lid + s];
            local_sum_cus[lid] += local_sum_cus[lid + s];
            local_sum_qus[lid] += local_sum_qus[lid + s];
            local_mins[lid] = min(local_mins[lid], local_mins[lid + s]);
            local_maxs[lid] = max(local_maxs[lid], local_maxs[lid + s]);
            local_counts[lid] += local_counts[lid + s];
        }
        storageBarrier();
    }

    // Write back workgroup results
    if (lid == 0u) {
        let base = wg_id.x * 7u;
        results[base] = local_sums[0];
        results[base + 1u] = local_sum_sqs[0];
        results[base + 2u] = local_sum_cus[0];
        results[base + 3u] = local_sum_qus[0];
        results[base + 4u] = local_mins[0];
        results[base + 5u] = local_maxs[0];
        results[base + 6u] = local_counts[0];
    }
}

// Simple f32 parser in WGSL
fn parse_f32(start: u32, end: u32) -> f32 {
    var res: f32 = 0.0;
    var decimal_mult: f32 = 0.0;
    var sign: f32 = 1.0;
    var started = false;

    for (var i = start; i < end; i = i + 1u) {
        let byte = (data[i / 4u] >> (8u * (i % 4u))) & 0xFFu;
        if (byte == 32u) { continue; } // skip spaces
        if (byte == 45u) { sign = -1.0; continue; } // '-'
        if (byte == 46u) { decimal_mult = 0.1; continue; } // '.'
        if (byte >= 48u && byte <= 57u) {
            let digit = f32(byte - 48u);
            if (decimal_mult == 0.0) {
                res = res * 10.0 + digit;
            } else {
                res = res + digit * decimal_mult;
                decimal_mult = decimal_mult * 0.1;
            }
            started = true;
        } else if (started) {
            break;
        }
    }
    return res * sign;
}
