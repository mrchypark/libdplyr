#include <cstdio>
#include <string>
extern "C" int sdk_regression_value();
int main() {
    const std::string message = "Native macOS SDK link regression passed";
    std::puts(message.c_str());
    return sdk_regression_value() == 42 ? 0 : 1;
}
