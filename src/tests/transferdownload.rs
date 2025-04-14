use std::io::Write;

use super::common;

const TRANSFERDOWNLOAD_FILE: &str = r##"
- !TransferDownload
  compression_method: 1
  encrypt_method: 2
  addr: 19
  filename: /tmp/FD01.bin
  memorysize: 4
"##;
const EXPECTED_TRANSFERDOWNLOAD_FILE: &[&str] = &[
    "34 12 44 00 00 00 13 00 00 00 04", // TransferStart
    "36 01 de ad ba be",                // TransferData of 0xde 0xad 0xba 0xbe
    "37",                               // TransferExit
];
const TRANSFERDOWNLOAD_FILE_BIN: &[&str] = &["de ad ba be"];

#[tokio::test(flavor = "current_thread")]
async fn transferdownload() {
    {
        let mut file = std::fs::File::create("/tmp/FD01.bin").unwrap();
        file.write_all(&common::uds_seq(TRANSFERDOWNLOAD_FILE_BIN)[0])
            .unwrap();
    }
    let res = common::run_test_scenario_str(TRANSFERDOWNLOAD_FILE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_TRANSFERDOWNLOAD_FILE)));
}
