use std::{
    fs::File,
    io::{self, Read},
};

use serde::{Deserialize, Serialize};

pub type Steps = Vec<Step>;

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum RawBytes {
    #[serde(with = "uds_raw_command")]
    Bytes(Vec<u8>),
    BinFileName(String),
    EvalExprVarname(String),
}

#[derive(Serialize, Deserialize, PartialEq)]
struct Scenario {
    steps: Steps,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum Step {
    AbortIfNrc(AbortIfNrc),
    DisconnectDoIp(DisconnectDoIp),
    EvalExpr(EvalExpr),
    PrintLastReply,
    RawUds(RawUds),
    ReadDID(ReadDID),
    ReadSupportedDTC(ReadSupportedDTC),
    SleepMs(usize),
    WhileLoop(WhileLoop),
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
pub struct EvalExpr {
    #[serde(with = "evalexpression")]
    pub expression: evalexpression::Expression,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct RawUds {
    pub data: RawBytes,
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
    pub data: RawBytes,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TransferDownload {
    pub compression_method: u8,
    pub encrypt_method: u8,
    pub addr: usize,
    pub filename: String,
    pub memorysize: usize,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct WhileLoop {
    #[serde(with = "evalexpression")]
    pub condition: evalexpression::Expression,
    pub steps: Steps,
}

pub fn read_scenario(filename: &str) -> Steps {
    configfile::read_file(filename)
}

mod configfile {
    use super::Steps;
    use serde_yaml::from_reader;

    pub(super) fn read_file(filename: &str) -> Steps {
        let f = std::fs::File::open(filename)
            .inspect_err(|err| println!("Can't open configuration file {filename}: {err}"))
            .unwrap();
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
        use std::fmt::Write;

        let mut str = String::new();
        for &b in bytes {
            let _ = write!(&mut str, " {:02x}", b);
        }
        if !bytes.is_empty() {
            s.serialize_str(&str[1..])
        } else {
            s.serialize_str("")
        }
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

impl RawBytes {
    pub fn get_bytes(
        &self,
        get_variable: impl FnOnce(&str) -> Result<Vec<u8>, io::Error>,
    ) -> Result<Vec<u8>, io::Error> {
        match self {
            RawBytes::Bytes(vec) => Ok(vec.clone()),
            RawBytes::BinFileName(filename) => Self::read_file(filename),
            RawBytes::EvalExprVarname(varname) => get_variable(varname),
        }
    }

    fn read_file(filename: &str) -> Result<Vec<u8>, io::Error> {
        let mut file = File::open(filename)?;
        let mut res = vec![];
        let _ = file.read_to_end(&mut res)?;
        Ok(res)
    }
}

mod evalexpression {
    use evalexpr;
    use serde::{self, Deserialize, Deserializer, Serializer};

    #[derive(Debug, PartialEq)]
    pub struct Expression {
        pub str: String,
        pub compiled: evalexpr::Node<evalexpr::DefaultNumericTypes>,
    }

    impl TryFrom<&str> for Expression {
        type Error = String;

        fn try_from(s: &str) -> Result<Self, Self::Error> {
            let compiled = evalexpr::build_operator_tree(s)
                .map_err(|err| format!("parse evalexpr: \"{s}\": {err}"))?;
            Ok(Expression {
                str: s.to_owned(),
                compiled,
            })
        }
    }

    pub fn serialize<S>(expr: &Expression, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        s.serialize_str(&expr.str)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Expression, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: String = String::deserialize(deserializer)?;
        Expression::try_from(s.as_str()).map_err(|err| {
            serde::de::Error::custom(format!("Cannot parse evalexpr: \"{s}\": {err}"))
        })
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
        let step7 = Step::EvalExpr(EvalExpr {
            expression: evalexpression::Expression::try_from("a = 1;").unwrap(),
        });
        let step8 = Step::WhileLoop(WhileLoop {
            condition: evalexpression::Expression::try_from("a < 3;").unwrap(),
            steps: vec![
                Step::ReadDID(ReadDID { did: 0xf190 }),
                Step::ReadDID(ReadDID { did: 0xf191 }),
                Step::EvalExpr(EvalExpr {
                    expression: evalexpression::Expression::try_from("a = a + 1;").unwrap(),
                }),
            ],
        });

        let scenario = Scenario {
            steps: vec![step1, step2, step3, step4, step6, step7, step8],
        };
        to_writer(&io::stdout(), &scenario).unwrap();
    }

    #[test]
    fn generate_references() {
        use std::io;

        let steps = generate_all_possible_steps();
        let scenario1 = Scenario { steps };
        to_writer(&io::stdout(), &scenario1).unwrap();

        // Check bijection of yaml and deserialized structure
        use std::io::Cursor;
        let mut str1: Vec<u8> = vec![];
        to_writer(Cursor::new(&mut str1), &scenario1).unwrap();
        let scenario2: Scenario = serde_yaml::from_reader(Cursor::new(&mut str1)).unwrap();
        assert_eq!(&scenario1.steps, &scenario2.steps);
    }

    fn generate_all_possible_steps() -> Steps {
        vec![
            Step::AbortIfNrc(AbortIfNrc { nrc: Some(0x10) }),
            Step::AbortIfNrc(AbortIfNrc { nrc: None }),
            Step::DisconnectDoIp(DisconnectDoIp {
                wait_after_ms: Some(1000),
            }),
            Step::DisconnectDoIp(DisconnectDoIp {
                wait_after_ms: None,
            }),
            Step::EvalExpr(EvalExpr {
                expression: "a = a + 1;".try_into().unwrap(),
            }),
            Step::EvalExpr(EvalExpr {
                expression: "print(reply)".try_into().unwrap(),
            }),
            Step::EvalExpr(EvalExpr {
                expression: "print(reply_nth(0))".try_into().unwrap(),
            }),
            Step::EvalExpr(EvalExpr {
                expression: "vin = loadfile(\"vin.bin\"); print(vin);"
                    .try_into()
                    .unwrap(),
            }),
            Step::EvalExpr(EvalExpr {
                expression: "reply_nth(0) == 0x62".try_into().unwrap(),
            }),
            Step::PrintLastReply,
            Step::RawUds(RawUds {
                data: RawBytes::BinFileName("raw_file.bin".to_string()),
            }),
            Step::RawUds(RawUds {
                data: RawBytes::EvalExprVarname("request".to_string()),
            }),
            Step::RawUds(RawUds {
                data: RawBytes::Bytes(vec![0x22, 0xf1, 0x90]),
            }),
            Step::ReadDID(ReadDID { did: 0xf190 }),
            Step::SleepMs(1000),
            Step::WhileLoop(WhileLoop {
                condition: evalexpression::Expression::try_from("a < 3").unwrap(),
                steps: vec![
                    Step::ReadDID(ReadDID { did: 0xf190 }),
                    Step::EvalExpr(EvalExpr {
                        expression: evalexpression::Expression::try_from("a = a + 1;").unwrap(),
                    }),
                ],
            }),
            Step::WriteDID(WriteDID {
                did: 0xf190,
                data: RawBytes::Bytes("VF1FRSYSBENCH01".as_bytes().to_vec()),
            }),
            Step::WriteDID(WriteDID {
                did: 0xf190,
                data: RawBytes::BinFileName("toto.bin".to_string()),
            }),
            Step::WriteDID(WriteDID {
                did: 0xf190,
                data: RawBytes::EvalExprVarname("vin".to_string()),
            }),
            Step::TransferDownload(TransferDownload {
                compression_method: 0x01,
                encrypt_method: 0x0,
                addr: 0x4000,
                filename: "FD01.bin".to_string(),
                memorysize: 10240,
            }),
        ]
    }
}
