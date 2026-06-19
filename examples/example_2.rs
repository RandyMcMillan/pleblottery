use bitcoin::opcodes::all::{
    OP_2DROP, OP_DROP, OP_FROMALTSTACK, OP_PUSHBYTES_0, OP_TOALTSTACK,
};
use bitcoin::script;

fn main() {
    // 1. Define our generic payload data matching the thread
    let tag = b"tag"; // 3 bytes
    let field_id = &[0x02]; // 1 byte
    let content_type = b"application/octet-stream"; // 24 bytes (0x18)
    let payload = b"Hello World!"; // 12 bytes (0x0c)

    // 2. Programmatically verify BIP-110 compliance parameters
    assert!(tag.len() <= 256, "BIP-110 violation: tag push > 256 bytes");
    assert!(field_id.len() <= 256, "BIP-110 violation: field_id push > 256 bytes");
    assert!(content_type.len() <= 256, "BIP-110 violation: content_type push > 256 bytes");
    assert!(payload.len() <= 256, "BIP-110 violation: payload push > 256 bytes");

    // 3. Assemble the Tapscript matching the altstack clean idiom exactly
    let mut builder = script::Builder::new();

    // STAGE 1: DATA INGESTION (Isolating Elements to Altstack)
    builder = builder
        .push_slice(tag)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(field_id)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(content_type)
        .push_opcode(OP_TOALTSTACK)
        .push_opcode(OP_PUSHBYTES_0) // Serves as the boundary vector (0x00)
        .push_opcode(OP_TOALTSTACK)
        .push_slice(payload)
        .push_opcode(OP_TOALTSTACK);

    // STAGE 2: THE CLEANSING UNWIND (Flushing Data safely)
    builder = builder
        // Flush payload & boundary
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_2DROP)
        // Flush content type & field ID
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_2DROP)
        // Flush initial structural tag
        .push_opcode(OP_FROMALTSTACK)
        .push_opcode(OP_DROP)
        // Final tail drop for tracking flag/signature parameter passed from witness
        .push_opcode(OP_DROP);

    let script = builder.into_script();

    // 4. Output results matching thread layout specifications
    println!("===========================================================");
    println!("BITCOIN TAPSCRIPT BINDINGS & HEX ENGINE");
    println!("===========================================================");
    println!("BIP-110 Compliant: YES (No conditional branches, pushes <= 256B)");
    println!("Total Script Size : {} bytes", script.len());
    println!("-----------------------------------------------------------");
    println!("Compiled Hex Stream:");
    println!("{}", hex::encode(script.as_bytes()));
    println!("-----------------------------------------------------------");
    
    println!("Opcode Execution Log View:");
    for instruction in script.instructions() {
        match instruction {
            Ok(script::Instruction::Op(op)) => println!("  Opcode: {:?}", op),
            Ok(script::Instruction::PushBytes(bytes)) => {
                let ascii = String::from_utf8_lossy(bytes.as_bytes());
                println!("  Push (0x{:02x} bytes): Hex [{}] ASCII [{}]", 
                    bytes.len(), 
                    hex::encode(bytes.as_bytes()), 
                    ascii
                );
            }
            Err(e) => println!("  Error parsing instruction: {:?}", e),
        }
    }
    println!("===========================================================");
}

mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}
