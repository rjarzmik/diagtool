use std::io::Write;

use super::common;

const RAWUDS_BYTES: &str = r##"
- !RawUds
  data: !Bytes 22 f1 90
"##;
const EXPECTED_RAWUDS_BYTES: &[&str] = &["22 f1 90"];

#[tokio::test(flavor = "current_thread")]
async fn rawuds_bytes() {
    let res = common::run_test_scenario_str(RAWUDS_BYTES).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_RAWUDS_BYTES)));
}

const RAWUDS_FILE: &str = r##"
- !RawUds
  data: !BinFileName "/tmp/read_vin.bin"
"##;
const EXPECTED_RAWUDS_FILE: &[&str] = &["22 f1 90"];

#[tokio::test(flavor = "current_thread")]
async fn rawuds_filename() {
    {
        let mut file = std::fs::File::create("/tmp/read_vin.bin").unwrap();
        file.write_all(&common::uds_seq(EXPECTED_RAWUDS_FILE)[0])
            .unwrap();
    }
    let res = common::run_test_scenario_str(RAWUDS_FILE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_RAWUDS_FILE)));
}

const RAWUDS_EVALEXPR: &str = r##"
- !EvalExpr
  expression: wvin = (0x2e, 0xf1, 0x90, "VF1R");
- !RawUds
  data: !EvalExprVarname wvin
"##;
const EXPECTED_RAWUDS_EVALEXPR: &[&str] = &["2e f1 90 56 46 31 52"];

#[tokio::test(flavor = "current_thread")]
async fn evalexpr() {
    let res = common::run_test_scenario_str(RAWUDS_EVALEXPR).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_RAWUDS_EVALEXPR)));
}
