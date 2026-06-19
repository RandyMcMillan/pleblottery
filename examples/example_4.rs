use bitcoin::opcodes::all::{
    OP_2DROP, OP_DROP,
};
use bitcoin::script;

fn main() {
    // 1. Define our Ordinal payload data segments
    let tag = b"ord"; // 3 bytes
    let field_id = &[0x01]; // 1 byte
    let content_type = b"text/plain"; // 9 bytes
    let payload = b"Hello World!"; // 12 bytes

    // 2. Programmatically verify BIP-110 compliance parameters
    assert!(tag.len() <= 256, "BIP-110 violation: tag push > 256 bytes");
    assert!(field_id.len() <= 256, "BIP-110 violation: field_id push > 256 bytes");
    assert!(content_type.len() <= 256, "BIP-110 violation: content_type push > 256 bytes");
    assert!(payload.len() <= 256, "BIP-110 violation: payload push > 256 bytes");

    // 3. Assemble the linear Tapscript sequence cleanly
    let mut builder = script::Builder::new();

    builder = builder
        .push_slice(tag)
        .push_slice(field_id)
        .push_slice(content_type)
        .push_int(0) // Pushes 0x00 (OP_FALSE) cleanly to the stack
        .push_slice(payload)
        // THE CLEANSING TAIL (Replaces the 4 bytes of control flow)
        .push_opcode(OP_2DROP) // Drops "Hello World!" and 0x00
        .push_opcode(OP_2DROP) // Drops "text/plain" and 0x01
        .push_opcode(OP_DROP)  // Drops "ord"
        .push_opcode(OP_DROP); // Consumes tracking flag/signature parameter from witness

    let script = builder.into_script();

    // 4. Output results matching thread layout specifications
    println!("===========================================================");
    println!("BITCOIN TAPSCRIPT BINDINGS & HEX ENGINE (LINEAR)");
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
