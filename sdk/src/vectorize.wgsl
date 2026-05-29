// Numeric Vectorization Kernel
// Converts a CSV column (string) into a dense f32 buffer on the GPU for CPU consumption.

@group(0) @binding(0) var<storage, read> data: array<u32>;
@group(0) @binding(1) var<storage, read> offsets: array<u32>;
@group(0) @binding(2) var<storage, read_write> results: array<f32>; // The dense f32 output

struct Params {
    data_len: u32,
    num_rows: u32,
    col_index: u32,
    delimiter: u32,
};

@group(0) @binding(3) var<uniform> params: Params;

// Re-use our fast f32 parser logic
fn parse_f32(start: u32, end: u32) -> f32 {
    var val: f32 = 0.0;
    var decimal_found = false;
    var divisor: f32 = 1.0;
    var sign: f32 = 1.0;
    var has_digits = false;

    for (var i: u32 = start; i < end; i = i + 1u) {
        let byte = (data[i / 4u] >> (8u * (i % 4u))) & 0xFFu;
        
        if (byte == 0x2D) { // '-'
            sign = -1.0;
        } else if (byte == 0x2E) { // '.'
            decimal_found = true;
        } else if (byte >= 0x30 && byte <= 0x39) { // '0'-'9'
            let digit = f32(byte - 0x30);
            if (decimal_found) {
                divisor = divisor * 10.0;
                val = val + (digit / divisor);
            } else {
                val = val * 10.0 + digit;
            }
            has_digits = true;
        }
    }
    
    return select(0.0, val * sign, has_digits);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let row_idx = global_id.x;
    if (row_idx >= params.num_rows) { return; }

    let start = offsets[row_idx];
    let end = select(params.data_len, offsets[row_idx + 1u], row_idx + 1u < params.num_rows);
    
    var current_col: u32 = 0u;
    var field_start: u32 = start;
    var found = false;
    var val: f32 = 0.0;

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

    results[row_idx] = val;
}
