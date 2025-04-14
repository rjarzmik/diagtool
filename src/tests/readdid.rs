use super::common;

const READDID_DID: &str = r##"
- !ReadDID
  did: 0xf190
"##;
const EXPECTED_READDID_DID: &[&str] = &["22 f1 90"];

#[tokio::test(flavor = "current_thread")]
async fn readdid_did() {
    let res = common::run_test_scenario_str(READDID_DID).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_READDID_DID)));
}
