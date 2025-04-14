use std::time::{Duration, Instant};

use super::common;

const SLEEPMS: &str = r##"
- !SleepMs 250
"##;
const EXPECTED_SLEEPMS: &[&str] = &[];

#[tokio::test(flavor = "current_thread")]
async fn sleepms_did() {
    let start = Instant::now();
    let res = common::run_test_scenario_str(SLEEPMS).await;
    let duration = Instant::now() - start;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_SLEEPMS)));
    assert!(duration > Duration::from_millis(250));
    assert!(duration < Duration::from_millis(300));
}
