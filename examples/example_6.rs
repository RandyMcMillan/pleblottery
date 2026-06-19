use bitcoin::opcodes::all::{
    OP_CHECKSIGVERIFY, OP_ENDIF, OP_FROMALTSTACK, OP_IF, OP_TOALTSTACK,
};
use bitcoin::script::{self, PushBytes, ScriptBuf};

fn generate_bip64mod_script() -> (Vec<String>, ScriptBuf) {
    // Target Configuration
    const TOTAL_CHUNKS: usize = 1000;
    const CHUNK_SIZE: usize = 256; // Bytes per matrix chunk

    // 32-byte Mock Public Key for Stage 1 Guard
    let mock_pubkey_hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    let pubkey_bytes = hex_decode(mock_pubkey_hex);

    let mut asm_lines: Vec<String> = Vec::new();
    let mut builder = script::Builder::new();

    println!("[*] Initializing BIP-64MOD Script Compiler...");
    println!("[*] Configuration: {} chunks of {} bytes (Total Payload: ~256 KB)", TOTAL_CHUNKS, CHUNK_SIZE);
    println!("{}", "-".repeat(70));

    // =====================================================================
    // STAGE 1: CRYPTOGRAPHIC GUARD & SIGNATURE BINDING
    // =====================================================================
    asm_lines.push("# =====================================================================".to_string());
    asm_lines.push("# STAGE 1: CRYPTOGRAPHIC GUARD & SIGNATURE BINDING".to_string());
    asm_lines.push("# =====================================================================".to_string());

    // Safe conversion of standard byte arrays into validated script PushBytes
    let pubkey_push: &PushBytes = pubkey_bytes.as_slice().try_into().unwrap();
    builder = builder.push_slice(pubkey_push);
    asm_lines.push(format!("OP_PUSHBYTES_32 {}", mock_pubkey_hex));

    builder = builder.push_opcode(OP_CHECKSIGVERIFY);
    asm_lines.push("OP_CHECKSIGVERIFY".to_string());
    asm_lines.push("".to_string());

    // =====================================================================
    // STAGE 2: THE BLOCK-HASH LOOKALIKE GATE (Proof 3 Variation)
    // =====================================================================
    asm_lines.push("# =====================================================================".to_string());
    asm_lines.push("# STAGE 2: THE BLOCK-HASH LOOKALIKE GATE (Proof 3 Variation)".to_string());
    asm_lines.push("# =====================================================================".to_string());

    builder = builder.push_int(0); 
    asm_lines.push("OP_FALSE".to_string());

    builder = builder.push_opcode(OP_IF);
    asm_lines.push("OP_IF".to_string());

    let header_str = b"STRUCT: MASSIVE_ALT_STACK";
    let header_push: &PushBytes = header_str.as_slice().try_into().unwrap();
    builder = builder.push_slice(header_push);
    asm_lines.push(format!("OP_PUSHBYTES_25 \"{}\"", String::from_utf8_lossy(header_str)));

    let arb_str = b"arb_data";
    let arb_push: &PushBytes = arb_str.as_slice().try_into().unwrap();
    builder = builder.push_slice(arb_push);
    asm_lines.push(format!("OP_PUSHBYTES_8 \"{}\"", String::from_utf8_lossy(arb_str)));

    builder = builder.push_int(0);
    asm_lines.push("OP_FALSE".to_string());
    asm_lines.push("".to_string());

    // =====================================================================
    // STAGE 3: THE 1,000X DATA CHUNKING MATRIX
    // =====================================================================
    asm_lines.push("# =====================================================================".to_string());
    asm_lines.push("# STAGE 3: THE 1,000X DATA CHUNKING MATRIX".to_string());
    asm_lines.push("# =====================================================================".to_string());

    for i in 1..=TOTAL_CHUNKS {
        let fill_byte = if i % 2 != 0 { 0xaa } else { 0xbb };
        let chunk_data = vec![fill_byte; CHUNK_SIZE];

        let chunk_hex_preview = format!(
            "{:02x}{:02x}{:02x}{:02x}...{:02x}{:02x}{:02x}{:02x}",
            chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3],
            chunk_data[CHUNK_SIZE - 4], chunk_data[CHUNK_SIZE - 3], chunk_data[CHUNK_SIZE - 2], chunk_data[CHUNK_SIZE - 1]
        );

        let chunk_push: &PushBytes = chunk_data.as_slice().try_into().unwrap();
        builder = builder.push_slice(chunk_push);
        builder = builder.push_opcode(OP_TOALTSTACK);

        if i <= 2 || i >= (TOTAL_CHUNKS - 1) {
            asm_lines.push(format!("  # Chunk {}", i));
            asm_lines.push(format!("  OP_PUSHDATA2_256 [{}]", chunk_hex_preview));
            asm_lines.push("  OP_TOALTSTACK".to_string());
        } else if i == 3 {
            asm_lines.push("  # ... [Repeat this pairing 996 more times] ...".to_string());
        }
    }
    asm_lines.push("".to_string());

    // =====================================================================
    // STAGE 4: THE ALTERNATE STACK UNWIND
    // =====================================================================
    asm_lines.push("# =====================================================================".to_string());
    asm_lines.push("# STAGE 4: THE ALTERNATE STACK UNWIND".to_string());
    asm_lines.push("# =====================================================================".to_string());

    for j in (1..=TOTAL_CHUNKS).rev() {
        builder = builder.push_opcode(OP_FROMALTSTACK);

        if j >= (TOTAL_CHUNKS - 1) || j <= 2 {
            asm_lines.push(format!("  OP_FROMALTSTACK # Pulls Chunk {}", j));
        } else if j == (TOTAL_CHUNKS - 2) {
            asm_lines.push("  # ... [Repeat 998 more times to unwind completely] ...".to_string());
        }
    }

    builder = builder.push_opcode(OP_ENDIF);
    asm_lines.push("OP_ENDIF".to_string());

    // Returns ScriptBuf (the owned variant) to resolve sized return type errors
    (asm_lines, builder.into_script())
}

fn main() {
    let (asm_output, script) = generate_bip64mod_script();

    println!("\n{}", "=".repeat(70));
    println!("VERBOSE ASSEMBLY BREAKDOWN");
    println!("{}", "=".repeat(70));
    for line in asm_output {
        println!("{}", line);
    }

    println!("\n{}", "=".repeat(70));
    println!("COMPILATION METRICS");
    println!("{}", "=".repeat(70));
    let total_bytes = script.len();
    println!("Compiled Script Size : {} bytes (~{:.2} KB)", total_bytes, total_bytes as f64 / 1024.0);

    //let hex_encoded = hex_encode(script.as_bytes());
    //println!("\n[+] RAW SCRIPT HEX (Ready for GCC Input / Unit Testing):");
    //println!("{}", "-".repeat(70));
    //println!("{}", hex_encoded);
    //println!("{}", "-".repeat(70));
}

#[allow(unused)]
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

fn hex_decode(hex_str: &str) -> Vec<u8> {
    (0..hex_str.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex_str[i..i + 2], 16).expect("Invalid hex character"))
        .collect()
}
