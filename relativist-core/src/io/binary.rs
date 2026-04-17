//! Binary format: serde + bincode serialization of Net (SPEC-12 R2-R5).

use std::path::Path;

use crate::error::RelativistError;
use crate::net::Net;
use crate::protocol::bincode_v2;

/// Serialize a Net to bytes (bincode v2 — SPEC-18 §3.1).
pub fn serialize_net(net: &Net) -> Result<Vec<u8>, RelativistError> {
    bincode_v2::encode(net)
        .map_err(|e| RelativistError::Config(format!("serialization failed: {}", e)))
}

/// Deserialize a Net from bytes (bincode v2 — SPEC-18 §3.1).
pub fn deserialize_net(bytes: &[u8]) -> Result<Net, RelativistError> {
    bincode_v2::decode_value(bytes)
        .map_err(|e| RelativistError::Config(format!("deserialization failed: {}", e)))
}

/// Load a Net from a .bin file.
pub fn load_bin(path: &Path) -> Result<Net, RelativistError> {
    let bytes = std::fs::read(path)?;
    deserialize_net(&bytes)
        .map_err(|e| RelativistError::Config(format!("failed to deserialize {:?}: {}", path, e)))
}

/// Save a Net to a .bin file.
pub fn save_bin(net: &Net, path: &Path) -> Result<(), RelativistError> {
    let bytes = serialize_net(net)?;
    std::fs::write(path, &bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::net::{Net, PortRef, Symbol};

    #[test]
    fn test_roundtrip_empty_net() {
        let net = Net::new();
        let bytes = serialize_net(&net).unwrap();
        let restored = deserialize_net(&bytes).unwrap();
        assert_eq!(restored.count_live_agents(), 0);
    }

    #[test]
    fn test_roundtrip_net_with_agents() {
        let mut net = Net::new();
        let a = net.create_agent(Symbol::Con);
        let b = net.create_agent(Symbol::Dup);
        net.connect(PortRef::AgentPort(a, 0), PortRef::AgentPort(b, 0));
        let bytes = serialize_net(&net).unwrap();
        let restored = deserialize_net(&bytes).unwrap();
        assert_eq!(restored.count_live_agents(), 2);
    }

    #[test]
    fn test_deserialize_corrupt_data() {
        let result = deserialize_net(&[0xFF, 0xFF, 0x00]);
        assert!(result.is_err());
    }

    #[test]
    fn test_file_roundtrip() {
        let mut net = Net::new();
        net.create_agent(Symbol::Era);
        let path = std::env::temp_dir().join("relativist_test_bin_io.bin");
        save_bin(&net, &path).unwrap();
        let restored = load_bin(&path).unwrap();
        assert_eq!(restored.count_live_agents(), 1);
        let _ = std::fs::remove_file(&path);
    }
}
