use std::io::Write;

use super::common;

const WRITEDID_BYTES: &str = r##"
- !WriteDID
  did: 0xf190
  data: !Bytes 56 46 31 52
"##;
const EXPECTED_WRITEDID_BYTES: &[&str] = &["2e f1 90 56 46 31 52"];

#[tokio::test(flavor = "current_thread")]
async fn writedid_bytes() {
    let res = common::run_test_scenario_str(WRITEDID_BYTES).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_WRITEDID_BYTES)));
}

const WRITEDID_FILE: &str = r##"
- !WriteDID
  did: 0xf190
  data: !BinFileName "/tmp/vin.bin"
"##;
const EXPECTED_WRITEDID_FILE: &[&str] = &["2e f1 90 56 46 31 52"];
const WRITEDID_FILE_VIN: &[&str] = &["56 46 31 52"];

#[tokio::test(flavor = "current_thread")]
async fn writedid_filename() {
    {
        let mut file = std::fs::File::create("/tmp/vin.bin").unwrap();
        file.write_all(&common::uds_seq(WRITEDID_FILE_VIN)[0])
            .unwrap();
    }
    let res = common::run_test_scenario_str(WRITEDID_FILE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_WRITEDID_FILE)));
}

const WRITEDID_EVALEXPR: &str = r##"
- !EvalExpr
  expression: wvin = "VF1R";
- !WriteDID
  did: 0xf190
  data: !EvalExprVarname wvin
"##;
const EXPECTED_WRITEDID_EVALEXPR: &[&str] = &["2e f1 90 56 46 31 52"];

#[tokio::test(flavor = "current_thread")]
async fn evalexpr() {
    let res = common::run_test_scenario_str(WRITEDID_EVALEXPR).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_WRITEDID_EVALEXPR)));
}
