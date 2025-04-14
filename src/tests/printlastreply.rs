use super::common;

const PRINTLASTREPLY: &str = r##"
- !ReadDID
  did: 0xf190
- PrintLastReply
"##;
const EXPECTED_PRINTLASTREPLY: &[&str] = &["22 f1 90"];

#[tokio::test(flavor = "current_thread")]
async fn printlastreply() {
    let res = common::run_test_scenario_str(PRINTLASTREPLY).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_PRINTLASTREPLY)));
}
