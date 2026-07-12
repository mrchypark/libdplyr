#ifndef DPLYR_EXTENSION_HPP
#define DPLYR_EXTENSION_HPP

#include "dplyr.h"
#include "duckdb.hpp"

namespace dplyr {

class DplyrExtension : public duckdb::Extension {
public:
    void Load(duckdb::ExtensionLoader &loader) override;
    std::string Name() override;
    std::string Version() const override { return dplyr_version(); }
};

struct DplyrParserExtension : public duckdb::ParserExtension {
    DplyrParserExtension();
};

struct DplyrParseData : duckdb::ParserExtensionParseData {
    std::string sql;
    bool is_compiled_sql;

    explicit DplyrParseData(std::string sql_p, bool is_compiled_sql_p = true)
        : sql(std::move(sql_p)), is_compiled_sql(is_compiled_sql_p) {}

    duckdb::unique_ptr<duckdb::ParserExtensionParseData> Copy() const override {
        return duckdb::make_uniq_base<duckdb::ParserExtensionParseData, DplyrParseData>(sql, is_compiled_sql);
    }

    std::string ToString() const override { return "DplyrParseData"; }
};

duckdb::ParserExtensionParseResult dplyr_parse(duckdb::ParserExtensionInfo *info, const std::string &query);
duckdb::ParserOverrideResult dplyr_parser_override(
    duckdb::ParserExtensionInfo *info,
    const std::string &query,
    duckdb::ParserOptions &options
);
duckdb::ParserExtensionPlanResult dplyr_plan(
    duckdb::ParserExtensionInfo *info,
    duckdb::ClientContext &context,
    duckdb::unique_ptr<duckdb::ParserExtensionParseData> parse_data
);

} // namespace dplyr

#endif
