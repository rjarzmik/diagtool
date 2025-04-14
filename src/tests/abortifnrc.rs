use super::common;

const ABORT_ANY_NRC: &str = r"
- !RawUds
  data: !Bytes 22 ff ff
- !RawUds
  data: !Bytes 22 ff ff
- !AbortIfNrc
- !RawUds
  data: !Bytes 22 ff ff
";
const EXPECTED_ANY_NRC: &[&str] = &["22 ff ff", "22 ff ff"];

#[tokio::test(flavor = "current_thread")]
async fn abortifnrc() {
    let res = common::run_test_scenario_str(ABORT_ANY_NRC).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_ANY_NRC)));
}

const ABORT_NRC_10: &str = r"
- !RawUds
  data: !Bytes 22 ff ff
- !RawUds
  data: !Bytes 22 ff ff
- !AbortIfNrc
  nrc: 0x10
- !RawUds
  data: !Bytes 22 ff ff
";
const EXPECTED_NRC_10: &[&str] = &["22 ff ff", "22 ff ff"];

#[tokio::test(flavor = "current_thread")]
async fn abortifnrc10() {
    let res = common::run_test_scenario_str(ABORT_NRC_10).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_NRC_10)));
}

const ABORT_NRC_11: &str = r"
- !RawUds
  data: !Bytes 22 ff ff
- !RawUds
  data: !Bytes 22 ff ff
- !AbortIfNrc
  nrc: 0x11
- !RawUds
  data: !Bytes 22 ff ff
";
const EXPECTED_NRC_11: &[&str] = &["22 ff ff", "22 ff ff", "22 ff ff"];

#[tokio::test(flavor = "current_thread")]
async fn abortifnrc11() {
    let res = common::run_test_scenario_str(ABORT_NRC_11).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_NRC_11)));
}
