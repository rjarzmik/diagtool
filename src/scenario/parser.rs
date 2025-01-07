use serde::{Deserialize, Serialize};

pub type Steps = Vec<Step>;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum FileOrRawBytes {
    #[serde(with = "uds_raw_command")]
    Bytes(Vec<u8>),
    #[serde(with = "bytes_bin_file")]
    BinFileName(Vec<u8>),
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Scenario {
    steps: Steps,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Step {
    AbortIfNrc(AbortIfNrc),
    DisconnectDoIp(DisconnectDoIp),
    PrintLastReply,
    RawUds(RawUds),
    ReadDID(ReadDID),
    ReadSupportedDTC(ReadSupportedDTC),
    SleepMs(usize),
    WriteDID(WriteDID),
    TransferDownload(TransferDownload),
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AbortIfNrc {
    pub nrc: Option<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DisconnectDoIp {
    pub wait_after_ms: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RawUds {
    #[serde(with = "uds_raw_command")]
    pub uds_bytes: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ReadDID {
    pub did: u16,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ReadSupportedDTC {}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct WriteDID {
    pub did: u16,
    pub data: FileOrRawBytes,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TransferDownload {
    pub compression_method: u8,
    pub encrypt_method: u8,
    pub addr: usize,
    pub filename: String,
    pub memorysize: usize,
}

pub fn read_scenario(filename: &str) -> Steps {
    configfile::read_file(filename)
}

mod configfile {
    use super::Steps;
    use serde_yaml::from_reader;

    pub(super) fn read_file(filename: &str) -> Steps {
        let f = std::fs::File::open(filename).unwrap();
        let steps: Steps = from_reader(f).unwrap();
        steps
    }
}

mod uds_raw_command {
    use serde::{self, Deserialize, Deserializer, Serializer};

    fn parse_hex_u8(ins: &str) -> Option<u8> {
        u8::from_str_radix(ins, 16).ok()
    }

    fn parse_hex_u8_multiple(ins: &str) -> Option<(u8, usize)> {
        let ss: Vec<&str> = ins.split('*').collect();
        let cardinality: Option<usize> = match ss.len() {
            1 => Some(1),
            2 => ss[1].parse().ok(),
            _ => None,
        };
        let value = parse_hex_u8(ss[0]);
        if let (Some(cardinality), Some(value)) = (cardinality, value) {
            Some((value, cardinality))
        } else {
            None
        }
    }

    fn parse_uds_command(ins: &str) -> Option<Vec<u8>> {
        let atoms = ins.split(' ').map(parse_hex_u8_multiple);
        let atoms = atoms.into_iter().collect::<Option<Vec<(u8, usize)>>>();
        match atoms {
            Some(atoms) => {
                let mut v = Vec::new();
                for atom in atoms.into_iter() {
                    v.extend_from_slice(&vec![atom.0; atom.1]);
                }
                Some(v)
            }
            None => None,
        }
    }

    pub fn serialize<S>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        let command = parse_uds_command(&s);
        Ok(command.unwrap_or_default())
    }
}

mod bytes_bin_file {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::fs::File;
    use std::io::{self, Read};

    fn read_file(filename: String) -> Result<Vec<u8>, io::Error> {
        let mut file = File::open(filename)?;
        let mut res = vec![];
        let _ = file.read_to_end(&mut res)?;
        Ok(res)
    }

    pub fn serialize<S>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_bytes(bytes)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let filename: String = String::deserialize(deserializer)?;
        read_file(filename).map_err(|_| serde::de::Error::custom("File not found"))
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use serde_yaml::to_writer;

    #[test]
    fn sample_config_file() {
        use std::io;
        let step1 = Step::ReadSupportedDTC(ReadSupportedDTC {});
        let step2 = Step::TransferDownload(TransferDownload {
            compression_method: 1,
            encrypt_method: 0,
            addr: 0xfd01,
            memorysize: 4,
            filename: "FD01.bin".to_string(),
        });
        let step3 = Step::ReadDID(ReadDID { did: 0xf190 });
        let step4 = Step::AbortIfNrc(AbortIfNrc { nrc: Some(0x22) });
        /*
        let step5 = Step::RawUds(RawUds {
                uds_bytes: vec![0x22, 0xfd, 0x01],
        });
        */
        let step6 = Step::AbortIfNrc(AbortIfNrc { nrc: None });
        let scenario = Scenario {
            steps: vec![step1, step2, step3, step4, step6],
        };
        to_writer(&io::stdout(), &scenario).unwrap();
    }
}
