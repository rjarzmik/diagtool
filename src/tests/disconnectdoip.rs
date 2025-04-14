use super::common;

const DISCON_1S: &str = r"
- !RawUds
  data: !Bytes 22 ff ff
- !DisconnectDoIp
  wait_after_ms: 1000
- !RawUds
  data: !Bytes 22 ff ff
";
const EXPECTED_DISCON: &[&str] = &["22 ff ff", "22 ff ff"];

#[tokio::test(flavor = "current_thread")]
async fn discon_1s() {
    let res = common::run_test_scenario_str(DISCON_1S).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_DISCON)));
}

const DISCON_NONE: &str = r"
- !RawUds
  data: !Bytes 22 ff ff
- !DisconnectDoIp
  wait_after_ms: 1000
- !RawUds
  data: !Bytes 22 ff ff
";

#[tokio::test(flavor = "current_thread")]
async fn discon_none() {
    let res = common::run_test_scenario_str(DISCON_NONE).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_DISCON)));
}
