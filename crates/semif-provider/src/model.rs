#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemifModelTier {
    Phone,
    Desktop,
    HighMemory,
}

impl SemifModelTier {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "phone" => Some(Self::Phone),
            "desktop" => Some(Self::Desktop),
            "high-memory" | "high_memory" => Some(Self::HighMemory),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Phone => "phone",
            Self::Desktop => "desktop",
            Self::HighMemory => "high-memory",
        }
    }

    pub fn pin(self) -> SemifModelPin {
        match self {
            Self::Phone => SemifModelPin {
                tier: self,
                source: "Qwen/Qwen3-0.6B",
                source_revision: "c1899de289a04d12100db370d81485cdf75e47ca",
                gguf_repository: "Qwen/Qwen3-0.6B-GGUF",
                gguf_revision: "23749fefcc72300e3a2ad315e1317431b06b590a",
                gguf_file: "Qwen3-0.6B-Q8_0.gguf",
                gguf_bytes: 639_446_688,
                gguf_sha256: "9465e63a22add5354d9bb4b99e90117043c7124007664907259bd16d043bb031",
            },
            Self::Desktop => SemifModelPin {
                tier: self,
                source: "openbmb/MiniCPM5-2B",
                source_revision: "12a3808a956f869c767195e9266b59c4d21d92e2",
                gguf_repository: "openbmb/MiniCPM5-2B-GGUF",
                gguf_revision: "2079a22f3beaa4e306449978533478fe0522f4b3",
                gguf_file: "MiniCPM5-2B-Q4_K_M.gguf",
                gguf_bytes: 1_561_318_368,
                gguf_sha256: "ec2d5801640099e97d8d7e8003ad4d81f336e757811f03a26173dddf386602fd",
            },
            Self::HighMemory => SemifModelPin {
                tier: self,
                source: "Qwen/Qwen3.5-4B",
                source_revision: "851bf6e806efd8d0a36b00ddf55e13ccb7b8cd0a",
                gguf_repository: "bartowski/Qwen_Qwen3.5-4B-GGUF",
                gguf_revision: "4168f45a16a1290d65a4ec0fa312ae917a4c15d6",
                gguf_file: "Qwen_Qwen3.5-4B-Q4_K_M.gguf",
                gguf_bytes: 3_013_027_808,
                gguf_sha256: "13c16f426047e2de38cd075bdade4a7bcbc8c774384876f677740cda65f8a983",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SemifModelPin {
    pub tier: SemifModelTier,
    pub source: &'static str,
    pub source_revision: &'static str,
    pub gguf_repository: &'static str,
    pub gguf_revision: &'static str,
    pub gguf_file: &'static str,
    pub gguf_bytes: u64,
    pub gguf_sha256: &'static str,
}

impl SemifModelPin {
    pub fn gguf_url(self) -> String {
        format!(
            "https://huggingface.co/{}/resolve/{}/{}",
            self.gguf_repository, self.gguf_revision, self.gguf_file
        )
    }
}

pub fn model_catalog() -> [SemifModelPin; 3] {
    [
        SemifModelTier::Phone.pin(),
        SemifModelTier::Desktop.pin(),
        SemifModelTier::HighMemory.pin(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_uses_immutable_semif_model_revisions() {
        for pin in model_catalog() {
            assert_eq!(pin.source_revision.len(), 40);
            assert_eq!(pin.gguf_revision.len(), 40);
            assert!(pin.gguf_bytes > 0);
            assert_eq!(pin.gguf_sha256.len(), 64);
            assert!(pin
                .gguf_sha256
                .chars()
                .all(|character| character.is_ascii_hexdigit()));
            assert!(pin.gguf_url().contains(pin.gguf_revision));
        }
    }
}
