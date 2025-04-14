use super::common;

const EVALEXPR: &str = r##"
- !EvalExpr
  expression: vin = "VF1R";
- !WriteDID
  did: 0xf190
  data: !EvalExprVarname vin
"##;
const EXPECTED_EVALEXPR: &[&str] = &["2e f1 90 56 46 31 52"];

#[tokio::test(flavor = "current_thread")]
async fn evalexpr() {
    let res = common::run_test_scenario_str(EVALEXPR).await;
    assert_eq!(res, Ok(common::uds_seq(EXPECTED_EVALEXPR)));
}
