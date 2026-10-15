// Stand-in for a Rust static archive; the regression concerns its link interface.
extern "C" int sdk_regression_value() { return 42; }
