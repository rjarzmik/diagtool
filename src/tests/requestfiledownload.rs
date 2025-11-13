use super::common;

const REQUESTFILEDOWNLOAD_FILE: &str = r##"
- !RequestFileDownload
  compression_method: 1
  encrypt_method: 7
  local_filename: FD01.bin
  remote_filename: /tmp/FD01.bin
"##;
const EXPECTED_REQUESTFILEDOWNLOAD_FILE: &[&str] = &[
    "38 04 00 0d 2f 74 6d 70 2f 46 44 30 31 2e 62 69 6e 17", // RequestFileTransfer
    "36 01",
    "37",
];

#[tokio::test(flavor = "current_thread")]
async fn requestfiledownload() {
    let res = common::run_test_scenario_str(REQUESTFILEDOWNLOAD_FILE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_REQUESTFILEDOWNLOAD_FILE)));
}
