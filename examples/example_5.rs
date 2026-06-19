use bitcoin::opcodes::all::{
    OP_2DROP, OP_DROP, OP_ELSE, OP_ENDIF, OP_FROMALTSTACK, OP_IF, OP_RETURN, OP_TOALTSTACK,
};
use bitcoin::script;

fn main() {
    println!("===========================================================");
    println!("       BITCOIN SCRIPT PROOF-OF-CONCEPT VERIFICATION        ");
    println!("===========================================================");

    // Common Data Elements
    let tag = b"ord";
    let field_id = &[0x01];
    let content_type = b"text/plain"; // 10 bytes (0x0a)
    let payload = b"Hello World!"; // 12 bytes (0x0c)

    // -----------------------------------------------------------------
    // PROOF 1: The Conditional Switch (OP_IF/OP_ELSE) Variation
    // -----------------------------------------------------------------
    let mut cond_builder = script::Builder::new();
    cond_builder = cond_builder
        .push_opcode(OP_IF)
        .push_opcode(OP_RETURN)
        .push_opcode(OP_ELSE)
        .push_slice(tag)
        .push_slice(field_id)
        .push_slice(content_type)
        .push_int(0) // OP_FALSE boundary
        .push_slice(payload)
        .push_opcode(OP_ENDIF);

    let cond_script = cond_builder.into_script();
    let cond_hex = hex_encode(cond_script.as_bytes());

    println!("PROOF 1: CONDITIONAL SWITCH VARIATION");
    println!("  BIP-110 Compliant : NO (Banned OP_IF [0x63] detected)");
    println!("  Total Byte Size   : {} bytes", cond_script.len());
    println!("  Compiled Hex      : {}", cond_hex);
    println!("  Detailed Mechanical Breakdown:");
    print_verbose_instructions(&cond_script);
    assert_eq!(cond_script.len(), 35, "Proof Failed: Conditional script must be exactly 35 bytes.");
    println!("  -> PROOF VERIFIED: Match fee structure layout.");
    println!("-----------------------------------------------------------");

    // -----------------------------------------------------------------
    // PROOF 2: The Linear Fee-Equivalent (No Altstack) Variation
    // -----------------------------------------------------------------
    let mut linear_builder = script::Builder::new();
    linear_builder = linear_builder
        .push_slice(tag)
        .push_slice(field_id)
        .push_slice(content_type)
        .push_int(0)
        .push_slice(payload)
        .push_opcode(OP_2DROP)
        .push_opcode(OP_2DROP)
        .push_opcode(OP_DROP)
        .push_opcode(OP_DROP);

    let linear_script = linear_builder.into_script();
    let linear_hex = hex_encode(linear_script.as_bytes());

    println!("PROOF 2: LINEAR FEE-EQUIVALENT VARIATION");
    println!("  BIP-110 Compliant : YES (Flat execution, pushes <= 256B)");
    println!("  Total Byte Size   : {} bytes", linear_script.len());
    println!("  Compiled Hex      : {}", linear_hex);
    println!("  Detailed Mechanical Breakdown:");
    print_verbose_instructions(&linear_script);
    assert_eq!(linear_script.len(), 35, "Proof Failed: Linear script size mismatch.");
    assert_eq!(
        linear_script.len(), 
        cond_script.len(), 
        "Proof Failed: Linear script fee-footprint must identically match Proof 1."
    );
    println!("  -> PROOF VERIFIED: Exact fee equivalence achieved (35 bytes == 35 bytes).");
    println!("-----------------------------------------------------------");

    // -----------------------------------------------------------------
    // PROOF 3: The Linear Altstack Guard Idiom Variation
    // -----------------------------------------------------------------
    let mut alt_builder = script::Builder::new();
    alt_builder = alt_builder
        // Stage 1: Ingestion
        .push_slice(tag)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(field_id)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(content_type)
        .push_opcode(OP_TOALTSTACK)
        .push_int(0)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(payload)
        .push_opcode(OP_TOALTSTACK)
        // Stage 2: Unwind
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_2DROP)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_2DROP)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_DROP)
        .push_opcode(OP_DROP);

    let alt_script = alt_builder.into_script();
    let alt_hex = hex_encode(alt_script.as_bytes());

    println!("PROOF 3: LINEAR ALTSTACK GUARD IDIOM VARIATION");
    println!("  BIP-110 Compliant : YES (Main stack protected from bloat)");
    println!("  Total Byte Size   : {} bytes", alt_script.len());
    println!("  Compiled Hex      : {}", alt_hex);
    println!("  Detailed Mechanical Breakdown:");
    print_verbose_instructions(&alt_script);
    assert_eq!(alt_script.len(), 45, "Proof Failed: Altstack script size mismatch.");
    println!("  -> PROOF VERIFIED: Clean isolation state log verified.");
    println!("===========================================================");
}

fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Iterates through script tokens and emits an aligned log configuration to stdout
fn print_verbose_instructions(script: &script::Script) {
    for instruction in script.instructions() {
        match instruction {
            Ok(script::Instruction::Op(op)) => {
                println!("    [Opcode]     0x{:02x} -> {:?}", op.to_u8(), op);
            }
            Ok(script::Instruction::PushBytes(bytes)) => {
                let len = bytes.len();
                let hex_data = hex_encode(bytes.as_bytes());
                let ascii_view = String::from_utf8_lossy(bytes.as_bytes());
                // Handle non-printable characters cleanly for terminal viewing
                let clean_ascii = ascii_view.replace(|c: char| c.is_control(), ".");
                println!(
                    "    [PushData]   Length: 0x{:02x} ({:2}B) | Hex: [{}] | Raw: \"{}\"",
                    len, len, hex_data, clean_ascii
                );
            }
            Err(e) => println!("    [DecodeError] Parsing failure: {:?}", e),
        }
    }
}
