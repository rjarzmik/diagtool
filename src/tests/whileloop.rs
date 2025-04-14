use super::common;

const WHILELOOP: &str = r##"
- !EvalExpr
  expression: vin = "VF1R"; idx = 0;
- !WhileLoop
  condition: idx < 3
  steps:
  - !ReadDID
    did: 0xf190
  - !EvalExpr
    expression: idx = idx + 1;
"##;
const EXPECTED_WHILELOOP: &[&str] = &["22 f1 90", "22 f1 90", "22 f1 90"];

#[tokio::test(flavor = "current_thread")]
async fn whileloop() {
    let res = common::run_test_scenario_str(WHILELOOP).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_WHILELOOP)));
}
