use std::io::Write;

use super::common;

const REQUESTFILEUPLOAD_FILE: &str = r##"
- !RequestFileUpload
  compression_method: 1
  encrypt_method: 7
  local_filename: FD01.bin
  remote_filename: /tmp/FD01.bin
  file_size_compressed: 2
  file_size_uncompressed: 3
"##;
const EXPECTED_REQUESTFILEUPLOAD_FILE: &[&str] = &[
    "38 01 00 0d 2f 74 6d 70 2f 46 44 30 31 2e 62 69 6e 17 04 00 00 00 03 00 00 00 02", // RequestFileTransfer
    "36 01 07 07", // TransferData
    "37",
];

#[tokio::test(flavor = "current_thread")]
async fn requestfileupload() {
    std::fs::File::create("FD01.bin")
        .unwrap()
        .write_all(&[7u8; 2])
        .unwrap();
    let res = common::run_test_scenario_str(REQUESTFILEUPLOAD_FILE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_REQUESTFILEUPLOAD_FILE)));
}
